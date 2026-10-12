mod ack;
mod compaction;
mod crash;
mod decode_guard;
mod dirs;
mod failures;
mod fixture;
mod measure;
mod publication;
mod real_disk;
mod refusals;
mod salvage;
mod snapshot_copies;
mod store;

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use yrs::updates::decoder::Decode;
use yrs::{Text, Transact};

use super::clock::{Clock, ManualClock, SystemClock};
use super::doc_id::DocId;
use super::equality::Ops;
use super::layout::{DocFile, Generation, SegmentNo};
use super::log::{self, LogContents, LogFormat, SEGMENT_HEADER};
use super::sim_fs::SimFs;
use super::snapshot::{self, Condition, SnapshotFormat};
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

    pub fn quarantine(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        let root = self.path("quarantine");
        let files = self.fs.files().into_iter();
        files.filter(|(path, _)| path.starts_with(&root)).collect()
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

pub(crate) fn applied(bytes: &[u8], case: &str) -> (Ops, yrs::Doc) {
    let update = yrs::Update::decode_v1(bytes).unwrap_or_else(|e| panic!("{case}: {e}"));
    let held = Ops::of_update(&update);
    let doc = yrs::Doc::new();
    doc.transact_mut()
        .apply_update(update)
        .unwrap_or_else(|e| panic!("{case}: {e}"));
    (held, doc)
}

pub(crate) fn typed(client: u64, n: usize) -> Vec<u8> {
    let client = yrs::Doc::with_client_id(client);
    let text = client.get_or_insert_text("t");
    let mut txn = client.transact_mut();
    text.insert(&mut txn, 0, &"x".repeat(n));
    txn.encode_update_v1()
}

pub(crate) fn with_a_loner_sixth(mut updates: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let client = yrs::Doc::with_client_id(999_999_999);
    let text = client.get_or_insert_text("loner");
    let mut txn = client.transact_mut();
    text.insert(&mut txn, 0, "alone");
    updates.insert(5, txn.encode_update_v1());
    updates
}

pub(crate) fn without(updates: &[Vec<u8>], lost: &[usize]) -> Vec<Vec<u8>> {
    let numbered = updates.iter().enumerate();
    let kept = numbered.filter(|(i, _)| !lost.contains(i));
    kept.map(|(_, update)| update.clone()).collect()
}

pub(crate) fn snapshot_payload<'a>(name: &str, bytes: &'a [u8]) -> &'a [u8] {
    let Some(DocFile::Snapshot(_, format)) = DocFile::parse(name) else {
        panic!("{name} is not a snapshot")
    };
    match snapshot::read(format, bytes) {
        Some((payload, Condition::Whole)) => payload,
        read => panic!("{name} is not a whole snapshot: {read:?}"),
    }
}

pub(crate) fn log_name(generation: u64, segment: u64) -> String {
    DocFile::Log(
        Generation(generation),
        SegmentNo(segment),
        LogFormat::WRITTEN,
    )
    .name()
}

pub(crate) fn read_log_file(path: &Path, bytes: &[u8]) -> LogContents {
    let name = name(path);
    let Some(DocFile::Log(.., format)) = DocFile::parse(&name) else {
        panic!("{name} is not a log")
    };
    log::read(format, bytes)
}

pub(crate) fn v2_log(updates: &[impl AsRef<[u8]>]) -> Vec<u8> {
    let mut out = Vec::new();
    for update in updates {
        log::encode_frame(update.as_ref(), &mut out);
    }
    out
}

/// What compaction writes once the store has written a segment this lifetime.
pub(crate) fn snapshot_name(generation: u64) -> String {
    DocFile::Snapshot(Generation(generation), SnapshotFormat::V2).name()
}

/// What compaction writes before the store's first segment of a lifetime.
pub(crate) fn v1_snapshot_name(generation: u64) -> String {
    DocFile::Snapshot(Generation(generation), SnapshotFormat::V1).name()
}

pub(crate) fn snapshot_tmp_name(generation: u64) -> String {
    DocFile::SnapshotTmp(Generation(generation), SnapshotFormat::V2).name()
}

pub(crate) struct Frame {
    pub head: Range<usize>,
    pub body: Range<usize>,
    pub tail: Range<usize>,
}

pub(crate) fn frames(updates: &[impl AsRef<[u8]>]) -> Vec<Frame> {
    let mut at = 0;
    updates
        .iter()
        .map(|update| {
            let n = update.as_ref().len();
            let head = at..at + 20;
            let body = head.end..head.end + n + n.div_ceil(7);
            let tail = body.end..body.end + 20;
            at = tail.end;
            Frame { head, body, tail }
        })
        .collect()
}

impl Frame {
    fn head_field(&self, byte: usize) -> usize {
        self.head.start + byte + byte / 7
    }

    pub fn length_byte(&self, byte: usize) -> usize {
        self.head_field(1 + byte)
    }

    pub fn crc_byte(&self, byte: usize) -> usize {
        self.head_field(5 + byte)
    }
}

pub(crate) fn v1_record_at(updates: &[Vec<u8>], n: usize) -> usize {
    SEGMENT_HEADER + updates[..n].iter().map(|u| 8 + u.len()).sum::<usize>()
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
