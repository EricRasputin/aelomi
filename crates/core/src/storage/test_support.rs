//! A simulated disk for tests outside this crate.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::clock::SystemClock;
use super::sim_fs::{Crash, OpKind, SimFs};
use super::state::Limits;
use super::store::{Driver, OpenError, Store};

/// An in-memory disk that loses every byte not yet durable when it crashes.
pub struct SimDisk(Arc<SimFs>);

impl Default for SimDisk {
    fn default() -> Self {
        Self(SimFs::new(Arc::new(SystemClock::new())))
    }
}

impl SimDisk {
    /// Opens a store on this disk, with its writer and compactor threads running.
    pub fn open(&self, root: &Path) -> Result<Store, OpenError> {
        let clock = Arc::new(SystemClock::new());
        Store::open_with(root, self.0.clone(), clock, Driver::Threads, Limits::DATA_6)
    }

    pub fn set_full_sync_latency(&self, latency: Duration) {
        self.0.set_latency(OpKind::FullSync, latency);
    }

    /// Fails with `EIO` every append to a path that contains `path_contains`.
    pub fn fail_appends(&self, path_contains: &str) {
        self.0.fail(OpKind::Append, path_contains, libc::EIO);
    }

    pub fn clear_faults(&self) {
        self.0.clear_faults();
    }

    /// Drops everything not yet durable and releases every lock, as a power loss would.
    pub fn crash(&self) {
        self.0.crash(Crash::DropUnsynced);
    }
}
