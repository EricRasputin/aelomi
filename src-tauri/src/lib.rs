mod commands;
#[cfg(target_os = "macos")]
mod macos;
mod perf;
mod storage;

use aelomi_core::storage::DocId;
use storage::{QUIT_FLUSH_TIMEOUT, QuitBlocked, Storage, StorageHealthChanged};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
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
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && !flush_then_allow_quit(window.app_handle(), "CloseRequested")
            {
                api.prevent_close();
            }
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
        .run(|app, event| match event {
            RunEvent::ExitRequested { api, .. } => {
                if !flush_then_allow_quit(app, "ExitRequested") {
                    api.prevent_exit();
                }
            }
            // Logout and the Dock's Quit end here, where the exit can no longer be stopped.
            RunEvent::Exit => {
                let _ = flush_before_quit(app, "Exit");
            }
            _ => {}
        });
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

fn flush_then_allow_quit(app: &AppHandle, via: &str) -> bool {
    let Err(pending) = flush_before_quit(app, via) else {
        return true;
    };
    let pending_docs = pending.iter().map(DocId::to_string).collect();
    let _ = QuitBlocked { pending_docs }.emit(app);
    false
}

fn flush_before_quit(app: &AppHandle, via: &str) -> Result<(), Vec<DocId>> {
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
