//! Store tests. Every test on `SimFs` also asserts the append-only rules (invariant 1):
//! `SimFs` checks them as each operation runs, and [`Env`] checks for violations on drop.

mod store;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::clock::{Clock, ManualClock, SystemClock};
use super::doc_id::DocId;
use super::sim_fs::SimFs;
use super::store::{Driver, OpenError, Store};
use super::testkit::{doc_from, fingerprint};

pub(crate) struct Env {
    pub fs: Arc<SimFs>,
    pub clock: Arc<ManualClock>,
    store_clock: Arc<dyn Clock>,
    driver: Driver,
}

impl Env {
    /// The writer runs only when the test steps it, on a manual clock.
    pub fn manual() -> Self {
        let clock = Arc::new(ManualClock::default());
        let fs = SimFs::new(clock.clone());
        Self {
            fs,
            store_clock: clock.clone(),
            clock,
            driver: Driver::Manual,
        }
    }

    /// The writer runs on its own thread, on the system clock.
    pub fn threaded() -> Self {
        let system: Arc<dyn Clock> = Arc::new(SystemClock::new());
        let fs = SimFs::new(system.clone());
        let clock = Arc::default();
        Self {
            fs,
            clock,
            store_clock: system,
            driver: Driver::Threads,
        }
    }

    pub fn root(&self) -> &Path {
        Path::new("/data")
    }

    pub fn try_open(&self) -> Result<Store, OpenError> {
        Store::open_with(
            self.root(),
            self.fs.clone(),
            self.store_clock.clone(),
            self.driver,
        )
    }

    pub fn open(&self) -> Store {
        self.try_open().expect("store opens")
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root().join(relative)
    }

    /// The doc's files, by name.
    pub fn doc_files(&self, doc: &DocId) -> Vec<String> {
        let dir = self.path("docs").join(doc.as_str());
        let files = self.fs.files();
        files
            .keys()
            .filter(|p| p.parent() == Some(&dir))
            .map(|p| name(p))
            .collect()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert_eq!(
                self.fs.violations(),
                Vec::<String>::new(),
                "append-only violations"
            );
        }
    }
}

pub(crate) fn doc(id: &str) -> DocId {
    DocId::new(id).unwrap()
}

pub(crate) fn name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

pub(crate) fn soon() -> Instant {
    Instant::now() + Duration::from_secs(10)
}

/// Asserts that `store` holds exactly the doc built from `updates`.
pub(crate) fn assert_doc(store: &Store, id: &DocId, updates: &[Vec<u8>]) {
    let loaded = doc_from([store.load(id).expect("doc loads").as_slice()]);
    let expected = doc_from(updates.iter().map(Vec::as_slice));
    assert_eq!(fingerprint(&loaded), fingerprint(&expected), "doc {id}");
}
