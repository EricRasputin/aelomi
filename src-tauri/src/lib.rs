mod commands;
#[cfg(target_os = "macos")]
mod macos;
mod perf;
mod quit;
mod storage;

use aelomi_core::storage::DocId;
use quit::{QuitStatus, QuitStatusChanged};
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
    let storage = Storage::open(&location);
    println!("{}", storage.startup_line());
    if let Ok(store) = storage.store() {
        let app = app.handle().clone();
        store.subscribe_health(move |health| {
            let _ = StorageHealthChanged(health.into()).emit(&app);
        });
    }
    app.manage(storage);
}

fn flush_then_allow_quit<R: Runtime>(app: &AppHandle<R>, via: &str) -> bool {
    let Some(storage) = app.try_state::<Storage>() else {
        return true;
    };
    if storage.quit().confirmed() {
        println!("quit allowed via={via} confirmed=true");
        return true;
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
            .store()
            .unwrap()
            .append(
                &DocId::new("page").unwrap(),
                &storage::tests::update("unsaved"),
            )
            .unwrap();
        app.manage(storage);
        app
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
                r#"quit-status-changed {"revision":1,"pending_docs":["page"]}"#,
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
                r#"quit-status-changed {"revision":1,"pending_docs":["page"]}"#,
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
                        pending_docs: None
                    }
                );
                let storage = app.state::<Storage>();
                assert_eq!(
                    storage::tests::text_of(
                        &storage
                            .store()
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
                r#"quit-status-changed {"revision":1,"pending_docs":["page"]}"#,
                r#"quit-status-changed {"revision":2,"pending_docs":null}"#,
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
            pending_docs: Some(vec!["page".into()]),
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
            pending_docs: None,
        };
        assert_eq!(commands::keep_open_with(app.handle(), 1), kept);
        assert_eq!(commands::keep_open_with(app.handle(), 1), kept);
        assert_eq!(commands::quit_status(app.state()), kept);

        disk.fail_appends("docs/page/");
        storage
            .store()
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
                pending_docs: Some(vec!["page".into()]),
            }
        );
        assert_eq!(
            *late_listener.lock().unwrap(),
            [
                r#"quit-status-changed {"revision":2,"pending_docs":null}"#,
                r#"quit-status-changed {"revision":3,"pending_docs":["page"]}"#,
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
        txn.apply_update(Update::decode_v1(&storage.store().unwrap().load(&id).unwrap()).unwrap())
            .unwrap();
        body.insert(&mut txn, 7, " after confirmation");
        storage
            .store()
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
                r#"quit-status-changed {"revision":1,"pending_docs":["page"]}"#,
                r#"quit-status-changed {"revision":2,"pending_docs":null}"#,
            ]
        );
    }
}
