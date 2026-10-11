use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use yrs::updates::decoder::Decode;
use yrs::{ReadTxn, StateVector, Transact, Update};

use super::clock::{Clock, SystemClock};
use super::doc_id::DocId;
use super::format::{FORMAT_VERSION, encode_format, parse_format};
use super::fs::{Fs, FsLock, LockError, OsFs};
use super::layout::Layout;
use super::recover::recover;
use super::state::{DocSlot, DocState, Queued, Seq};
use super::writer::{self, Step};

/// The durable store of Y.Docs in one data folder. See `ARCHITECTURE.md` ("Storage").
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
    /// The bytes are not a Yjs v1 update this doc can apply. Nothing was stored.
    InvalidUpdate,
    Io(io::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Driver {
    /// A background thread writes. Production uses this.
    Threads,
    /// Nothing is written until a test steps the writer.
    Manual,
}

pub(crate) struct Shared {
    pub fs: Arc<dyn Fs>,
    pub clock: Arc<dyn Clock>,
    pub layout: Layout,
    driver: Driver,
    next_seq: AtomicU64,
    docs: Mutex<BTreeMap<DocId, Arc<DocSlot>>>,
    signal: Mutex<Signal>,
    wake_writer: Condvar,
    batch_done: Condvar,
}

#[derive(Default)]
struct Signal {
    work: bool,
    shutdown: bool,
    batches: u64,
}

impl Store {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, OpenError> {
        let clock = Arc::new(SystemClock::new());
        Self::open_with(root.as_ref(), Arc::new(OsFs), clock, Driver::Threads)
    }

    pub(crate) fn open_with(
        root: &Path,
        fs: Arc<dyn Fs>,
        clock: Arc<dyn Clock>,
        driver: Driver,
    ) -> Result<Self, OpenError> {
        let layout = Layout::new(root);
        let lock = prepare_folder(&*fs, &layout)?;
        let shared = Arc::new(Shared {
            fs,
            clock,
            layout,
            driver,
            next_seq: AtomicU64::new(1),
            docs: Mutex::default(),
            signal: Mutex::default(),
            wake_writer: Condvar::new(),
            batch_done: Condvar::new(),
        });
        let threads = match driver {
            Driver::Threads => {
                let writer = shared.clone();
                vec![
                    std::thread::Builder::new()
                        .name("aelomi-storage-writer".into())
                        .spawn(move || writer::run(&writer))
                        .map_err(OpenError::Io)?,
                ]
            }
            Driver::Manual => Vec::new(),
        };
        Ok(Self {
            shared,
            threads,
            _lock: lock,
        })
    }

    /// The doc's full state as one Yjs v1 update, including updates not yet durable. A doc
    /// that was never written loads as an empty doc.
    pub fn load(&self, doc: &DocId) -> io::Result<Vec<u8>> {
        self.shared.with_doc(doc, |state| {
            state
                .ydoc
                .transact()
                .encode_state_as_update_v1(&StateVector::default())
        })
    }

    /// Applies `update` to the doc in memory and queues it for the writer. Never waits on a
    /// write: the update is durable once a later batch's full sync covers it.
    pub fn append(&self, doc: &DocId, update: &[u8]) -> Result<Seq, AppendError> {
        let decoded = Update::decode_v1(update).map_err(|_| AppendError::InvalidUpdate)?;
        let applied = self.shared.with_doc(doc, |state| {
            if state.ydoc.transact_mut().apply_update(decoded).is_err() {
                return None;
            }
            let seq = Seq(self.shared.next_seq.fetch_add(1, Ordering::SeqCst));
            state.queue.push_back(Queued {
                seq,
                update: update.into(),
            });
            Some(seq)
        })?;
        match applied {
            Some(seq) => {
                self.shared.notify_writer();
                Ok(seq)
            }
            None => {
                self.shared.rebuild(doc)?;
                Err(AppendError::InvalidUpdate)
            }
        }
    }

    /// Waits until every update appended before the call is durable, or `deadline` passes.
    /// Returns the docs that still have such updates pending.
    pub fn flush(&self, deadline: Instant) -> Vec<DocId> {
        let target = Seq(self.shared.next_seq.load(Ordering::SeqCst) - 1);
        loop {
            let batches = self.shared.signal.lock().unwrap().batches;
            let pending = self.shared.pending_through(target);
            let now = Instant::now();
            if pending.is_empty() || now >= deadline {
                return pending;
            }
            match self.shared.driver {
                Driver::Manual => {
                    if let Step::Idle = writer::run_once(&self.shared) {
                        return pending;
                    }
                }
                Driver::Threads => {
                    let signal = self.shared.signal.lock().unwrap();
                    if signal.batches == batches {
                        let _wait = self.shared.batch_done.wait_timeout(signal, deadline - now);
                    }
                }
            }
        }
    }

    /// Stops the writer after its current batch and releases the folder. Returns the docs
    /// whose pending updates were not durable: call [`Store::flush`] first.
    pub fn close(mut self) -> Vec<DocId> {
        self.stop();
        self.shared.pending_through(Seq(u64::MAX))
    }

    fn stop(&mut self) {
        self.shared.signal.lock().unwrap().shutdown = true;
        self.shared.wake_writer.notify_all();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }

    #[cfg(test)]
    pub(crate) fn run_writer_once(&self) -> Step {
        writer::run_once(&self.shared)
    }

    /// Whether every update of `doc` up to `seq` is durable.
    #[cfg(test)]
    pub(crate) fn is_durable(&self, doc: &DocId, seq: Seq) -> bool {
        self.shared.pending_through(seq).binary_search(doc).is_err()
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Shared {
    pub fn slots(&self) -> Vec<(DocId, Arc<DocSlot>)> {
        let docs = self.docs.lock().unwrap();
        docs.iter()
            .map(|(id, slot)| (id.clone(), slot.clone()))
            .collect()
    }

    fn slot(&self, doc: &DocId) -> Arc<DocSlot> {
        self.docs
            .lock()
            .unwrap()
            .entry(doc.clone())
            .or_default()
            .clone()
    }

    /// Runs `f` on the doc, loading it from its files first if this is its first use.
    fn with_doc<R>(&self, doc: &DocId, f: impl FnOnce(&mut DocState) -> R) -> io::Result<R> {
        let slot = self.slot(doc);
        let mut state = slot.state.lock().unwrap();
        if state.is_none() {
            let recovered = recover(&*self.fs, &self.layout, doc)?;
            *state = Some(DocState {
                ydoc: recovered.ydoc,
                queue: VecDeque::new(),
                log: recovered.log,
            });
        }
        Ok(f(state.as_mut().expect("loaded above")))
    }

    /// Rebuilds the doc in memory from its files plus its queue, after an update that decoded
    /// but failed to apply may have left part of itself in memory only.
    fn rebuild(&self, doc: &DocId) -> io::Result<()> {
        let slot = self.slot(doc);
        let _io = slot.io.lock().unwrap();
        let mut state = slot.state.lock().unwrap();
        let state = state.as_mut().expect("append loaded the doc");
        let ydoc = recover(&*self.fs, &self.layout, doc)?.ydoc;
        for queued in &state.queue {
            let update = Update::decode_v1(&queued.update).expect("queued updates decode");
            let _ = ydoc.transact_mut().apply_update(update);
        }
        state.ydoc = ydoc;
        Ok(())
    }

    fn pending_through(&self, target: Seq) -> Vec<DocId> {
        self.slots()
            .into_iter()
            .filter(|(_, slot)| {
                let state = slot.state.lock().unwrap();
                let front = state.as_ref().and_then(|s| s.queue.front());
                front.is_some_and(|queued| queued.seq <= target)
            })
            .map(|(id, _)| id)
            .collect()
    }

    fn notify_writer(&self) {
        self.signal.lock().unwrap().work = true;
        self.wake_writer.notify_one();
    }

    pub fn batch_finished(&self) {
        self.signal.lock().unwrap().batches += 1;
        self.batch_done.notify_all();
    }

    /// Waits for new work, or until `wake_at` on the store's clock. Returns `false` once the
    /// store is shutting down.
    pub fn wait_for_work(&self, wake_at: Option<Duration>) -> bool {
        let mut signal = self.signal.lock().unwrap();
        loop {
            if signal.shutdown {
                return false;
            }
            if std::mem::take(&mut signal.work) {
                return true;
            }
            match wake_at.map(|at| at.saturating_sub(self.clock.now())) {
                Some(wait) if wait.is_zero() => return true,
                Some(wait) => signal = self.wake_writer.wait_timeout(signal, wait).unwrap().0,
                None => signal = self.wake_writer.wait(signal).unwrap(),
            }
        }
    }
}

/// Checks `format.json` before writing anything, so a folder this build can't handle is left
/// unchanged. Then takes the lock and creates what a new folder needs.
fn prepare_folder(fs: &dyn Fs, layout: &Layout) -> Result<FsLock, OpenError> {
    let root = layout.root();
    match fs.list(root) {
        Ok(names) => {
            check_format(fs, layout, &names)?;
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => create_dir_all(fs, root)?,
        Err(e) => return Err(e.into()),
    }
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
        fs.create(&tmp)?;
        fs.append(&tmp, encode_format().as_bytes())?;
        fs.fsync(&tmp)?;
        // The contents must be durable before the name points at them.
        fs.full_sync(&tmp)?;
        fs.rename(&tmp, &layout.format_json())?;
        changed = true;
    }
    if !has("docs") {
        fs.create_dir(&layout.docs())?;
        changed = true;
    }
    if changed {
        fs.fsync(root)?;
        fs.full_sync(root)?;
    }
    Ok(lock)
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

fn create_dir_all(fs: &dyn Fs, dir: &Path) -> io::Result<()> {
    match fs.create_dir(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let parent = dir.parent().ok_or(e)?;
            create_dir_all(fs, parent)?;
            fs.create_dir(dir)?;
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(()),
        result => result?,
    }
    match dir.parent() {
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
