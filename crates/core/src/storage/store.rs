use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use yrs::updates::decoder::Decode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};

use super::clock::{Clock, SystemClock};
use super::compact;
use super::doc_id::DocId;
use super::format::{FORMAT_VERSION, encode_format, parse_format};
use super::fs::{Fs, FsLock, LockError, OsFs, create_durable};
use super::health::{FailReason, Health};
use super::layout::Layout;
use super::recover::recover;
use super::state::{
    Compaction, DocSlot, DocState, Limits, Queued, Seq, Ydoc, lock_ignoring_poison,
};
use super::writer;

/// The durable store of Y.Docs in one data folder. See `ARCHITECTURE.md`, "Data model and storage".
pub struct Store {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
    _lock: FsLock,
}

#[derive(Debug)]
pub enum OpenError {
    /// Another process has the data folder open.
    Locked,
    /// The folder was written by a newer build. Nothing was written to it.
    NewerFormat {
        found: u32,
        supported: u32,
    },
    /// The folder has data but no readable `format.json`.
    UnknownFormat,
    Io(io::Error),
}

#[derive(Debug)]
pub enum AppendError {
    /// The bytes are not a Yjs v1 update this doc can apply, or are more than a log record
    /// holds. Nothing was stored.
    InvalidUpdate,
    Io(io::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Driver {
    Threads,
    #[cfg(test)]
    Manual,
}

pub(crate) struct Shared {
    pub fs: Arc<dyn Fs>,
    pub clock: Arc<dyn Clock>,
    pub layout: Layout,
    pub limits: Limits,
    driver: Driver,
    next_seq: AtomicU64,
    pub quarantine_exists: AtomicBool,
    docs: Mutex<BTreeMap<DocId, Arc<DocSlot>>>,
    signal: Mutex<Signal>,
    wake_writer: Condvar,
    wake_compactor: Condvar,
    batch_done: Condvar,
    health: Mutex<Health>,
    subscribers: Mutex<Vec<Subscriber>>,
}

type Subscriber = Arc<dyn Fn(&Health) + Send + Sync>;

#[derive(Default)]
struct Signal {
    work: bool,
    retry_failing: bool,
    shutdown: bool,
    batches: u64,
    compactions: VecDeque<DocId>,
}

impl Store {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, OpenError> {
        let clock = Arc::new(SystemClock::new());
        let fs = Arc::new(OsFs);
        Self::open_with(root.as_ref(), fs, clock, Driver::Threads, Limits::DATA_6)
    }

    pub(crate) fn open_with(
        root: &Path,
        fs: Arc<dyn Fs>,
        clock: Arc<dyn Clock>,
        driver: Driver,
        limits: Limits,
    ) -> Result<Self, OpenError> {
        let layout = Layout::new(root);
        let (lock, quarantine_exists) = prepare_folder(&*fs, &layout)?;
        let shared = Arc::new(Shared {
            fs,
            clock,
            layout,
            limits,
            driver,
            next_seq: AtomicU64::new(1),
            quarantine_exists: AtomicBool::new(quarantine_exists),
            docs: Mutex::default(),
            signal: Mutex::default(),
            wake_writer: Condvar::new(),
            wake_compactor: Condvar::new(),
            batch_done: Condvar::new(),
            health: Mutex::new(Health::Healthy),
            subscribers: Mutex::default(),
        });
        let mut threads = Vec::new();
        if driver == Driver::Threads {
            let writer = shared.clone();
            let compactor = shared.clone();
            let spawn = |name: &str, f: Box<dyn FnOnce() + Send>| {
                std::thread::Builder::new().name(name.into()).spawn(f)
            };
            threads.push(spawn(
                "aelomi-storage-writer",
                Box::new(move || writer::run(&writer)),
            )?);
            threads.push(spawn(
                "aelomi-storage-compactor",
                Box::new(move || compact::run(&compactor)),
            )?);
        }
        Ok(Self {
            shared,
            threads,
            _lock: lock,
        })
    }

    /// The doc's full state as one Yjs v1 update, including updates not yet durable. A doc
    /// that was never written loads as an empty doc.
    pub fn load(&self, doc: &DocId) -> io::Result<Vec<u8>> {
        self.shared.with_doc(doc, |ydoc, _| {
            ydoc.transact()
                .encode_state_as_update_v1(&StateVector::default())
        })
    }

    /// Applies `update` to the doc in memory and queues it for the writer. Never waits on a
    /// write: the update is durable once a later batch's full sync covers it.
    ///
    /// An update that decodes but fails to apply is rejected. yrs may have applied part of it,
    /// so the doc is rebuilt from its files and queue before it is used again.
    pub fn append(&self, doc: &DocId, update: &[u8]) -> Result<Seq, AppendError> {
        if update.len() > self.shared.limits.max_payload as usize {
            return Err(AppendError::InvalidUpdate);
        }
        let decoded = Update::decode_v1(update).map_err(|_| AppendError::InvalidUpdate)?;
        let seq = self.shared.with_doc(doc, |ydoc, state| {
            if ydoc.transact_mut().apply_update(decoded).is_err() {
                state.ydoc = Ydoc::Stale;
                return None;
            }
            let seq = Seq(self.shared.next_seq.fetch_add(1, Ordering::SeqCst));
            state.queue.push_back(Queued {
                seq,
                update: update.into(),
            });
            Some(seq)
        })?;
        let seq = seq.ok_or(AppendError::InvalidUpdate)?;
        self.shared.notify_writer();
        Ok(seq)
    }

    /// Waits until every update appended before the call is durable, or `deadline` passes.
    /// Returns the docs that still have such updates pending. A doc whose writes are failing
    /// retries once at the start, whatever its backoff.
    pub fn flush(&self, deadline: Instant) -> Vec<DocId> {
        let target = Seq(self.shared.next_seq.load(Ordering::SeqCst) - 1);
        lock_ignoring_poison(&self.shared.signal).retry_failing = true;
        self.shared.notify_writer();
        loop {
            let batches = lock_ignoring_poison(&self.shared.signal).batches;
            let pending = self.shared.pending_through(target);
            let now = Instant::now();
            if pending.is_empty() || now >= deadline {
                return pending;
            }
            match self.shared.driver {
                #[cfg(test)]
                Driver::Manual => {
                    if let writer::Step::Idle { .. } = writer::run_once(&self.shared) {
                        return pending;
                    }
                }
                Driver::Threads => {
                    let signal = lock_ignoring_poison(&self.shared.signal);
                    if signal.batches == batches {
                        let _wait = self.shared.batch_done.wait_timeout(signal, deadline - now);
                    }
                }
            }
        }
    }

    pub fn health(&self) -> Health {
        lock_ignoring_poison(&self.shared.health).clone()
    }

    /// Calls `f` with the new health each time it changes, from the writer's thread. A panic in
    /// `f` stops neither the writer nor the other subscribers.
    pub fn subscribe_health(&self, f: impl Fn(&Health) + Send + Sync + 'static) {
        lock_ignoring_poison(&self.shared.subscribers).push(Arc::new(f));
    }

    /// Flushes until `deadline`, then stops the threads and releases the folder. If updates are
    /// still pending, nothing is discarded: the store comes back with the docs that have them.
    pub fn close(self, deadline: Instant) -> Result<(), (Store, Vec<DocId>)> {
        let pending = self.flush(deadline);
        if pending.is_empty() {
            Ok(())
        } else {
            Err((self, pending))
        }
    }

    #[cfg(test)]
    pub(crate) fn run_writer_once(&self) -> writer::Step {
        writer::run_once(&self.shared)
    }

    #[cfg(test)]
    pub(crate) fn run_compactor_once(&self) -> Option<(DocId, Result<(), compact::CompactError>)> {
        let doc = lock_ignoring_poison(&self.shared.signal)
            .compactions
            .pop_front()?;
        let result = compact::compact_queued(&self.shared, &doc);
        Some((doc, result))
    }

    #[cfg(test)]
    pub(crate) fn queued_compactions(&self) -> Vec<DocId> {
        lock_ignoring_poison(&self.shared.signal)
            .compactions
            .iter()
            .cloned()
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn compact(&self, doc: &DocId) -> Result<(), compact::CompactError> {
        let rotated = self.begin_compaction(doc)?;
        self.finish_compaction(doc, rotated)
    }

    #[cfg(test)]
    pub(crate) fn begin_compaction(&self, doc: &DocId) -> io::Result<Option<compact::Rotated>> {
        let was = self.shared.with_doc(doc, |_, state| {
            std::mem::replace(&mut state.compaction, Compaction::Queued)
        })?;
        if was == Compaction::Queued {
            lock_ignoring_poison(&self.shared.signal)
                .compactions
                .retain(|d| d != doc);
        }
        Ok(compact::rotate(&self.shared, doc))
    }

    #[cfg(test)]
    pub(crate) fn finish_compaction(
        &self,
        doc: &DocId,
        rotated: Option<compact::Rotated>,
    ) -> Result<(), compact::CompactError> {
        compact::finish(&self.shared, doc, rotated)
    }

    #[cfg(test)]
    pub(crate) fn panic_while_applying(&self, doc: &DocId, update: &[u8]) {
        let slot = self.shared.slot(doc);
        let update = update.to_vec();
        let _ = std::thread::spawn(move || {
            let state = slot.state();
            if let Some(Ydoc::Fresh(ydoc)) = state.as_ref().map(|s| &s.ydoc) {
                let update = Update::decode_v1(&update).expect("a valid update");
                ydoc.transact_mut().apply_update(update).unwrap();
            }
            panic!("yrs panicked while applying an update");
        })
        .join();
    }

    #[cfg(test)]
    pub(crate) fn ops(&self, doc: &DocId) -> super::equality::Ops {
        self.shared
            .with_doc(doc, |ydoc, _| {
                super::equality::Ops::of_doc(&ydoc.transact())
            })
            .expect("doc loads")
            .expect("yrs decodes its own encoding")
    }

    #[cfg(test)]
    pub(crate) fn is_durable(&self, doc: &DocId, seq: Seq) -> bool {
        self.shared.pending_through(seq).binary_search(doc).is_err()
    }
}

/// Stops the threads and drops any update still pending, so dropping a store is for process
/// exit. Otherwise use [`Store::close`].
impl Drop for Store {
    fn drop(&mut self) {
        lock_ignoring_poison(&self.shared.signal).shutdown = true;
        self.shared.wake_writer.notify_all();
        self.shared.wake_compactor.notify_all();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("root", &self.shared.layout.root())
            .finish_non_exhaustive()
    }
}

impl Shared {
    pub fn slots(&self) -> Vec<(DocId, Arc<DocSlot>)> {
        let docs = lock_ignoring_poison(&self.docs);
        docs.iter()
            .map(|(id, slot)| (id.clone(), slot.clone()))
            .collect()
    }

    pub fn slot(&self, doc: &DocId) -> Arc<DocSlot> {
        lock_ignoring_poison(&self.docs)
            .entry(doc.clone())
            .or_default()
            .clone()
    }

    fn with_doc<R>(&self, doc: &DocId, f: impl FnOnce(&Doc, &mut DocState) -> R) -> io::Result<R> {
        let slot = self.slot(doc);
        loop {
            {
                let mut state = slot.state();
                if state.is_none() {
                    let recovered = recover(&*self.fs, &self.layout, doc, self.limits)?;
                    *state = Some(DocState {
                        ydoc: Ydoc::Fresh(recovered.ydoc),
                        queue: VecDeque::new(),
                        log: recovered.log,
                        failure: None,
                        corrupt: recovered.corrupt,
                        compaction: Compaction::Idle,
                    });
                    if recovered.needs_compaction {
                        self.queue_compaction(doc, state.as_mut().expect("loaded above"));
                    }
                }
                let state = state.as_mut().expect("loaded above");
                if let Ydoc::Fresh(ydoc) = state.ydoc.clone() {
                    return Ok(f(&ydoc, state));
                }
            }
            self.rebuild(&slot, doc)?;
        }
    }

    fn rebuild(&self, slot: &DocSlot, doc: &DocId) -> io::Result<()> {
        let _io = slot.files();
        let mut state = slot.state();
        let state = state.as_mut().expect("only a loaded doc goes stale");
        if let Ydoc::Fresh(_) = state.ydoc {
            return Ok(());
        }
        let recovered = recover(&*self.fs, &self.layout, doc, self.limits)?;
        for queued in &state.queue {
            let update = Update::decode_v1(&queued.update).expect("queued updates decode");
            let _ = recovered.ydoc.transact_mut().apply_update(update);
        }
        state.ydoc = Ydoc::Fresh(recovered.ydoc);
        state.corrupt.extend(recovered.corrupt);
        if recovered.needs_compaction {
            self.queue_compaction(doc, state);
        }
        Ok(())
    }

    fn pending_through(&self, target: Seq) -> Vec<DocId> {
        self.slots()
            .into_iter()
            .filter(|(_, slot)| {
                let state = slot.state();
                let front = state.as_ref().and_then(|s| s.queue.front());
                front.is_some_and(|queued| queued.seq <= target)
            })
            .map(|(id, _)| id)
            .collect()
    }

    pub fn refresh_health(&self, latest: Option<FailReason>) {
        let mut failing = Vec::new();
        let mut reason = latest;
        for (id, slot) in self.slots() {
            let state = slot.state();
            if let Some(failure) = state.as_ref().and_then(|s| s.failure.as_ref()) {
                reason.get_or_insert_with(|| failure.reason.clone());
                failing.push(id);
            }
        }
        let mut health = lock_ignoring_poison(&self.health);
        let next = match (reason, &*health) {
            (Some(reason), Health::Failing { since, .. }) if !failing.is_empty() => {
                Health::Failing {
                    reason,
                    since: *since,
                    pending_docs: failing,
                }
            }
            (Some(reason), Health::Healthy) if !failing.is_empty() => Health::Failing {
                reason,
                since: self.clock.now(),
                pending_docs: failing,
            },
            _ => Health::Healthy,
        };
        if next == *health {
            return;
        }
        *health = next.clone();
        drop(health);
        let subscribers = lock_ignoring_poison(&self.subscribers).clone();
        for subscriber in subscribers {
            let _ = panic::catch_unwind(AssertUnwindSafe(|| subscriber(&next)));
        }
    }

    pub fn sync_quarantine_then_doc_dir(&self, doc: &DocId) -> io::Result<()> {
        if self.quarantine_exists.load(Ordering::SeqCst) {
            match self.fs.fsync(&self.layout.quarantine_dir(doc)) {
                Ok(()) => {
                    self.fs.fsync(&self.layout.quarantine())?;
                    self.fs.fsync(self.layout.root())?;
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        self.fs.fsync(&self.layout.doc_dir(doc))
    }

    pub fn queue_compaction(&self, doc: &DocId, state: &mut DocState) {
        match state.compaction {
            Compaction::Idle => {
                state.compaction = Compaction::Queued;
                lock_ignoring_poison(&self.signal)
                    .compactions
                    .push_back(doc.clone());
                self.wake_compactor.notify_one();
            }
            Compaction::Queued => {}
            Compaction::Running { .. } => {
                state.compaction = Compaction::Running {
                    asked_after_rotate: true,
                }
            }
        }
    }

    pub fn compaction_ended(&self, doc: &DocId) {
        let slot = self.slot(doc);
        let mut state = slot.state();
        let state = state.as_mut().expect("compaction runs on loaded docs");
        let ended = std::mem::replace(&mut state.compaction, Compaction::Idle);
        if matches!(
            ended,
            Compaction::Running {
                asked_after_rotate: true
            }
        ) {
            self.queue_compaction(doc, state);
        }
    }

    pub fn wait_for_compaction(&self) -> Option<DocId> {
        let mut signal = lock_ignoring_poison(&self.signal);
        loop {
            if signal.shutdown {
                return None;
            }
            if let Some(doc) = signal.compactions.pop_front() {
                return Some(doc);
            }
            signal = self
                .wake_compactor
                .wait(signal)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    pub fn is_shutting_down(&self) -> bool {
        lock_ignoring_poison(&self.signal).shutdown
    }

    fn notify_writer(&self) {
        lock_ignoring_poison(&self.signal).work = true;
        self.wake_writer.notify_one();
    }

    pub fn take_retry_failing(&self) -> bool {
        std::mem::take(&mut lock_ignoring_poison(&self.signal).retry_failing)
    }

    pub fn batch_finished(&self) {
        lock_ignoring_poison(&self.signal).batches += 1;
        self.batch_done.notify_all();
    }

    pub fn wait_for_work(&self, wake_at: Option<Duration>) -> bool {
        let mut signal = lock_ignoring_poison(&self.signal);
        loop {
            if signal.shutdown {
                return false;
            }
            if std::mem::take(&mut signal.work) {
                return true;
            }
            match wake_at.map(|at| at.saturating_sub(self.clock.now())) {
                Some(wait) if wait.is_zero() => return true,
                Some(wait) => {
                    signal = self
                        .wake_writer
                        .wait_timeout(signal, wait)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
                None => {
                    signal = self
                        .wake_writer
                        .wait(signal)
                        .unwrap_or_else(PoisonError::into_inner)
                }
            }
        }
    }
}

fn prepare_folder(fs: &dyn Fs, layout: &Layout) -> Result<(FsLock, bool), OpenError> {
    let root = layout.root();
    refuse_before_writing(fs, layout)?;
    create_dir_all_synced(fs, root)?;
    let lock = fs.lock(&layout.lock()).map_err(|e| match e {
        LockError::Held => OpenError::Locked,
        LockError::Io(e) => OpenError::Io(e),
    })?;
    let names = fs.list(root)?;
    let has = |name: &str| names.iter().any(|n| n == name);
    let mut changed = false;
    if check_format(fs, layout, &names)? == Format::Missing {
        let tmp = layout.format_json_tmp();
        if has("format.json.tmp") {
            fs.remove(&tmp)?;
        }
        create_durable(fs, &tmp, encode_format().as_bytes())?;
        fs.rename(&tmp, &layout.format_json())?;
        changed = true;
    }
    if !has("docs") {
        fs.create_dir(&layout.docs())?;
        changed = true;
    }
    fs.fsync(root)?;
    if changed {
        fs.full_sync(root)?;
    }
    Ok((lock, has("quarantine")))
}

/// Refuses a folder this build can't handle before anything, even `.lock`, is written to it.
fn refuse_before_writing(fs: &dyn Fs, layout: &Layout) -> Result<(), OpenError> {
    match fs.list(layout.root()) {
        Ok(names) => check_format(fs, layout, &names).map(drop),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Format {
    Current,
    Missing,
}

fn check_format(fs: &dyn Fs, layout: &Layout, names: &[String]) -> Result<Format, OpenError> {
    let has = |name: &str| names.iter().any(|n| n == name);
    if !has("format.json") {
        return if has("docs") {
            Err(OpenError::UnknownFormat)
        } else {
            Ok(Format::Missing)
        };
    }
    match parse_format(&fs.read(&layout.format_json())?) {
        Some(FORMAT_VERSION) => Ok(Format::Current),
        Some(found) if found > FORMAT_VERSION => Err(OpenError::NewerFormat {
            found,
            supported: FORMAT_VERSION,
        }),
        _ => Err(OpenError::UnknownFormat),
    }
}

fn create_dir_all_synced(fs: &dyn Fs, dir: &Path) -> io::Result<()> {
    match fs.create_dir(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            create_dir_all_synced(fs, dir.parent().ok_or(e)?)?;
            fs.create_dir(dir)?;
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Ok(()) => sync_entry(fs, dir.parent().expect("created inside a parent"))?,
        Err(e) => return Err(e),
    }
    sync_entry(fs, dir)
}

fn sync_entry(fs: &dyn Fs, path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) => fs.fsync(parent),
        None => Ok(()),
    }
}

impl From<io::Error> for OpenError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<io::Error> for AppendError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locked => f.write_str("another process has the data folder open"),
            Self::NewerFormat { found, supported } => write!(
                f,
                "the data folder has format {found}, newer than this build's format {supported}"
            ),
            Self::UnknownFormat => f.write_str("the data folder has no readable format.json"),
            Self::Io(e) => write!(f, "could not open the data folder: {e}"),
        }
    }
}

impl fmt::Display for AppendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUpdate => f.write_str("not a Yjs v1 update this doc can apply"),
            Self::Io(e) => write!(f, "could not load the doc: {e}"),
        }
    }
}

impl std::error::Error for OpenError {}
impl std::error::Error for AppendError {}
