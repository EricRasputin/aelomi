use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aelomi_core::index::IndexEvent;
use aelomi_core::storage::{self as core, DocId, OpenError, Pending};
use aelomi_core::workspace::Workspace;
use serde::Serialize;
use specta::Type;

use crate::quit::QuitGate;
use crate::recovery::{OpenMode, RecoveryDirectory, open_workspace};

pub const QUIT_FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct DataLocation {
    pub path: PathBuf,
    pub cloud_roots: Vec<PathBuf>,
}

/// `AELOMI_DATA_DIR` replaces the path in every build. `AELOMI_CLOUD_ROOTS`, a `:`-separated
/// list, replaces the cloud roots only in debug builds.
pub fn data_location(
    platform: DataLocation,
    var: impl Fn(&str) -> Option<OsString>,
    debug: bool,
) -> DataLocation {
    let path = var("AELOMI_DATA_DIR")
        .filter(|dir| !dir.is_empty())
        .map_or(platform.path, PathBuf::from);
    let cloud_roots = match var("AELOMI_CLOUD_ROOTS").filter(|_| debug) {
        Some(roots) => std::env::split_paths(&roots).collect(),
        None => platform.cloud_roots,
    };
    DataLocation { path, cloud_roots }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum StorageStatus {
    Ready,
    CloudPath {
        configured: String,
        resolved: String,
        recovery: CloudRecovery,
    },
    FinishMoving {
        destination_exists: bool,
        configured: String,
        resolved: String,
        recovery: CloudRecovery,
    },
    RecoveryRecordUnreadable {
        configured: String,
    },
    Locked,
    NewerFormat,
    UnknownFormat,
    Io(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum CloudRecovery {
    LinkAtDataFolder {
        configured_parent: String,
        configured_name: String,
        resolved_parent: String,
        resolved_name: String,
    },
    SharedLink {
        link: String,
    },
    Override,
}

impl CloudRecovery {
    pub(crate) fn from_paths(
        configured: &Path,
        resolved: &Path,
        link: Option<&Path>,
    ) -> std::io::Result<Self> {
        match link {
            Some(link) if link == configured => {
                let parts = |path: &Path| {
                    path.parent()
                        .zip(path.file_name())
                        .map(|(parent, name)| {
                            (
                                parent.to_string_lossy().into_owned(),
                                name.to_string_lossy().into_owned(),
                            )
                        })
                        .ok_or_else(|| {
                            std::io::Error::other("the cloud data folder has no parent or name")
                        })
                };
                let (configured_parent, configured_name) = parts(configured)?;
                let (resolved_parent, resolved_name) = parts(resolved)?;
                Ok(Self::LinkAtDataFolder {
                    configured_parent,
                    configured_name,
                    resolved_parent,
                    resolved_name,
                })
            }
            Some(link) => Ok(Self::SharedLink {
                link: link.to_string_lossy().into_owned(),
            }),
            None => Ok(Self::Override),
        }
    }
}

pub(crate) fn normalize_configured(path: &Path) -> std::io::Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in std::path::absolute(path)?.components() {
        if component == Component::ParentDir {
            normalized.pop();
        } else {
            normalized.push(component);
        }
    }
    Ok(normalized)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum FailReason {
    DiskFull,
    PermissionDenied,
    ReadOnlyVolume,
    Io(String),
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum Health {
    Healthy,
    Failing {
        reason: FailReason,
        pending_docs: Vec<String>,
        pending_settings: bool,
    },
}

#[derive(Debug, Serialize, Type)]
pub enum StorageError {
    DestinationNotEmpty {
        items: Vec<String>,
    },
    /// The store is not open. `storage_status` says why.
    Unavailable,
    InvalidDocId,
    InvalidUpdate,
    Io(String),
}

pub struct Storage {
    opened: Mutex<Result<Arc<Workspace>, StorageStatus>>,
    recovery: Option<(DataLocation, RecoveryDirectory)>,
    path: PathBuf,
    quit: QuitGate,
    #[cfg(test)]
    _index_dir: Option<tempfile::TempDir>,
}

impl Storage {
    #[cfg(test)]
    pub fn open(
        location: &DataLocation,
        on_index: impl Fn(&IndexEvent) + Send + Sync + 'static,
    ) -> Self {
        Self::open_inner(location, None, on_index)
    }

    pub(crate) fn open_with_recovery(
        location: &DataLocation,
        record: PathBuf,
        on_index: impl Fn(&IndexEvent) + Send + Sync + 'static,
    ) -> Self {
        Self::open_inner(location, Some(RecoveryDirectory(record)), on_index)
    }

    fn open_inner(
        location: &DataLocation,
        record: Option<RecoveryDirectory>,
        on_index: impl Fn(&IndexEvent) + Send + Sync + 'static,
    ) -> Self {
        let (opened, path) = open_workspace(location, record.as_ref(), OpenMode::Startup, on_index);
        Self {
            quit: QuitGate::for_workspace(opened.is_ok()),
            opened: Mutex::new(opened),
            recovery: record.map(|file| (location.clone(), file)),
            path,
            #[cfg(test)]
            _index_dir: None,
        }
    }

    pub(crate) fn start_empty_workspace(
        &self,
        on_index: impl Fn(&IndexEvent) + Send + Sync + 'static,
    ) -> Result<StorageStatus, StorageError> {
        let (location, record) = self.recovery.as_ref().ok_or(StorageError::Unavailable)?;
        let on_index = Arc::new(on_index);
        self.start_empty_workspace_with(|mode| {
            let on_index = Arc::clone(&on_index);
            open_workspace(location, Some(record), mode, move |event| on_index(event))
        })
    }

    fn start_empty_workspace_with(
        &self,
        mut open: impl FnMut(OpenMode) -> (Result<Arc<Workspace>, StorageStatus>, PathBuf),
    ) -> Result<StorageStatus, StorageError> {
        if !matches!(
            self.status(),
            StorageStatus::FinishMoving { .. } | StorageStatus::RecoveryRecordUnreadable { .. }
        ) {
            return Err(StorageError::Unavailable);
        }
        let (location, _) = self.recovery.as_ref().ok_or(StorageError::Unavailable)?;
        let configured = normalize_configured(&location.path)
            .map_err(|error| StorageError::Io(error.to_string()))?;
        let mode = match core::check_data_path(&configured, &location.cloud_roots) {
            Ok(resolved) => match core::classify_destination(&resolved)
                .map_err(|error| StorageError::Io(error.to_string()))?
            {
                core::DestinationFolder::Absent | core::DestinationFolder::Empty => {
                    OpenMode::CreateEmpty
                }
                core::DestinationFolder::Workspace => OpenMode::Startup,
                core::DestinationFolder::NotEmpty { items }
                | core::DestinationFolder::Unrecognized { items } => {
                    if let Err(StorageStatus::FinishMoving {
                        destination_exists, ..
                    }) = self
                        .opened
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .as_mut()
                    {
                        *destination_exists = true;
                    }
                    return Err(StorageError::DestinationNotEmpty { items });
                }
            },
            Err(_) => OpenMode::CreateEmpty,
        };
        let (mut result, _) = open(mode);
        let creation_error = match &result {
            Err(StorageStatus::Io(error)) if mode == OpenMode::CreateEmpty => Some(error.clone()),
            _ => None,
        };
        if creation_error.is_some() {
            (result, _) = open(OpenMode::Startup);
        }
        let mut opened = self
            .opened
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if opened.is_ok() {
            return Ok(StorageStatus::Ready);
        }
        if let Some(error) = creation_error {
            if result.is_err() {
                *opened = result;
                return Err(StorageError::Io(error));
            }
        }
        match result {
            Err(StorageStatus::Io(error)) => Err(StorageError::Io(error)),
            Err(status) if !matches!(status, StorageStatus::CloudPath { .. }) => Err(
                StorageError::Io(format!("could not open the destination: {status:?}")),
            ),
            other => {
                let ready = other.is_ok();
                *opened = other;
                if ready {
                    self.quit.workspace_opened();
                }
                Ok(opened
                    .as_ref()
                    .map_or_else(Clone::clone, |_| StorageStatus::Ready))
            }
        }
    }

    pub(crate) fn recovery_folder(&self) -> Result<String, StorageError> {
        match self.status() {
            StorageStatus::CloudPath { resolved, .. }
            | StorageStatus::FinishMoving { resolved, .. } => Ok(resolved),
            _ => Err(StorageError::Unavailable),
        }
    }

    #[cfg(test)]
    pub(crate) fn not_open() -> Self {
        Self {
            opened: Mutex::new(Err(StorageStatus::Locked)),
            recovery: None,
            path: "/data".into(),
            quit: QuitGate::for_workspace(false),
            _index_dir: None,
        }
    }

    pub fn workspace(&self) -> Result<Arc<Workspace>, StorageError> {
        self.opened
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(Arc::clone)
            .map_err(|_| StorageError::Unavailable)
    }

    pub fn index_status(&self) -> crate::index::IndexStatus {
        match &*self
            .opened
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            Ok(workspace) => workspace.index_status().into(),
            Err(_) => crate::index::IndexStatus::Failed {
                reason: "the data folder is not open".into(),
            },
        }
    }

    pub(crate) fn register_frontend(&self) {
        if self
            .opened
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_ok()
        {
            self.quit.register_frontend();
            println!("quit frontend ready");
        } else {
            println!("quit frontend ready skipped: storage refused");
        }
    }

    pub(crate) fn quit(&self) -> &QuitGate {
        &self.quit
    }

    pub fn status(&self) -> StorageStatus {
        match &*self
            .opened
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            Ok(_) => StorageStatus::Ready,
            Err(status) => status.clone(),
        }
    }

    pub fn health(&self) -> Health {
        self.workspace()
            .map_or(Health::Healthy, |workspace| (&workspace.health()).into())
    }

    pub fn startup_line(&self) -> String {
        format!(
            "storage status={:?} path={}",
            self.status(),
            self.path.display()
        )
    }

    pub fn pending_edits(&self) -> Pending {
        match self.workspace().map(|workspace| workspace.health()) {
            Ok(core::Health::Failing {
                pending_docs,
                pending_settings,
                ..
            }) => Pending {
                docs: pending_docs,
                settings: pending_settings,
            },
            _ => Pending::default(),
        }
    }

    pub fn flush_before_quit(&self, timeout: Duration) -> Result<(), Pending> {
        let deadline = Instant::now() + timeout;
        if !self.quit.wait_for_record_edits(deadline) {
            let mut pending = self.pending_edits();
            pending.docs.push(DocId::workspace());
            pending.docs.sort();
            pending.docs.dedup();
            return Err(pending);
        }
        let Ok(workspace) = self.workspace() else {
            return Ok(());
        };
        let pending = workspace.flush(deadline);
        if pending.is_empty() {
            Ok(())
        } else {
            Err(pending)
        }
    }
}

impl From<OpenError> for StorageStatus {
    fn from(error: OpenError) -> Self {
        match error {
            OpenError::Locked => Self::Locked,
            OpenError::NewerFormat { .. } => Self::NewerFormat,
            OpenError::UnknownFormat => Self::UnknownFormat,
            OpenError::Io(error) => Self::Io(error.to_string()),
        }
    }
}

impl From<&core::Health> for Health {
    fn from(health: &core::Health) -> Self {
        match health {
            core::Health::Healthy => Self::Healthy,
            core::Health::Failing {
                reason,
                pending_docs,
                pending_settings,
                ..
            } => Self::Failing {
                reason: reason.into(),
                pending_docs: pending_docs.iter().map(ToString::to_string).collect(),
                pending_settings: *pending_settings,
            },
        }
    }
}

impl From<&core::FailReason> for FailReason {
    fn from(reason: &core::FailReason) -> Self {
        match reason {
            core::FailReason::DiskFull => Self::DiskFull,
            core::FailReason::PermissionDenied => Self::PermissionDenied,
            core::FailReason::ReadOnlyVolume => Self::ReadOnlyVolume,
            core::FailReason::Io(error) => Self::Io(error.clone()),
            core::FailReason::Internal => Self::Internal,
        }
    }
}

impl From<core::AppendError> for StorageError {
    fn from(error: core::AppendError) -> Self {
        match error {
            core::AppendError::InvalidUpdate => Self::InvalidUpdate,
            core::AppendError::Io(error) => Self::Io(error.to_string()),
        }
    }
}

impl From<StorageError> for crate::index::QueryError {
    fn from(_: StorageError) -> Self {
        Self::Unavailable
    }
}

impl From<core::InvalidDocId> for StorageError {
    fn from(_: core::InvalidDocId) -> Self {
        Self::InvalidDocId
    }
}

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[derive(Clone, Serialize, Type, tauri_specta::Event)]
pub struct StorageHealthChanged(pub Health);

#[cfg(test)]
pub(crate) mod tests {
    use std::path::Path;

    use aelomi_core::storage::{DocId, test_support::SimDisk};

    use super::*;

    fn platform() -> DataLocation {
        DataLocation {
            path: "/home/Library/Application Support/Aelomi".into(),
            cloud_roots: vec!["/home/Library/Mobile Documents".into()],
        }
    }

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let vars: Vec<(String, OsString)> = vars
            .iter()
            .map(|(name, value)| (name.to_string(), value.into()))
            .collect();
        move |name| vars.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone())
    }

    #[test]
    fn the_data_dir_override_applies_in_every_build() {
        for debug in [true, false] {
            let location =
                data_location(platform(), env(&[("AELOMI_DATA_DIR", "/run/data")]), debug);
            assert_eq!(location.path, PathBuf::from("/run/data"));
            assert_eq!(location.cloud_roots, platform().cloud_roots);
        }
        let location = data_location(platform(), env(&[("AELOMI_DATA_DIR", "")]), true);
        assert_eq!(location.path, platform().path);
    }

    #[test]
    fn only_debug_builds_replace_the_cloud_roots() {
        let vars = env(&[(
            "AELOMI_CLOUD_ROOTS",
            "/fake/Mobile Documents:/fake/CloudStorage",
        )]);
        let debug = data_location(platform(), &vars, true);
        assert_eq!(
            debug.cloud_roots,
            [
                PathBuf::from("/fake/Mobile Documents"),
                PathBuf::from("/fake/CloudStorage")
            ]
        );
        assert_eq!(debug.path, platform().path);
        let release = data_location(platform(), &vars, false);
        assert_eq!(release.cloud_roots, platform().cloud_roots);
    }

    #[test]
    fn opening_an_empty_folder_is_ready() {
        let dir = tempfile::tempdir().unwrap();
        let location = DataLocation {
            path: dir.path().to_path_buf(),
            cloud_roots: vec![],
        };

        let storage = Storage::open(&location, |_| {});

        assert_eq!(storage.status(), StorageStatus::Ready);
        assert_eq!(
            storage.startup_line(),
            format!(
                "storage status=Ready path={}",
                std::fs::canonicalize(dir.path()).unwrap().display()
            )
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("format.json")).unwrap(),
            "{\n  \"format\": 2\n}\n"
        );
        assert!(dir.path().join("docs").is_dir());
        let tree = storage.workspace().unwrap().tree().unwrap();
        assert_eq!(tree.pages.len(), 1);
        assert_eq!(tree.pages[0].title, "Getting started");
    }

    #[test]
    fn opening_a_folder_held_by_another_store_is_locked() {
        let dir = tempfile::tempdir().unwrap();
        let first = aelomi_core::storage::Store::open(dir.path()).unwrap();
        let location = DataLocation {
            path: dir.path().to_path_buf(),
            cloud_roots: vec![],
        };

        let storage = Storage::open(&location, |_| {});

        assert_eq!(storage.status(), StorageStatus::Locked);
        assert_eq!(
            storage.startup_line(),
            format!(
                "storage status=Locked path={}",
                std::fs::canonicalize(dir.path()).unwrap().display()
            )
        );
        drop(first);
    }

    #[test]
    fn opening_a_newer_format_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("format.json"), r#"{"format": 3}"#).unwrap();
        let location = DataLocation {
            path: dir.path().to_path_buf(),
            cloud_roots: vec![],
        };

        let storage = Storage::open(&location, |_| {});

        assert_eq!(storage.status(), StorageStatus::NewerFormat);
        assert_eq!(
            storage.startup_line(),
            format!(
                "storage status=NewerFormat path={}",
                std::fs::canonicalize(dir.path()).unwrap().display()
            )
        );
    }

    #[test]
    fn opening_an_unknown_format_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("format.json"), r#"{"format": "one"}"#).unwrap();
        let location = DataLocation {
            path: dir.path().to_path_buf(),
            cloud_roots: vec![],
        };

        let storage = Storage::open(&location, |_| {});

        assert_eq!(storage.status(), StorageStatus::UnknownFormat);
        assert_eq!(
            storage.startup_line(),
            format!(
                "storage status=UnknownFormat path={}",
                std::fs::canonicalize(dir.path()).unwrap().display()
            )
        );
    }

    #[test]
    fn opening_a_regular_file_reports_io() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data");
        std::fs::write(&path, b"not a data folder").unwrap();
        let location = DataLocation {
            path: path.clone(),
            cloud_roots: vec![],
        };

        let storage = Storage::open(&location, |_| {});

        assert_eq!(
            storage.status(),
            StorageStatus::Io("Not a directory (os error 20)".into())
        );
        assert_eq!(
            storage.startup_line(),
            format!(
                "storage status=Io(\"Not a directory (os error 20)\") path={}",
                std::fs::canonicalize(&path).unwrap().display()
            )
        );
        assert_eq!(std::fs::read(path).unwrap(), b"not a data folder");
    }

    #[test]
    fn cloud_leaf_recovery_has_parts_for_normal_trailing_slash_and_dot_dot_paths() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let cloud = base.join("cloud");
        let target = cloud.join("Aelomi Notes");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("sentinel"), b"pages stay intact").unwrap();
        let link = base.join("Aelomi");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        for path in [link.clone(), link.join(""), base.join("missing/../Aelomi")] {
            let storage = Storage::open(
                &DataLocation {
                    path,
                    cloud_roots: vec![cloud.clone()],
                },
                |_| {},
            );
            assert_eq!(
                serde_json::to_value(storage.status()).unwrap(),
                serde_json::json!({
                    "CloudPath": {
                        "configured": link,
                        "resolved": target,
                        "recovery": { "LinkAtDataFolder": {
                            "configured_parent": base,
                            "configured_name": "Aelomi",
                            "resolved_parent": cloud,
                            "resolved_name": "Aelomi Notes"
                        }}
                    }
                })
            );
            assert_eq!(
                std::fs::read_dir(&target)
                    .unwrap()
                    .map(|e| e.unwrap().file_name())
                    .collect::<Vec<_>>(),
                [OsString::from("sentinel")]
            );
            assert_eq!(
                std::fs::read(target.join("sentinel")).unwrap(),
                b"pages stay intact"
            );
            assert_eq!(std::fs::read_link(&link).unwrap(), target);
        }
    }

    #[test]
    fn cloud_shared_link_recovery_names_the_shared_folder_without_changing_data() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let cloud = base.join("cloud");
        let target = cloud.join("Aelomi Dev");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("sentinel"), b"pages stay intact").unwrap();
        std::fs::create_dir(base.join("Library")).unwrap();
        let link = base.join("Library/Application Support");
        std::os::unix::fs::symlink(&cloud, &link).unwrap();
        let configured = link.join("Aelomi Dev");
        let storage = Storage::open(
            &DataLocation {
                path: configured.clone(),
                cloud_roots: vec![cloud.clone()],
            },
            |_| {},
        );
        assert_eq!(
            serde_json::to_value(storage.status()).unwrap(),
            serde_json::json!({
                "CloudPath": { "configured": configured, "resolved": target, "recovery": { "SharedLink": { "link": link } } }
            })
        );
        assert_eq!(std::fs::read_link(link).unwrap(), cloud);
        assert_eq!(
            std::fs::read_dir(&target)
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect::<Vec<_>>(),
            [OsString::from("sentinel")]
        );
        assert_eq!(
            std::fs::read(target.join("sentinel")).unwrap(),
            b"pages stay intact"
        );
    }

    #[test]
    fn cloud_override_recovery_keeps_paths_without_changing_data() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let cloud = base.join("cloud");
        let target = cloud.join("Aelomi");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("sentinel"), b"pages stay intact").unwrap();
        let storage = Storage::open(
            &DataLocation {
                path: target.clone(),
                cloud_roots: vec![cloud],
            },
            |_| {},
        );
        assert_eq!(
            serde_json::to_value(storage.status()).unwrap(),
            serde_json::json!({
                "CloudPath": { "configured": target, "resolved": target, "recovery": "Override" }
            })
        );
        assert_eq!(
            std::fs::read_dir(&target)
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect::<Vec<_>>(),
            [OsString::from("sentinel")]
        );
        assert_eq!(
            std::fs::read(target.join("sentinel")).unwrap(),
            b"pages stay intact"
        );
    }

    pub(crate) fn open_on(disk: &SimDisk) -> Storage {
        open_on_with(disk, QuitGate::default())
    }

    pub(crate) fn open_on_with(disk: &SimDisk, quit: QuitGate) -> Storage {
        let index_dir = tempfile::tempdir().unwrap();
        let index = std::fs::canonicalize(index_dir.path())
            .unwrap()
            .join(aelomi_core::index::FILE);
        let store = disk.open(Path::new("/data")).expect("opens");
        Storage {
            opened: Mutex::new(Ok(Arc::new(Workspace::with_store(store, index, |_| {})))),
            recovery: None,
            path: "/data".into(),
            quit,
            _index_dir: Some(index_dir),
        }
    }

    pub(crate) fn update(text: &str) -> Vec<u8> {
        use yrs::{Text, Transact};
        let doc = yrs::Doc::new();
        let body = doc.get_or_insert_text("body");
        let mut txn = doc.transact_mut();
        body.insert(&mut txn, 0, text);
        txn.encode_update_v1()
    }

    #[test]
    fn quitting_waits_for_the_flush_and_leaves_the_update_durable() {
        let disk = SimDisk::default();
        let storage = open_on(&disk);
        disk.set_full_sync_latency(Duration::from_millis(300));
        let doc = DocId::new("page").unwrap();
        let typed = update("typed just before quit");
        storage.workspace().unwrap().append(&doc, &typed).unwrap();

        let start = Instant::now();
        assert_eq!(storage.flush_before_quit(QUIT_FLUSH_TIMEOUT), Ok(()));
        assert!(
            start.elapsed() >= Duration::from_millis(300),
            "{:?}",
            start.elapsed()
        );

        disk.crash();
        let reopened = disk.open(Path::new("/data")).expect("reopens");
        assert_eq!(
            text_of(&reopened.load(&doc).unwrap()),
            "typed just before quit"
        );
    }

    #[test]
    fn quitting_is_refused_while_updates_are_pending() {
        let disk = SimDisk::default();
        let storage = open_on(&disk);
        disk.fail_appends("docs/page/");
        let doc = DocId::new("page").unwrap();
        storage
            .workspace()
            .unwrap()
            .append(&doc, &update("unsaved"))
            .unwrap();

        let start = Instant::now();
        assert_eq!(
            storage.flush_before_quit(Duration::from_millis(300)),
            Err(Pending {
                docs: vec![doc.clone()],
                settings: false
            })
        );
        assert!(
            start.elapsed() >= Duration::from_millis(300),
            "{:?}",
            start.elapsed()
        );

        disk.clear_faults();
        assert_eq!(storage.flush_before_quit(QUIT_FLUSH_TIMEOUT), Ok(()));
    }

    #[test]
    fn record_wait_timeout_keeps_all_failing_documents_sorted_and_deduplicated() {
        let disk = SimDisk::default();
        let storage = open_on(&disk);
        disk.fail_appends("docs/");
        let workspace = storage.workspace().unwrap();
        for id in ["z-page", "workspace", "a-page"] {
            workspace
                .append(&DocId::new(id).unwrap(), &update(id))
                .unwrap();
        }
        let expected = vec![
            DocId::new("a-page").unwrap(),
            DocId::workspace(),
            DocId::new("z-page").unwrap(),
        ];
        assert_eq!(
            workspace
                .flush(Instant::now() + Duration::from_millis(100))
                .docs,
            expected
        );
        let held = storage.quit.record_edit().unwrap();
        storage.quit.register_frontend();
        let crate::quit::QuitRequest::StartDrain(status) = storage.quit.request() else {
            panic!("starts a drain")
        };
        assert!(storage.quit.begin_completing(status.revision));
        assert_eq!(
            storage.flush_before_quit(Duration::ZERO),
            Err(Pending {
                docs: expected,
                settings: false
            })
        );
        drop(held);
        disk.clear_faults();
        assert_eq!(storage.flush_before_quit(QUIT_FLUSH_TIMEOUT), Ok(()));
    }

    #[test]
    fn quitting_goes_ahead_when_the_store_never_opened() {
        let storage = Storage {
            opened: Mutex::new(Err(StorageStatus::Locked)),
            recovery: None,
            path: "/data".into(),
            quit: QuitGate::default(),
            _index_dir: None,
        };
        assert_eq!(storage.flush_before_quit(QUIT_FLUSH_TIMEOUT), Ok(()));
        assert_eq!(storage.status(), StorageStatus::Locked);
        assert_eq!(storage.health(), Health::Healthy);
        assert_eq!(storage.startup_line(), "storage status=Locked path=/data");
    }

    pub(crate) fn text_of(state: &[u8]) -> String {
        use yrs::updates::decoder::Decode;
        use yrs::{GetString, Transact, Update};
        let doc = yrs::Doc::new();
        let body = doc.get_or_insert_text("body");
        doc.transact_mut()
            .apply_update(Update::decode_v1(state).unwrap())
            .unwrap();
        body.get_string(&doc.transact())
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod recovery_tests;
