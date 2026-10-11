mod ack;
mod compaction;
mod crash;
mod dirs;
mod failures;
mod fixture;
mod measure;
mod real_disk;
mod refusals;
mod store;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use yrs::updates::decoder::Decode;

use super::clock::{Clock, ManualClock, SystemClock};
use super::doc_id::DocId;
use super::equality::Ops;
use super::sim_fs::SimFs;
use super::state::Limits;
use super::store::{Driver, OpenError, Store};
use super::testkit::{SplitMix64, UpdateGen, doc_from, fingerprint};

pub(crate) struct Env {
    pub fs: Arc<SimFs>,
    pub clock: Arc<ManualClock>,
    pub limits: Limits,
    pub root: PathBuf,
    store_clock: Arc<dyn Clock>,
    driver: Driver,
}

impl Env {
    pub fn manual() -> Self {
        let clock = Arc::new(ManualClock::default());
        let fs = SimFs::new(clock.clone());
        Self {
            fs,
            store_clock: clock.clone(),
            clock,
            limits: Limits::DATA_6,
            root: PathBuf::from("/data"),
            driver: Driver::Manual,
        }
    }

    pub fn threaded() -> Self {
        let system: Arc<dyn Clock> = Arc::new(SystemClock::new());
        let fs = SimFs::new(system.clone());
        let clock = Arc::default();
        Self {
            fs,
            clock,
            limits: Limits::DATA_6,
            root: PathBuf::from("/data"),
            store_clock: system,
            driver: Driver::Threads,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn try_open(&self) -> Result<Store, OpenError> {
        Store::open_with(
            self.root(),
            self.fs.clone(),
            self.store_clock.clone(),
            self.driver,
            self.limits,
        )
    }

    pub fn open(&self) -> Store {
        self.try_open().expect("store opens")
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root().join(relative)
    }

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

pub(crate) fn assert_doc(store: &Store, id: &DocId, updates: &[Vec<u8>]) {
    let loaded = doc_from([store.load(id).expect("doc loads").as_slice()]);
    let expected = doc_from(updates.iter().map(Vec::as_slice));
    assert_eq!(fingerprint(&loaded), fingerprint(&expected), "doc {id}");
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Delivery {
    Shuffled,
    OneWithheld,
}

pub(crate) fn out_of_order(seed: u64, n: usize, delivery: Delivery) -> Vec<Vec<u8>> {
    let mut rng = SplitMix64::new(seed);
    let mut updates = UpdateGen::new(seed, 3).updates(n);
    match delivery {
        Delivery::Shuffled => {
            for i in (1..n).rev() {
                updates.swap(i, rng.below(i as u64 + 1) as usize);
            }
        }
        Delivery::OneWithheld => {
            updates.remove(rng.below(n as u64 / 2) as usize);
        }
    }
    updates
}

pub(crate) fn in_order(seed: u64, n: usize) -> yrs::Doc {
    doc_from(UpdateGen::new(seed, 3).updates(n).iter().map(Vec::as_slice))
}

/// A doc can hold more deletions: yrs deletes what it integrates into a deleted parent.
pub(crate) fn ops_of_updates(updates: &[Vec<u8>]) -> Ops {
    let mut ops = Ops::default();
    for update in updates {
        let update = yrs::Update::decode_v1(update).expect("a valid update");
        ops.merge(Ops::of_update(&update));
    }
    ops
}
