use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aelomi_core::index::IndexEvent;
use aelomi_core::storage::{self as core, DataPathError};
use aelomi_core::workspace::Workspace;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::storage::{
    CloudRecovery, DataLocation, QUIT_FLUSH_TIMEOUT, StorageStatus, normalize_configured,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum RefusalKind {
    LeafLink,
    SharedLink { link: PathBuf },
    Override,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PendingCloudRefusal {
    version: u32,
    pub configured: PathBuf,
    pub resolved: PathBuf,
    kind: RefusalKind,
    pub refused_at_unix_ms: u64,
}

impl PendingCloudRefusal {
    pub fn new(configured: &Path, resolved: &Path, recovery: CloudRecovery) -> Self {
        Self {
            version: 1,
            configured: configured.into(),
            resolved: resolved.into(),
            kind: match recovery {
                CloudRecovery::LinkAtDataFolder { .. } => RefusalKind::LeafLink,
                CloudRecovery::SharedLink { link } => RefusalKind::SharedLink { link: link.into() },
                CloudRecovery::Override => RefusalKind::Override,
            },
            refused_at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        }
    }

    pub fn recovery(&self) -> io::Result<CloudRecovery> {
        let link = match &self.kind {
            RefusalKind::LeafLink => Some(self.configured.as_path()),
            RefusalKind::SharedLink { link } => Some(link.as_path()),
            RefusalKind::Override => None,
        };
        CloudRecovery::from_paths(&self.configured, &self.resolved, link)
    }
}

#[derive(Clone)]
pub(crate) struct RecoveryDirectory(pub PathBuf);

impl RecoveryDirectory {
    pub fn validated(&self, location: &DataLocation, configured: &Path) -> io::Result<Self> {
        let directory = aelomi_core::storage::check_data_path(&self.0, &location.cloud_roots)
            .map_err(|error| io::Error::other(format!("invalid recovery directory: {error}")))?;
        let data = aelomi_core::storage::check_data_path(configured, &[]).map_err(|error| {
            io::Error::other(format!("cannot validate recovery directory: {error}"))
        })?;
        if directory.starts_with(data) {
            return Err(io::Error::other(
                "the recovery directory must be outside the data folder",
            ));
        }
        Ok(Self(directory))
    }

    pub fn file_for(&self, configured: &Path) -> RecoveryFile {
        let hash = Sha256::digest(configured.as_os_str().as_encoded_bytes());
        RecoveryFile {
            path: self.0.join(format!("{hash:x}.json")),
            configured: configured.into(),
        }
    }
}

pub(crate) struct RecoveryFile {
    pub path: PathBuf,
    configured: PathBuf,
}

impl RecoveryFile {
    pub fn load(&self) -> io::Result<Option<PendingCloudRefusal>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return match fs::symlink_metadata(&self.path) {
                    Err(missing) if missing.kind() == io::ErrorKind::NotFound => Ok(None),
                    _ => Err(error),
                };
            }
            Err(error) => return Err(error),
        };
        let record: PendingCloudRefusal = serde_json::from_slice(&bytes)?;
        if record.version != 1
            || record.configured != self.configured
            || !record.resolved.is_absolute()
        {
            return Err(io::Error::other("unrecognized recovery record"));
        }
        record.recovery()?;
        Ok(Some(record))
    }

    pub fn save(&self, record: &PendingCloudRefusal) -> io::Result<()> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = self.path.parent().expect("recovery directory");
        fs::create_dir_all(parent)?;
        let (temporary, mut file) = loop {
            let temporary = parent.join(format!(
                ".aelomi-recovery-{}-{}.tmp",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => break (temporary, file),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        };
        let mut write = || -> io::Result<()> {
            file.write_all(&serde_json::to_vec(record)?)?;
            file.sync_all()?;
            fs::rename(&temporary, &self.path)?;
            fs::File::open(parent)?.sync_all()
        };
        let result = write();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub fn clear(&self) {
        let clear = || -> io::Result<()> {
            match fs::remove_file(&self.path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                result => result?,
            }
            fs::File::open(self.path.parent().expect("recovery directory"))?.sync_all()
        };
        if let Err(error) = clear() {
            if cfg!(debug_assertions) {
                eprintln!("recovery record not cleared: {error}");
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenMode {
    Startup,
    CreateEmpty,
}

type OpenedWorkspace = Result<Arc<Workspace>, StorageStatus>;

pub(crate) fn open_workspace(
    location: &DataLocation,
    record: Option<&RecoveryDirectory>,
    mode: OpenMode,
    on_index: impl Fn(&IndexEvent) + Send + Sync + 'static,
) -> (OpenedWorkspace, PathBuf) {
    let configured = match normalize_configured(&location.path) {
        Ok(path) => path,
        Err(error) => {
            return (
                Err(StorageStatus::Io(error.to_string())),
                location.path.clone(),
            );
        }
    };
    let directory = record
        .map(|directory| directory.validated(location, &configured))
        .transpose();
    match core::check_data_path(&configured, &location.cloud_roots) {
        Ok(resolved) => {
            let destination = match core::classify_destination(&resolved) {
                Ok(destination) => destination,
                Err(error) => return (Err(StorageStatus::from(error)), resolved),
            };
            let directory = if destination == core::DestinationFolder::Workspace {
                directory.ok().flatten()
            } else {
                match directory {
                    Ok(directory) => directory,
                    Err(error) => {
                        return (Err(StorageStatus::Io(error.to_string())), resolved);
                    }
                }
            };
            let record = directory
                .as_ref()
                .map(|directory| directory.file_for(&configured));
            if mode == OpenMode::Startup && destination != core::DestinationFolder::Workspace {
                if let Some(record) = &record {
                    let status = match record.load() {
                        Ok(Some(pending)) => Some(StorageStatus::FinishMoving {
                            destination_exists: destination != core::DestinationFolder::Absent,
                            configured: configured.to_string_lossy().into_owned(),
                            resolved: pending.resolved.to_string_lossy().into_owned(),
                            recovery: pending.recovery().expect("validated recovery record"),
                        }),
                        Err(error) => {
                            if cfg!(debug_assertions) {
                                eprintln!("recovery record unreadable: {error}");
                            }
                            Some(StorageStatus::RecoveryRecordUnreadable {
                                configured: configured.to_string_lossy().into_owned(),
                            })
                        }
                        Ok(None) => None,
                    };
                    if let Some(status) = status {
                        return (Err(status), resolved);
                    }
                }
            }
            let opened = Workspace::prepare(&resolved)
                .map_err(StorageStatus::from)
                .and_then(|mut prepared| {
                    let deadline = Instant::now() + QUIT_FLUSH_TIMEOUT;
                    match mode {
                        OpenMode::Startup => prepared.seed_getting_started(deadline),
                        OpenMode::CreateEmpty => prepared
                            .initialize_empty(deadline)
                            .map_err(|error| StorageStatus::Io(error.to_string()))?,
                    }
                    Ok(Arc::new(prepared.start(on_index)))
                });
            if opened.is_ok() {
                if let Some(record) = &record {
                    record.clear();
                }
            }
            (opened, resolved)
        }
        Err(DataPathError::CloudFolder { resolved, link, .. }) => {
            let refusal = || -> std::io::Result<StorageStatus> {
                let directory = directory?;
                let record = directory
                    .as_ref()
                    .map(|directory| directory.file_for(&configured));
                let recovery = CloudRecovery::from_paths(&configured, &resolved, link.as_deref())?;
                if let Some(record) = &record {
                    record.save(&PendingCloudRefusal::new(
                        &configured,
                        &resolved,
                        recovery.clone(),
                    ))?;
                }
                Ok(StorageStatus::CloudPath {
                    configured: configured.to_string_lossy().into_owned(),
                    resolved: resolved.to_string_lossy().into_owned(),
                    recovery,
                })
            };
            let status = refusal().unwrap_or_else(|error| StorageStatus::Io(error.to_string()));
            (Err(status), resolved)
        }
        Err(DataPathError::Io(error)) => (
            Err(StorageStatus::Io(error.to_string())),
            location.path.clone(),
        ),
    }
}
