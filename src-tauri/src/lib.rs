mod commands;
mod external_links;
mod index;
#[cfg(target_os = "macos")]
mod macos;
mod perf;
mod quit;
mod recovery;
mod settings;
mod storage;
mod trace;

#[cfg(target_os = "macos")]
use macos::PACKAGED_APP_ORIGIN;
#[cfg(not(target_os = "macos"))]
const PACKAGED_APP_ORIGIN: &str = "tauri://localhost";

use aelomi_core::index::IndexEvent;
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
        .plugin(external_links::guard())
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
    #[cfg(target_os = "macos")]
    let record = macos::recovery_file();
    #[cfg(not(target_os = "macos"))]
    let record = app
        .path()
        .app_config_dir()
        .unwrap_or_default()
        .join("recovery.json");
    let storage = Storage::open_with_recovery(&location, record, move |event| {
        publish_index(&handle, event)
    });
    println!("{}", storage.startup_line());
    println!(
        "storage open ms={} t_ms={:.1}",
        trace::ms(started.elapsed()),
        trace::since_start_ms()
    );
    if let Ok(workspace) = storage.workspace() {
        prepare_workspace(app.handle(), workspace);
    }
    app.manage(storage);
}

pub(crate) fn prepare_workspace<R: Runtime>(
    app: &AppHandle<R>,
    workspace: std::sync::Arc<aelomi_core::workspace::Workspace>,
) {
    let settings_workspace = workspace.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = settings_workspace.settings();
    });
    warm_tree(workspace.clone());
    let app = app.clone();
    workspace.subscribe_health(move |health| {
        let _ = StorageHealthChanged(health.into()).emit(&app);
    });
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
    match storage.quit().refuse(&pending.docs, pending.settings) {
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
        let pending = app.state::<Storage>().pending_edits();
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
        let pending = app.state::<Storage>().pending_edits();
        drain_timed_out(app, revision, &pending);
    }
}

fn drain_timed_out<R: Runtime>(
    app: &AppHandle<R>,
    revision: u32,
    pending: &aelomi_core::storage::Pending,
) {
    if let Some(status) =
        app.state::<Storage>()
            .quit()
            .drain_timed_out(revision, &pending.docs, pending.settings)
    {
        println!("quit drain timed out revision={revision}");
        publish_quit(app, &status);
    }
}

fn flush_before_quit<R: Runtime>(
    app: &AppHandle<R>,
    via: &str,
) -> Result<(), aelomi_core::storage::Pending> {
    let Some(storage) = app.try_state::<Storage>() else {
        return Ok(());
    };
    let result = storage.flush_before_quit(QUIT_FLUSH_TIMEOUT);
    println!("quit flush via={via} pending={:?}", result.as_ref().err());
    result
}

#[cfg(test)]
pub(crate) mod tests;
