mod commands;
mod index;
#[cfg(target_os = "macos")]
mod macos;
mod perf;
mod quit;
mod settings;
mod storage;
mod trace;

use aelomi_core::index::IndexEvent;
use aelomi_core::storage::DocId;
use index::{IndexChanged, IndexStatusChanged};
use quit::{QuitRequest, QuitStatus, QuitStatusChanged};
use storage::{QUIT_FLUSH_TIMEOUT, Storage, StorageHealthChanged};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, RunEvent, Runtime, WindowEvent};
use tauri_specta::Event;

pub fn run() {
    perf::stamp(&perf::MAIN_US);
    let commands = commands::builder();
    let mut builder = tauri::Builder::default()
        .invoke_handler(commands.invoke_handler())
        .on_page_load(|_, payload| match payload.event() {
            PageLoadEvent::Started => perf::stamp(&perf::PAGE_START_US),
            PageLoadEvent::Finished => perf::stamp(&perf::PAGE_DONE_US),
        })
        .setup(move |app| {
            perf::stamp(&perf::SETUP_US);
            commands.mount_events(app);
            #[cfg(target_os = "macos")]
            macos::setup(app)?;
            open_storage(app);
            Ok(())
        });
    if perf::report_path().is_some() {
        builder = builder.plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("perf")
                .js_init_script(perf::ARM_SCRIPT)
                .build(),
        );
    }
    builder
        .build(tauri::generate_context!())
        .expect("failed to build Aelomi")
        .run(on_run_event);
}

fn on_run_event<R: Runtime>(app: &AppHandle<R>, event: RunEvent) {
    match event {
        RunEvent::WindowEvent {
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } => {
            if !flush_then_allow_quit(app, "CloseRequested") {
                api.prevent_close();
            }
        }
        RunEvent::ExitRequested { api, .. } => {
            if !flush_then_allow_quit(app, "ExitRequested") {
                api.prevent_exit();
            }
        }
        RunEvent::Exit => {
            let _ = flush_before_quit(app, "Exit");
        }
        _ => {}
    }
}

fn open_storage(app: &tauri::App) {
    #[cfg(target_os = "macos")]
    let platform = macos::data_location();
    #[cfg(not(target_os = "macos"))]
    let platform = storage::DataLocation {
        path: app.path().app_data_dir().unwrap_or_default(),
        cloud_roots: Vec::new(),
    };
    let location = storage::data_location(
        platform,
        |name| std::env::var_os(name),
        cfg!(debug_assertions),
    );
    let handle = app.handle().clone();
    let started = std::time::Instant::now();
    let storage = Storage::open(&location, move |event| publish_index(&handle, event));
    println!("{}", storage.startup_line());
    println!(
        "storage open ms={} t_ms={:.1}",
        trace::ms(started.elapsed()),
        trace::since_start_ms()
    );
    let settings_owner = storage.workspace().ok().map(settings::SettingsOwner::new);
    if let Some(owner) = settings_owner.clone() {
        tauri::async_runtime::spawn_blocking(move || owner.initialize());
    }
    app.manage(settings::SettingsState(settings_owner));
    if let Ok(workspace) = storage.workspace() {
        warm_tree(workspace.clone());
        let app = app.handle().clone();
        workspace.subscribe_health(move |health| {
            let _ = StorageHealthChanged(health.into()).emit(&app);
        });
    }
    app.manage(storage);
}

/// Projects the page tree while the webview starts, so the first `page_tree` call finds it in
/// the workspace's memo. The memo is checked against the workspace revision, so an append
/// before the call makes it project again.
fn warm_tree(workspace: std::sync::Arc<aelomi_core::workspace::Workspace>) {
    let spawned = std::thread::Builder::new()
        .name("aelomi-tree-warm".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let started = std::time::Instant::now();
            let outcome = match workspace.tree() {
                Ok(tree) => format!("pages={}", tree.pages.len()),
                Err(_) => "failed".to_owned(),
            };
            println!(
                "tree warm {outcome} ms={} t_ms={:.1}",
                trace::ms(started.elapsed()),
                trace::since_start_ms()
            );
        });
    if let Err(e) = spawned {
        println!("tree warm skipped: {e}");
    }
}

fn publish_index<R: Runtime>(app: &AppHandle<R>, event: &IndexEvent) {
    match event {
        IndexEvent::Status(status) => {
            println!(
                "index status={status:?} t_ms={:.1}",
                trace::since_start_ms()
            );
            let _ = IndexStatusChanged(status.clone().into()).emit(app);
        }
        IndexEvent::Cache(state) => {
            println!("index cache={state} t_ms={:.1}", trace::since_start_ms())
        }
        IndexEvent::Changed(change) => {
            let _ = IndexChanged::from(change.clone()).emit(app);
        }
        IndexEvent::Startup(report) => {
            println!("{}", trace::startup_line(report));
            #[cfg(target_os = "macos")]
            match macos::peak_rss_bytes() {
                Ok(bytes) => println!(
                    "index main_peak_rss_bytes={bytes} t_ms={:.1}",
                    trace::since_start_ms()
                ),
                Err(error) => eprintln!("index main_peak_rss_unavailable={error}"),
            }
        }
    }
}

fn flush_then_allow_quit<R: Runtime>(app: &AppHandle<R>, via: &str) -> bool {
    let Some(storage) = app.try_state::<Storage>() else {
        return true;
    };
    match storage.quit().request() {
        QuitRequest::Allow => {
            println!("quit allowed via={via} confirmed=true");
            return true;
        }
        QuitRequest::StartDrain(status) => {
            println!("quit draining via={via} revision={}", status.revision);
            start_drain(app, status);
            return false;
        }
        QuitRequest::DrainInProgress => return false,
        QuitRequest::FlushStore => {}
    }
    let Err(pending) = flush_before_quit(app, via) else {
        return true;
    };
    match storage.quit().refuse(&pending) {
        Some(status) => {
            publish_quit(app, &status);
            false
        }
        None => {
            println!("quit allowed via={via} confirmed=true");
            true
        }
    }
}

fn publish_quit<R: Runtime>(app: &AppHandle<R>, status: &QuitStatus) {
    let _ = QuitStatusChanged(status.clone()).emit(app);
}

fn start_drain<R: Runtime>(app: &AppHandle<R>, status: QuitStatus) {
    let revision = status.revision;
    if QuitStatusChanged(status).emit(app).is_err() {
        let pending = app.state::<Storage>().pending_docs();
        drain_timed_out(app, revision, &pending);
        return;
    }
    let timeout = app.state::<Storage>().quit().drain_timeout();
    let handle = app.clone();
    let timer = std::thread::Builder::new()
        .name("aelomi-quit-drain".into())
        .spawn(move || {
            std::thread::sleep(timeout);
            if !handle.state::<Storage>().quit().awaiting_drain(revision) {
                return;
            }
            let pending = flush_before_quit(&handle, "DrainTimeout")
                .err()
                .unwrap_or_default();
            drain_timed_out(&handle, revision, &pending);
        });
    if timer.is_err() {
        let pending = app.state::<Storage>().pending_docs();
        drain_timed_out(app, revision, &pending);
    }
}

fn drain_timed_out<R: Runtime>(app: &AppHandle<R>, revision: u32, pending: &[DocId]) {
    if let Some(status) = app
        .state::<Storage>()
        .quit()
        .drain_timed_out(revision, pending)
    {
        println!("quit drain timed out revision={revision}");
        publish_quit(app, &status);
    }
}

fn flush_before_quit<R: Runtime>(app: &AppHandle<R>, via: &str) -> Result<(), Vec<DocId>> {
    let Some(storage) = app.try_state::<Storage>() else {
        return Ok(());
    };
    let result = storage.flush_before_quit(QUIT_FLUSH_TIMEOUT);
    println!(
        "quit flush via={via} pending={:?}",
        result.as_ref().err().map_or(&[][..], Vec::as_slice)
    );
    result
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread;
    use std::time::{Duration, Instant};

    use aelomi_core::storage::test_support::SimDisk;
    use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
    use tauri::{App, Listener, WebviewUrl, WebviewWindowBuilder};

    use super::*;
    use crate::quit::QuitPhase;

    pub(crate) type Seen = Arc<Mutex<Vec<String>>>;

    pub(crate) struct BlockedApp {
        pub app: App<MockRuntime>,
        pub quit_events: Seen,
        pub quit_changed: mpsc::Receiver<()>,
    }

    pub(crate) fn app_with_page_pending(disk: &SimDisk) -> BlockedApp {
        let app = app_storing_page(disk);
        let (quit_events, quit_changed) = record_quit_status(&app);
        BlockedApp {
            app,
            quit_events,
            quit_changed,
        }
    }

    fn app_storing_page(disk: &SimDisk) -> App<MockRuntime> {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("builds the mock app");
        tauri_specta::Builder::<MockRuntime>::new()
            .events(tauri_specta::collect_events![QuitStatusChanged])
            .mount_events(&app);
        let storage = storage::tests::open_on(disk);
        disk.fail_appends("docs/page/");
        storage
            .workspace()
            .unwrap()
            .append(
                &DocId::new("page").unwrap(),
                &storage::tests::update("unsaved"),
            )
            .unwrap();
        app.manage(storage);
        app
    }

    fn app_with_webview(disk: &SimDisk, drain_timeout: Duration) -> BlockedApp {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("builds the mock app");
        tauri_specta::Builder::<MockRuntime>::new()
            .events(tauri_specta::collect_events![QuitStatusChanged])
            .mount_events(&app);
        let storage =
            storage::tests::open_on_with(disk, quit::QuitGate::with_drain_timeout(drain_timeout));
        storage.quit().register_frontend();
        app.manage(storage);
        let (quit_events, quit_changed) = record_quit_status(&app);
        BlockedApp {
            app,
            quit_events,
            quit_changed,
        }
    }

    fn status(revision: u32, phase: QuitPhase) -> QuitStatus {
        QuitStatus { revision, phase }
    }

    fn asking(revision: u32, docs: &[&str]) -> QuitStatus {
        status(
            revision,
            QuitPhase::Asking {
                pending_docs: docs.iter().map(|d| d.to_string()).collect(),
            },
        )
    }

    #[test]
    fn a_registered_webview_drains_every_route_before_the_app_exits() {
        let disk = SimDisk::default();
        let BlockedApp {
            app, quit_events, ..
        } = app_with_webview(&disk, Duration::from_secs(60));
        let page = DocId::new("page").unwrap();
        let storage = app.state::<Storage>();

        for via in ["CloseRequested", "ExitRequested", "Terminate"] {
            let started = Instant::now();
            assert!(!flush_then_allow_quit(app.handle(), via), "{via}");
            assert!(
                started.elapsed() < Duration::from_millis(500),
                "{via} never blocks"
            );
        }
        assert_eq!(
            commands::quit_status(app.state()),
            status(1, QuitPhase::Draining)
        );
        storage
            .workspace()
            .unwrap()
            .append(&page, &storage::tests::update("typed then quit"))
            .unwrap();
        let mut exits = 0;
        assert_eq!(
            commands::complete_quit_with(app.handle(), 0, || exits += 1),
            status(1, QuitPhase::Draining),
            "a stale completion changes nothing"
        );
        assert_eq!(
            commands::complete_quit_with(app.handle(), 1, || exits += 1),
            status(2, QuitPhase::Confirmed)
        );
        assert_eq!(
            commands::complete_quit_with(app.handle(), 1, || exits += 1),
            status(2, QuitPhase::Confirmed)
        );
        assert_eq!(exits, 1);
        assert!(flush_then_allow_quit(app.handle(), "ExitRequested"));
        assert_eq!(
            *quit_events.lock().unwrap(),
            [
                r#"quit-status-changed {"revision":1,"phase":"Draining"}"#,
                r#"quit-status-changed {"revision":2,"phase":"Confirmed"}"#,
            ]
        );
        disk.crash();
        let reopened = disk.open(std::path::Path::new("/data")).unwrap();
        assert_eq!(
            storage::tests::text_of(&reopened.load(&page).unwrap()),
            "typed then quit"
        );
    }

    #[test]
    fn a_drain_with_unsaved_docs_asks_and_keep_open_reopens_editing() {
        let disk = SimDisk::default();
        let BlockedApp {
            app, quit_events, ..
        } = app_with_webview(&disk, Duration::from_secs(60));
        let storage = app.state::<Storage>();
        disk.fail_appends("docs/page/");
        storage
            .workspace()
            .unwrap()
            .append(
                &DocId::new("page").unwrap(),
                &storage::tests::update("unsaved"),
            )
            .unwrap();

        assert!(!flush_then_allow_quit(app.handle(), "ExitRequested"));
        let mut exits = 0;
        assert_eq!(
            commands::complete_quit_with(app.handle(), 1, || exits += 1),
            asking(2, &["page"])
        );
        assert_eq!(exits, 0);
        assert!(!flush_then_allow_quit(app.handle(), "ExitRequested"));
        assert_eq!(
            commands::quit_status(app.state()),
            status(3, QuitPhase::Draining),
            "quitting again while asked drains again"
        );
        assert_eq!(
            commands::quit_drain_failed_with(app.handle(), 3, &[DocId::new("other").unwrap()]),
            asking(4, &["other"])
        );
        assert_eq!(
            commands::quit_drain_failed_with(app.handle(), 3, &[]),
            asking(4, &["other"]),
            "a stale failure changes nothing"
        );
        assert_eq!(
            commands::keep_open_with(app.handle(), 4),
            status(5, QuitPhase::Open)
        );
        commands::quit_anyway_with(app.handle(), || exits += 1);
        assert_eq!(exits, 1);
        assert_eq!(
            *quit_events.lock().unwrap(),
            [
                r#"quit-status-changed {"revision":1,"phase":"Draining"}"#,
                r#"quit-status-changed {"revision":2,"phase":{"Asking":{"pending_docs":["page"]}}}"#,
                r#"quit-status-changed {"revision":3,"phase":"Draining"}"#,
                r#"quit-status-changed {"revision":4,"phase":{"Asking":{"pending_docs":["other"]}}}"#,
                r#"quit-status-changed {"revision":5,"phase":"Open"}"#,
                r#"quit-status-changed {"revision":6,"phase":"Confirmed"}"#,
            ]
        );
    }

    #[test]
    fn a_drain_the_webview_never_finishes_asks_after_the_timeout() {
        let disk = SimDisk::default();
        let BlockedApp {
            app,
            quit_events,
            quit_changed,
        } = app_with_webview(&disk, Duration::from_millis(50));

        assert!(!flush_then_allow_quit(app.handle(), "CloseRequested"));
        quit_changed.recv_timeout(Duration::from_secs(10)).unwrap();
        quit_changed.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(commands::quit_status(app.state()), asking(2, &[]));
        let mut exits = 0;
        assert_eq!(
            commands::complete_quit_with(app.handle(), 1, || exits += 1),
            asking(2, &[]),
            "a completion after the timeout cannot quit"
        );
        assert_eq!(exits, 0);
        assert_eq!(
            *quit_events.lock().unwrap(),
            [
                r#"quit-status-changed {"revision":1,"phase":"Draining"}"#,
                r#"quit-status-changed {"revision":2,"phase":{"Asking":{"pending_docs":[]}}}"#,
            ]
        );
    }

    #[test]
    fn an_accepted_completion_is_not_cut_short_by_the_timeout() {
        let disk = SimDisk::default();
        let BlockedApp { app, .. } = app_with_webview(&disk, Duration::from_millis(50));
        disk.set_full_sync_latency(Duration::from_millis(300));
        app.state::<Storage>()
            .workspace()
            .unwrap()
            .append(
                &DocId::new("page").unwrap(),
                &storage::tests::update("slow disk"),
            )
            .unwrap();

        assert!(!flush_then_allow_quit(app.handle(), "ExitRequested"));
        let mut exits = 0;
        assert_eq!(
            commands::complete_quit_with(app.handle(), 1, || exits += 1),
            status(2, QuitPhase::Confirmed)
        );
        assert_eq!(exits, 1);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(
            commands::quit_status(app.state()),
            status(2, QuitPhase::Confirmed)
        );
    }

    fn record_quit_status(app: &App<MockRuntime>) -> (Seen, mpsc::Receiver<()>) {
        let quit_events = Seen::default();
        let (changed_tx, quit_changed) = mpsc::channel();
        let record = quit_events.clone();
        app.listen_any("quit-status-changed", move |event| {
            record
                .lock()
                .unwrap()
                .push(format!("quit-status-changed {}", event.payload()));
            let _ = changed_tx.send(());
        });
        (quit_events, quit_changed)
    }

    /// `ready` waits for the loop to start, because the mock runtime removes a window closed
    /// before then without sending `CloseRequested`.
    fn run_recording(
        app: App<MockRuntime>,
        quit_events: Seen,
        ready: impl FnOnce(&AppHandle<MockRuntime>) + 'static,
    ) {
        let started = Instant::now();
        let mut ready = Some(ready);
        app.run(move |app, event| {
            let name = match &event {
                RunEvent::Ready => {
                    ready.take().unwrap()(app);
                    None
                }
                RunEvent::WindowEvent {
                    event: WindowEvent::CloseRequested { .. },
                    ..
                } => Some("CloseRequested"),
                RunEvent::ExitRequested { .. } => Some("ExitRequested"),
                RunEvent::Exit => Some("Exit"),
                _ => None,
            };
            if let Some(name) = name {
                quit_events.lock().unwrap().push(name.to_owned());
            }
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the app never quit: {:?}",
                quit_events.lock().unwrap()
            );
            on_run_event(app, event);
        });
    }

    #[test]
    fn closing_the_last_window_waits_until_pending_updates_are_written() {
        let disk = Arc::new(SimDisk::default());
        let BlockedApp {
            app,
            quit_events,
            quit_changed,
        } = app_with_page_pending(&disk);
        WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();

        run_recording(app, quit_events.clone(), move |app| {
            let window = app.get_webview_window("main").unwrap();
            thread::spawn(move || {
                window.close().unwrap();
                let _ = quit_changed.recv_timeout(Duration::from_secs(10));
                disk.clear_faults();
                window.close().unwrap();
            });
        });

        assert_eq!(
            *quit_events.lock().unwrap(),
            [
                "CloseRequested",
                r#"quit-status-changed {"revision":1,"phase":{"Asking":{"pending_docs":["page"]}}}"#,
                "CloseRequested",
                "ExitRequested",
                "Exit",
            ]
        );
    }

    #[test]
    fn exiting_waits_until_pending_updates_are_written() {
        let disk = Arc::new(SimDisk::default());
        let BlockedApp {
            app,
            quit_events,
            quit_changed,
        } = app_with_page_pending(&disk);
        WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();

        run_recording(app, quit_events.clone(), move |app| {
            let app = app.clone();
            thread::spawn(move || {
                app.get_webview_window("main").unwrap().destroy().unwrap();
                let _ = quit_changed.recv_timeout(Duration::from_secs(10));
                disk.clear_faults();
                let (built_tx, built) = mpsc::channel();
                let handle = app.clone();
                app.run_on_main_thread(move || {
                    let window = WebviewWindowBuilder::new(&handle, "again", WebviewUrl::default())
                        .build()
                        .unwrap();
                    built_tx.send(window).unwrap();
                })
                .unwrap();
                built.recv().unwrap().destroy().unwrap();
            });
        });

        assert_eq!(
            *quit_events.lock().unwrap(),
            [
                "ExitRequested",
                r#"quit-status-changed {"revision":1,"phase":{"Asking":{"pending_docs":["page"]}}}"#,
                "ExitRequested",
                "Exit",
            ]
        );
    }

    #[test]
    fn confirmed_quit_bypasses_close_and_exit_holds_while_writes_still_fail() {
        let disk = Arc::new(SimDisk::default());
        let BlockedApp {
            app,
            quit_events,
            quit_changed,
        } = app_with_page_pending(&disk);
        WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();

        let worker = Arc::new(Mutex::new(None));
        let worker_handle = worker.clone();
        run_recording(app, quit_events.clone(), move |app| {
            let app = app.clone();
            *worker_handle.lock().unwrap() = Some(thread::spawn(move || {
                let window = app.get_webview_window("main").unwrap();
                window.close().unwrap();
                quit_changed.recv_timeout(Duration::from_secs(10)).unwrap();
                let mut exits = 0;
                commands::quit_anyway_with(&app, || {
                    exits += 1;
                    window.close().unwrap();
                });
                commands::quit_anyway_with(&app, || exits += 1);
                assert_eq!(exits, 1);
                assert_eq!(
                    commands::keep_open_with(&app, 1),
                    QuitStatus {
                        revision: 2,
                        phase: QuitPhase::Confirmed
                    }
                );
                let storage = app.state::<Storage>();
                assert_eq!(
                    storage::tests::text_of(
                        &storage
                            .workspace()
                            .unwrap()
                            .load(&DocId::new("page").unwrap())
                            .unwrap()
                    ),
                    "unsaved"
                );
            }));
        });
        worker.lock().unwrap().take().unwrap().join().unwrap();

        assert_eq!(
            *quit_events.lock().unwrap(),
            [
                "CloseRequested",
                r#"quit-status-changed {"revision":1,"phase":{"Asking":{"pending_docs":["page"]}}}"#,
                r#"quit-status-changed {"revision":2,"phase":"Confirmed"}"#,
                "CloseRequested",
                "ExitRequested",
                "Exit",
            ]
        );
    }

    #[test]
    fn a_refusal_without_a_listener_stays_outstanding_until_kept_open() {
        let disk = SimDisk::default();
        let app = app_storing_page(&disk);
        let asked = QuitStatus {
            revision: 1,
            phase: QuitPhase::Asking {
                pending_docs: vec!["page".into()],
            },
        };

        assert!(!flush_then_allow_quit(app.handle(), "ExitRequested"));
        assert_eq!(commands::quit_status(app.state()), asked);
        disk.clear_faults();
        let storage = app.state::<Storage>();
        assert_eq!(storage.flush_before_quit(QUIT_FLUSH_TIMEOUT), Ok(()));
        assert_eq!(storage.health(), storage::Health::Healthy);
        assert_eq!(commands::quit_status(app.state()), asked);

        let (late_listener, _) = record_quit_status(&app);
        let kept = QuitStatus {
            revision: 2,
            phase: QuitPhase::Open,
        };
        assert_eq!(commands::keep_open_with(app.handle(), 1), kept);
        assert_eq!(commands::keep_open_with(app.handle(), 1), kept);
        assert_eq!(commands::quit_status(app.state()), kept);

        disk.fail_appends("docs/page/");
        storage
            .workspace()
            .unwrap()
            .append(
                &DocId::new("page").unwrap(),
                &storage::tests::update(" again"),
            )
            .unwrap();
        assert!(!flush_then_allow_quit(app.handle(), "Terminate"));
        assert_eq!(
            commands::quit_status(app.state()),
            QuitStatus {
                revision: 3,
                phase: QuitPhase::Asking {
                    pending_docs: vec!["page".into()],
                },
            }
        );
        assert_eq!(
            *late_listener.lock().unwrap(),
            [
                r#"quit-status-changed {"revision":2,"phase":"Open"}"#,
                r#"quit-status-changed {"revision":3,"phase":{"Asking":{"pending_docs":["page"]}}}"#,
            ]
        );
    }

    #[test]
    fn final_exit_flush_saves_recovered_writes_and_appends_after_confirmation() {
        use std::path::Path;
        use yrs::updates::decoder::Decode;
        use yrs::{Text, Transact, Update};

        let disk = SimDisk::default();
        let BlockedApp {
            app, quit_events, ..
        } = app_with_page_pending(&disk);
        assert!(!flush_then_allow_quit(app.handle(), "CloseRequested"));
        let storage = app.state::<Storage>();
        let mut exits = 0;
        commands::quit_anyway_with(app.handle(), || exits += 1);
        assert_eq!(exits, 1);
        assert!(flush_then_allow_quit(app.handle(), "ExitRequested"));
        assert!(flush_then_allow_quit(app.handle(), "CloseRequested"));

        let id = DocId::new("page").unwrap();
        let doc = yrs::Doc::new();
        let body = doc.get_or_insert_text("body");
        let mut txn = doc.transact_mut();
        txn.apply_update(
            Update::decode_v1(&storage.workspace().unwrap().load(&id).unwrap()).unwrap(),
        )
        .unwrap();
        body.insert(&mut txn, 7, " after confirmation");
        storage
            .workspace()
            .unwrap()
            .append(&id, &txn.encode_update_v1())
            .unwrap();
        drop(txn);
        disk.clear_faults();
        on_run_event(app.handle(), RunEvent::Exit);
        disk.crash();
        let reopened = disk.open(Path::new("/data")).unwrap();
        assert_eq!(
            storage::tests::text_of(&reopened.load(&id).unwrap()),
            "unsaved after confirmation"
        );
        assert_eq!(
            *quit_events.lock().unwrap(),
            [
                r#"quit-status-changed {"revision":1,"phase":{"Asking":{"pending_docs":["page"]}}}"#,
                r#"quit-status-changed {"revision":2,"phase":"Confirmed"}"#,
            ]
        );
    }
}
