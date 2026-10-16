use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::storage::CloudRecovery;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PendingCloudRefusal {
    pub configured: PathBuf,
    pub resolved: PathBuf,
    pub recovery: CloudRecovery,
    pub refused_at_unix_ms: u64,
}

impl PendingCloudRefusal {
    pub fn new(configured: &Path, resolved: &Path, recovery: CloudRecovery) -> Self {
        Self {
            configured: configured.into(),
            resolved: resolved.into(),
            recovery,
            refused_at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        }
    }
}

#[derive(Clone)]
pub(crate) struct RecoveryFile(pub PathBuf);

impl RecoveryFile {
    pub fn load(&self) -> Option<PendingCloudRefusal> {
        let read = || -> io::Result<PendingCloudRefusal> {
            let record: PendingCloudRefusal = serde_json::from_slice(&fs::read(&self.0)?)?;
            if !record.configured.is_absolute() || !record.resolved.is_absolute() {
                return Err(io::Error::other("recovery paths must be absolute"));
            }
            Ok(record)
        };
        match read() {
            Ok(record) => Some(record),
            Err(error) => {
                if cfg!(debug_assertions) && error.kind() != io::ErrorKind::NotFound {
                    eprintln!("recovery record ignored: {error}");
                }
                None
            }
        }
    }

    pub fn save(&self, record: &PendingCloudRefusal) -> io::Result<()> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = self
            .0
            .parent()
            .ok_or_else(|| io::Error::other("recovery file has no parent"))?;
        fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(
            ".aelomi-recovery-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let mut write = || -> io::Result<()> {
            file.write_all(&serde_json::to_vec(record)?)?;
            file.sync_all()?;
            fs::rename(&temporary, &self.0)?;
            fs::File::open(parent)?.sync_all()
        };
        let result = write();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub fn clear_for(&self, configured: &Path) {
        if self
            .load()
            .is_some_and(|record| record.configured == configured)
        {
            let clear = || -> io::Result<()> {
                fs::remove_file(&self.0)?;
                fs::File::open(self.0.parent().expect("record parent"))?.sync_all()
            };
            if let Err(error) = clear() {
                if cfg!(debug_assertions) {
                    eprintln!("recovery record not cleared: {error}");
                }
            }
        }
    }
}
