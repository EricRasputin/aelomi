use std::time::Instant;

use aelomi_core::storage::DocId;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::index::{self, IndexChanged, IndexStatusChanged};
use crate::quit::{QuitPhase, QuitStatus, QuitStatusChanged};
use crate::storage::{Health, Storage, StorageError, StorageHealthChanged, StorageStatus};
use crate::trace::IpcMeasurements;

#[derive(Clone, Serialize, Type, tauri_specta::Event)]
pub struct MenuCommand {
    pub command: String,
}

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

#[tauri::command]
#[specta::specta]
pub fn show_recovery_folder(storage: State<'_, Storage>) -> Result<(), StorageError> {
    show_recovery_folder_with(&storage, |path| {
        tauri_plugin_opener::open_path(path, None::<&str>).map_err(|error| error.to_string())
    })?;
    println!("recovery folder opener launched=true");
    Ok(())
}

fn show_recovery_folder_with(
    storage: &Storage,
    opener: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), StorageError> {
    opener(&storage.recovery_folder()?).map_err(StorageError::Io)
}

#[tauri::command]
#[specta::specta]
pub async fn start_empty_workspace(app: AppHandle) -> Result<StorageStatus, StorageError> {
    tauri::async_runtime::spawn_blocking(move || {
        let handle = app.clone();
        let storage = app.state::<Storage>();
        let status =
            storage.start_empty_workspace(move |event| crate::publish_index(&handle, event))?;
        if let Ok(workspace) = storage.workspace() {
            crate::prepare_workspace(&app, workspace);
            storage.register_frontend();
        }
        println!("recovery new workspace status={status:?}");
        Ok(status)
    })
    .await
    .map_err(|error| StorageError::Io(error.to_string()))?
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

/// The webview listens for quit status and holds the editor. Call it after the quit listener
/// is registered and the quit status read, before editing is enabled. From then on every quit
/// first waits for the webview to deliver its edits (`Draining`) when the workspace is open.
#[tauri::command]
#[specta::specta]
pub fn frontend_ready(storage: State<'_, Storage>) {
    storage.register_frontend();
}

/// The webview delivered every edit of the drain at `revision`. Flushes the store: with
/// nothing pending the quit is confirmed and the app exits, else the user is asked. A stale
/// `revision` changes nothing. Returns the decision after the call.
#[tauri::command]
#[specta::specta]
pub async fn complete_quit(app: tauri::AppHandle, revision: u32) -> QuitStatus {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        complete_quit_with(&handle, revision, || handle.exit(0))
    })
    .await
    .unwrap_or_else(|_| app.state::<Storage>().quit().status())
}

pub(crate) fn complete_quit_with<R: Runtime>(
    app: &AppHandle<R>,
    revision: u32,
    exit: impl FnOnce(),
) -> QuitStatus {
    let storage = app.state::<Storage>();
    if !storage.quit().begin_completing(revision) {
        return storage.quit().status();
    }
    let pending = crate::flush_before_quit(app, "CompleteQuit")
        .err()
        .unwrap_or_default();
    let Some(status) = storage
        .quit()
        .finish_drain(revision, &pending.docs, pending.settings)
    else {
        return storage.quit().status();
    };
    crate::publish_quit(app, &status);
    if status.phase == QuitPhase::Confirmed {
        println!("quit confirmed via=drain");
        exit();
    }
    status
}

/// The webview could not deliver its edits to `doc_ids` during the drain at `revision`, so the
/// user is asked. A stale `revision` changes nothing. Returns the decision after the call.
#[tauri::command]
#[specta::specta]
pub fn quit_drain_failed(
    app: tauri::AppHandle,
    revision: u32,
    doc_ids: Vec<String>,
) -> Result<QuitStatus, StorageError> {
    let docs = doc_ids
        .into_iter()
        .map(DocId::new)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quit_drain_failed_with(&app, revision, &docs))
}

pub(crate) fn quit_drain_failed_with<R: Runtime>(
    app: &AppHandle<R>,
    revision: u32,
    docs: &[DocId],
) -> QuitStatus {
    let storage = app.state::<Storage>();
    let mut pending = storage.pending_edits();
    pending.docs.extend_from_slice(docs);
    pending.docs.sort();
    pending.docs.dedup();
    match storage
        .quit()
        .drain_failed(revision, &pending.docs, pending.settings)
    {
        Some(status) => {
            println!("quit drain failed pending={pending:?}");
            crate::publish_quit(app, &status);
            status
        }
        None => storage.quit().status(),
    }
}

/// Returns the doc's full state as base64. Loads off the main thread.
#[tauri::command]
#[specta::specta]
pub async fn doc_load(storage: State<'_, Storage>, doc_id: String) -> Result<String, StorageError> {
    let workspace = storage.workspace()?;
    let doc = DocId::new(doc_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let state = BASE64.encode(workspace.load(&doc)?);
        crate::trace::ipc(started, || IpcMeasurements::DocLoad {
            base64_bytes: state.len(),
        });
        Ok(state)
    })
    .await
    .map_err(|e| StorageError::Io(e.to_string()))?
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
    let started = Instant::now();
    let update = BASE64
        .decode(update)
        .map_err(|_| StorageError::InvalidUpdate)?;
    let seq = storage.workspace()?.append(&DocId::new(doc_id)?, &update)?;
    crate::trace::ipc(started, || IpcMeasurements::DocAppend {
        delta_bytes: update.len(),
    });
    Ok(seq.0 as f64)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum PageError {
    Unavailable,
    Quitting,
    InvalidPageId,
    NotActive,
    AlreadyExists,
    Cycle,
    OrderExhausted,
    InvalidTime,
    InvalidUpdate,
    Malformed { reason: String },
    Io(String),
    Internal(String),
}

#[derive(Clone, Serialize, Type, tauri_specta::Event)]
pub struct PageRecordsChanged;

impl From<aelomi_core::workspace::PageEditError> for PageError {
    fn from(error: aelomi_core::workspace::PageEditError) -> Self {
        use aelomi_core::workspace::PageEditError as E;
        match error {
            E::Io(e) => Self::Io(e.to_string()),
            E::Malformed(e) => Self::Malformed {
                reason: e.to_string(),
            },
            E::NotActive => Self::NotActive,
            E::AlreadyExists => Self::AlreadyExists,
            E::Cycle => Self::Cycle,
            E::OrderExhausted => Self::OrderExhausted,
            E::InvalidTime => Self::InvalidTime,
            E::InvalidUpdate => Self::InvalidUpdate,
        }
    }
}

async fn apply_page_edit<R: Runtime>(
    app: tauri::AppHandle<R>,
    workspace: std::sync::Arc<aelomi_core::workspace::Workspace>,
    edit: aelomi_core::workspace::PageEdit,
) -> Result<aelomi_core::workspace::PageEditReply, PageError> {
    let record_edit = app
        .state::<Storage>()
        .quit()
        .record_edit()
        .ok_or(PageError::Quitting)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _record_edit = record_edit;
        use tauri_specta::Event;
        let reply = workspace.edit_page(edit)?;
        if let Err(error) = PageRecordsChanged.emit(&app) {
            eprintln!("page records event: {error}");
        }
        Ok(reply)
    })
    .await
    .map_err(|e| PageError::Internal(e.to_string()))?
}

fn page_id(id: String) -> Result<aelomi_core::workspace::records::PageId, PageError> {
    aelomi_core::workspace::records::PageId::new(id).map_err(|_| PageError::InvalidPageId)
}

#[tauri::command]
#[specta::specta]
pub async fn page_create<R: Runtime>(
    app: tauri::AppHandle<R>,
    storage: State<'_, Storage>,
    parent_id: Option<String>,
    page_id: Option<String>,
) -> Result<String, PageError> {
    let workspace = storage.workspace().map_err(|_| PageError::Unavailable)?;
    let parent = parent_id.map(self::page_id).transpose()?;
    let edit = match page_id {
        Some(id) => aelomi_core::workspace::PageEdit::CreateWithId {
            page: self::page_id(id)?,
            parent,
            title: String::new(),
        },
        None => aelomi_core::workspace::PageEdit::Create {
            parent,
            title: String::new(),
        },
    };
    let reply = apply_page_edit(app, workspace, edit).await?;
    Ok(reply
        .created_page()
        .expect("create returns its page")
        .to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn page_rename<R: Runtime>(
    app: tauri::AppHandle<R>,
    storage: State<'_, Storage>,
    page_id: String,
    title: String,
) -> Result<(), PageError> {
    let workspace = storage.workspace().map_err(|_| PageError::Unavailable)?;
    apply_page_edit(
        app,
        workspace,
        aelomi_core::workspace::PageEdit::Rename {
            page: self::page_id(page_id)?,
            title,
        },
    )
    .await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn page_trash<R: Runtime>(
    app: tauri::AppHandle<R>,
    storage: State<'_, Storage>,
    page_id: String,
) -> Result<String, PageError> {
    let workspace = storage.workspace().map_err(|_| PageError::Unavailable)?;
    let reply = apply_page_edit(
        app,
        workspace,
        aelomi_core::workspace::PageEdit::Trash {
            page: self::page_id(page_id)?,
        },
    )
    .await?;
    Ok(reply.trash_batch().expect("trash returns its batch"))
}

#[tauri::command]
#[specta::specta]
pub fn dev_probe_log(line: String) {
    println!("{line}");
}

#[tauri::command]
#[specta::specta]
pub async fn settings_get(
    storage: State<'_, Storage>,
) -> Result<crate::settings::Settings, crate::settings::SettingsError> {
    let workspace = storage.workspace()?;
    tauri::async_runtime::spawn_blocking(move || {
        workspace
            .settings()
            .map(|settings| (&settings).into())
            .map_err(Into::into)
    })
    .await
    .map_err(|_| crate::settings::SettingsError::Internal)?
}

#[tauri::command]
#[specta::specta]
pub async fn settings_update(
    storage: State<'_, Storage>,
    patch: crate::settings::SettingsPatch,
) -> Result<crate::settings::Settings, crate::settings::SettingsError> {
    let workspace = storage.workspace()?;
    let patch = aelomi_core::settings::SettingsPatch::try_from(patch)?;
    let update = workspace.prepare_settings_update(patch);
    tauri::async_runtime::spawn_blocking(move || {
        update
            .run()
            .map(|settings| (&settings).into())
            .map_err(Into::into)
    })
    .await
    .map_err(|_| crate::settings::SettingsError::Internal)?
}

#[tauri::command]
#[specta::specta]
pub async fn open_url(url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::external_links::open(&url))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
#[specta::specta]
pub async fn storage_flush(storage: State<'_, Storage>) -> Result<bool, StorageError> {
    let workspace = storage.workspace()?;
    tauri::async_runtime::spawn_blocking(move || {
        workspace
            .flush(Instant::now() + crate::storage::QUIT_FLUSH_TIMEOUT)
            .is_empty()
    })
    .await
    .map_err(|error| StorageError::Io(error.to_string()))
}

pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
        .commands(tauri_specta::collect_commands![
            ping,
            perf_report,
            storage_status,
            storage_health,
            show_recovery_folder,
            start_empty_workspace,
            quit_status,
            keep_open,
            quit_anyway,
            frontend_ready,
            complete_quit,
            quit_drain_failed,
            doc_load,
            doc_append,
            page_create::<tauri::Wry>,
            page_rename::<tauri::Wry>,
            page_trash::<tauri::Wry>,
            index::index_status,
            index::page_tree,
            index::search,
            index::backlinks,
            dev_probe_log,
            settings_get,
            settings_update,
            open_url,
            storage_flush,
        ])
        .events(tauri_specta::collect_events![
            StorageHealthChanged,
            QuitStatusChanged,
            IndexStatusChanged,
            IndexChanged,
            MenuCommand,
            PageRecordsChanged
        ])
        .constant(
            "RECENT_PAGES_LIMIT",
            aelomi_core::settings::RECENT_PAGES_LIMIT,
        )
        .constant("MAX_SEARCH_LIMIT", aelomi_core::index::MAX_LIMIT)
        .constant("MAX_EXCLUDED_PAGES", aelomi_core::index::MAX_EXCLUDED)
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
    fn recovery_flush_waits_for_durability_and_survives_a_crash() {
        use aelomi_core::storage::{DocId, test_support::SimDisk};
        use std::time::Duration;
        use tauri::{
            Manager,
            test::{mock_builder, mock_context, noop_assets},
        };
        let disk = SimDisk::default();
        let storage = crate::storage::tests::open_on(&disk);
        disk.set_full_sync_latency(Duration::from_millis(100));
        let doc = DocId::new("page-recovered").unwrap();
        storage
            .workspace()
            .unwrap()
            .append(&doc, &crate::storage::tests::update("recovered x"))
            .unwrap();
        let app = mock_builder().build(mock_context(noop_assets())).unwrap();
        app.manage(storage);
        assert!(tauri::async_runtime::block_on(super::storage_flush(app.state())).unwrap());
        disk.crash();
        let reopened = disk.open(Path::new("/data")).unwrap();
        assert_eq!(
            crate::storage::tests::text_of(&reopened.load(&doc).unwrap()),
            "recovered x"
        );
    }

    #[test]
    fn recovery_opener_receives_only_the_native_last_seen_path() {
        use crate::storage::{DataLocation, Storage};
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let cloud = base.join("cloud");
        let target = cloud.join("Aelomi");
        std::fs::create_dir_all(&target).unwrap();
        let configured = base.join("Aelomi");
        std::os::unix::fs::symlink(&target, &configured).unwrap();
        let location = DataLocation {
            path: configured.clone(),
            cloud_roots: vec![cloud],
        };
        let record = base.join("recovery.json");
        for missing in [false, true] {
            if missing {
                std::fs::remove_file(&configured).unwrap();
            }
            let storage = Storage::open_with_recovery(&location, record.clone(), |_| {});
            let mut opened = String::new();
            super::show_recovery_folder_with(&storage, |path| {
                opened = path.into();
                Ok(())
            })
            .unwrap();
            assert_eq!(opened, target.to_string_lossy());
        }
        let ready = Storage::open(
            &DataLocation {
                path: base.join("new"),
                cloud_roots: vec![],
            },
            |_| {},
        );
        assert!(matches!(
            super::show_recovery_folder_with(&ready, |_| panic!("no folder on Ready")),
            Err(crate::storage::StorageError::Unavailable)
        ));
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

#[cfg(test)]
mod page_records_tests;
