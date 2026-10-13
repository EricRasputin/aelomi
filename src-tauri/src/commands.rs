use aelomi_core::storage::DocId;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::index::{self, IndexChanged, IndexStatusChanged};
use crate::quit::{QuitStatus, QuitStatusChanged};
use crate::storage::{Health, Storage, StorageError, StorageHealthChanged, StorageStatus};

#[derive(Deserialize, Type)]
pub struct PingRequest {
    pub seq: u32,
}

#[derive(Serialize, Type)]
pub struct PingReply {
    pub seq: u32,
    pub core_version: String,
}

#[tauri::command]
#[specta::specta]
pub fn ping(request: PingRequest) -> PingReply {
    println!("ping seq={}", request.seq);
    PingReply {
        seq: request.seq,
        core_version: aelomi_core::VERSION.to_owned(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn perf_report(app: tauri::AppHandle, marks: crate::perf::ColdStartMarks) {
    let Some(path) = crate::perf::report_path() else {
        return;
    };
    match crate::perf::write_report(&path, marks) {
        Ok(()) => app.exit(0),
        Err(error) => {
            eprintln!("perf report: {error}");
            app.exit(1);
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn storage_status(storage: State<'_, Storage>) -> StorageStatus {
    storage.status()
}

#[tauri::command]
#[specta::specta]
pub fn storage_health(storage: State<'_, Storage>) -> Health {
    storage.health()
}

/// The quit decision as it stands, for a webview that starts listening after a refusal.
#[tauri::command]
#[specta::specta]
pub fn quit_status(storage: State<'_, Storage>) -> QuitStatus {
    storage.quit().status()
}

/// Keep app open answers the refused quit asked at `revision`. Returns the decision after the
/// call.
#[tauri::command]
#[specta::specta]
pub fn keep_open(app: tauri::AppHandle, revision: u32) -> QuitStatus {
    keep_open_with(&app, revision)
}

pub(crate) fn keep_open_with<R: Runtime>(app: &AppHandle<R>, revision: u32) -> QuitStatus {
    let storage = app.state::<Storage>();
    match storage.quit().keep_open(revision) {
        Some(status) => {
            crate::publish_quit(app, &status);
            status
        }
        None => storage.quit().status(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn quit_anyway(app: tauri::AppHandle) {
    quit_anyway_with(&app, || app.exit(0));
}

pub(crate) fn quit_anyway_with<R: Runtime>(app: &AppHandle<R>, exit: impl FnOnce()) {
    if let Some(status) = app.state::<Storage>().quit().confirm() {
        println!("quit confirmed");
        crate::publish_quit(app, &status);
        exit();
    }
}

/// Returns the doc's full state as base64.
#[tauri::command]
#[specta::specta]
pub fn doc_load(storage: State<'_, Storage>, doc_id: String) -> Result<String, StorageError> {
    let state = storage.workspace()?.load(&DocId::new(doc_id)?)?;
    Ok(BASE64.encode(state))
}

/// Takes the update as base64. Returns its sequence number once it is queued, before it is
/// durable. That is an `f64` because specta refuses `u64`, and a JS number holds every value
/// below 2^53 exactly.
#[tauri::command]
#[specta::specta]
pub fn doc_append(
    storage: State<'_, Storage>,
    doc_id: String,
    update: String,
) -> Result<f64, StorageError> {
    let update = BASE64
        .decode(update)
        .map_err(|_| StorageError::InvalidUpdate)?;
    let seq = storage.workspace()?.append(&DocId::new(doc_id)?, &update)?;
    Ok(seq.0 as f64)
}

#[tauri::command]
#[specta::specta]
pub fn dev_probe_log(line: String) {
    println!("{line}");
}

pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
        .commands(tauri_specta::collect_commands![
            ping,
            perf_report,
            storage_status,
            storage_health,
            quit_status,
            keep_open,
            quit_anyway,
            doc_load,
            doc_append,
            index::index_status,
            index::page_tree,
            index::search,
            index::backlinks,
            dev_probe_log,
        ])
        .events(tauri_specta::collect_events![
            StorageHealthChanged,
            QuitStatusChanged,
            IndexStatusChanged,
            IndexChanged
        ])
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    #[test]
    fn ping_echoes_sequence_and_core_version() {
        let reply = super::ping(super::PingRequest { seq: 42 });
        assert_eq!(reply.seq, 42);
        assert_eq!(reply.core_version, aelomi_core::VERSION);
    }

    #[test]
    fn bindings_are_current() {
        let scratch =
            std::env::temp_dir().join(format!("aelomi-bindings-{}.ts", std::process::id()));
        super::builder()
            .export(specta_typescript::Typescript::default(), &scratch)
            .expect("generate bindings");
        let generated = format!(
            "{}\n",
            fs::read_to_string(&scratch)
                .expect("read generated bindings")
                .trim_end()
        );
        fs::remove_file(&scratch).expect("remove generated bindings");
        let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/ipc/bindings.ts");
        if std::env::var_os("AELOMI_WRITE_BINDINGS").is_some_and(|value| value == "1") {
            fs::write(&committed, &generated).expect("write bindings");
        }
        assert_eq!(
            fs::read_to_string(committed).expect("read committed bindings; run pnpm bindings"),
            generated,
            "IPC bindings changed; run pnpm bindings and check the TypeScript callers"
        );
    }
}
