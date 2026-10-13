mod commands;
#[cfg(target_os = "macos")]
mod macos;
mod perf;
mod storage;

use aelomi_core::storage::DocId;
use storage::{QUIT_FLUSH_TIMEOUT, QuitBlocked, Storage, StorageHealthChanged};
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
    if app
        .try_state::<Storage>()
        .is_some_and(|storage| storage.quit_confirmed())
    {
        println!("quit allowed via={via} confirmed=true");
        return true;
    }
    let Err(pending) = flush_before_quit(app, via) else {
        return true;
    };
    let pending_docs = pending.iter().map(DocId::to_string).collect();
    let _ = QuitBlocked { pending_docs }.emit(app);
    false
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
        pub quit_blocked: mpsc::Receiver<()>,
    }

    pub(crate) fn app_with_page_pending(disk: &SimDisk) -> BlockedApp {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("builds the mock app");
        tauri_specta::Builder::<MockRuntime>::new()
            .events(tauri_specta::collect_events![QuitBlocked])
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
        let quit_events = Seen::default();
        let (blocked_tx, quit_blocked) = mpsc::channel();
        let record = quit_events.clone();
        app.listen_any("quit-blocked", move |event| {
            record
                .lock()
                .unwrap()
                .push(format!("quit-blocked {}", event.payload()));
            let _ = blocked_tx.send(());
        });
        BlockedApp {
            app,
            quit_events,
            quit_blocked,
        }
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
            quit_blocked,
        } = app_with_page_pending(&disk);
        WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();

        run_recording(app, quit_events.clone(), move |app| {
            let window = app.get_webview_window("main").unwrap();
            thread::spawn(move || {
                window.close().unwrap();
                let _ = quit_blocked.recv_timeout(Duration::from_secs(10));
                disk.clear_faults();
                window.close().unwrap();
            });
        });

        assert_eq!(
            *quit_events.lock().unwrap(),
            [
                "CloseRequested",
                r#"quit-blocked {"pending_docs":["page"]}"#,
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
            quit_blocked,
        } = app_with_page_pending(&disk);
        WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();

        run_recording(app, quit_events.clone(), move |app| {
            let app = app.clone();
            thread::spawn(move || {
                app.get_webview_window("main").unwrap().destroy().unwrap();
                let _ = quit_blocked.recv_timeout(Duration::from_secs(10));
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
                r#"quit-blocked {"pending_docs":["page"]}"#,
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
            quit_blocked,
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
                quit_blocked.recv_timeout(Duration::from_secs(10)).unwrap();
                let storage = app.state::<Storage>();
                let mut exits = 0;
                commands::quit_anyway_with(&storage, || {
                    exits += 1;
                    window.close().unwrap();
                });
                commands::quit_anyway_with(&storage, || exits += 1);
                assert_eq!(exits, 1);
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
                r#"quit-blocked {"pending_docs":["page"]}"#,
                "CloseRequested",
                "ExitRequested",
                "Exit",
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
        commands::quit_anyway_with(&storage, || exits += 1);
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
            [r#"quit-blocked {"pending_docs":["page"]}"#]
        );
    }
}
