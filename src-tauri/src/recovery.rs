use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::storage::{CloudRecovery, DataLocation};

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
    pub fn validated(&self, location: &DataLocation) -> io::Result<Self> {
        let directory = aelomi_core::storage::check_data_path(&self.0, &location.cloud_roots)
            .map_err(|error| io::Error::other(format!("invalid recovery directory: {error}")))?;
        let data = aelomi_core::storage::check_data_path(&location.path, &[]).map_err(|error| {
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
