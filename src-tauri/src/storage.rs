use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use aelomi_core::storage::{self as core, DataPathError, DocId, OpenError, Store};
use serde::Serialize;
use specta::Type;

pub const QUIT_FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

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
    CloudPath { resolved: String },
    Locked,
    NewerFormat,
    UnknownFormat,
    Io(String),
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
    },
}

#[derive(Debug, Serialize, Type)]
pub enum StorageError {
    /// The store is not open. `storage_status` says why.
    Unavailable,
    InvalidDocId,
    InvalidUpdate,
    Io(String),
}

pub struct Storage {
    opened: Result<Store, StorageStatus>,
    path: PathBuf,
}

impl Storage {
    pub fn open(location: &DataLocation) -> Self {
        match core::check_data_path(&location.path, &location.cloud_roots) {
            Ok(resolved) => Self {
                opened: Store::open(&resolved).map_err(StorageStatus::from),
                path: resolved,
            },
            Err(DataPathError::CloudFolder { resolved, .. }) => Self {
                opened: Err(StorageStatus::CloudPath {
                    resolved: resolved.to_string_lossy().into_owned(),
                }),
                path: resolved,
            },
            Err(DataPathError::Io(error)) => Self {
                opened: Err(StorageStatus::Io(error.to_string())),
                path: location.path.clone(),
            },
        }
    }

    pub fn store(&self) -> Result<&Store, StorageError> {
        self.opened.as_ref().map_err(|_| StorageError::Unavailable)
    }

    pub fn status(&self) -> StorageStatus {
        match &self.opened {
            Ok(_) => StorageStatus::Ready,
            Err(status) => status.clone(),
        }
    }

    pub fn health(&self) -> Health {
        self.store()
            .map_or(Health::Healthy, |store| (&store.health()).into())
    }

    pub fn startup_line(&self) -> String {
        format!(
            "storage status={:?} path={}",
            self.status(),
            self.path.display()
        )
    }

    pub fn flush_before_quit(&self, timeout: Duration) -> Result<(), Vec<DocId>> {
        let Ok(store) = &self.opened else {
            return Ok(());
        };
        let pending = store.flush(Instant::now() + timeout);
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
                ..
            } => Self::Failing {
                reason: match reason {
                    core::FailReason::DiskFull => FailReason::DiskFull,
                    core::FailReason::PermissionDenied => FailReason::PermissionDenied,
                    core::FailReason::ReadOnlyVolume => FailReason::ReadOnlyVolume,
                    core::FailReason::Io(error) => FailReason::Io(error.clone()),
                    core::FailReason::Internal => FailReason::Internal,
                },
                pending_docs: pending_docs.iter().map(|doc| doc.to_string()).collect(),
            },
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

/// Quitting waited for pending updates and gave up, so the app stays open.
#[derive(Clone, Serialize, Type, tauri_specta::Event)]
pub struct QuitBlocked {
    pub pending_docs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use aelomi_core::storage::test_support::SimDisk;

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

    fn open_on(disk: &SimDisk) -> Storage {
        Storage {
            opened: Ok(disk.open(Path::new("/data")).expect("opens")),
            path: "/data".into(),
        }
    }

    fn update(text: &str) -> Vec<u8> {
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
        storage.store().unwrap().append(&doc, &typed).unwrap();

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
            .store()
            .unwrap()
            .append(&doc, &update("unsaved"))
            .unwrap();

        let start = Instant::now();
        assert_eq!(
            storage.flush_before_quit(Duration::from_millis(300)),
            Err(vec![doc.clone()])
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
    fn quitting_goes_ahead_when_the_store_never_opened() {
        let storage = Storage {
            opened: Err(StorageStatus::Locked),
            path: "/data".into(),
        };
        assert_eq!(storage.flush_before_quit(QUIT_FLUSH_TIMEOUT), Ok(()));
        assert_eq!(storage.status(), StorageStatus::Locked);
        assert_eq!(storage.health(), Health::Healthy);
        assert_eq!(storage.startup_line(), "storage status=Locked path=/data");
    }

    fn text_of(state: &[u8]) -> String {
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
