//! The SQLite index derived from the Y.Docs (DATA-5): page metadata, title and block text
//! search, and backlinks. See `ARCHITECTURE.md`, "The index".

mod db;
mod query;
mod worker;

#[cfg(test)]
pub(crate) mod gate;
#[cfg(not(test))]
pub(crate) mod gate {
    #[derive(Default)]
    pub(crate) struct Gate;

    impl Gate {
        pub(super) fn pass(&self, _point: super::HoldPoint) {}
        pub(super) fn open_all(&self) {}
    }
}
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rusqlite::Connection;

use crate::storage::DocId;
use crate::workspace::body::BlockId;
use crate::workspace::records::PageId;

pub use db::{CacheProblem, CacheState, FILE};

/// The index's state, as the shell reports it (SHELL-12).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexStatus {
    /// Every launch starts here until the background reconcile commits.
    Rebuilding,
    Ready,
    /// The reason names files, pages, and errors, never note text.
    Failed {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexEvent {
    /// The initial Rebuilding status and every later status change.
    Status(IndexStatus),
    /// What the startup check found in the index file.
    Cache(CacheState),
    /// A batch committed, so search and backlink results may differ.
    Changed(IndexChange),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexChange {
    /// The batch applied workspace doc changes: titles, the tree, or trash.
    pub workspace: bool,
    /// Pages whose body projection was replaced, sorted.
    pub pages: Vec<PageId>,
}

/// A query's answer. `Indexing` while the index cannot answer from current data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply<T> {
    Indexing,
    Ready(T),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryError {
    /// The index status is Failed, for this reason.
    Failed(String),
    Sql(String),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(reason) => write!(f, "the index failed: {reason}"),
            Self::Sql(e) => write!(f, "index query error: {e}"),
        }
    }
}

impl std::error::Error for QueryError {}

/// One page that matches a search, by title, by a block, or both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub page_id: PageId,
    /// The raw title, highlighted where the query matched it.
    pub title: Vec<TextPart>,
    /// The best matching block, if any block matched.
    pub block: Option<BlockHit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockHit {
    pub block_id: BlockId,
    pub text: Vec<TextPart>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextPart {
    pub text: String,
    pub highlighted: bool,
}

/// One active page linking to the target, with each linking block's text (LINK-7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backlink {
    pub page_id: PageId,
    pub title: String,
    pub blocks: Vec<BacklinkBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BacklinkBlock {
    pub block_id: BlockId,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HoldPoint {
    BeforeLoad(DocId),
    BeforeCommit,
}

pub(crate) trait Source: Send + Sync + 'static {
    fn load(&self, doc: &DocId) -> io::Result<Vec<u8>>;
}

type Listener = Arc<dyn Fn(&IndexEvent) + Send + Sync>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Config {
    pub first_edit_delay: Duration,
}

impl Config {
    pub const DEFAULT: Self = Self {
        first_edit_delay: Duration::from_millis(25),
    };
}

pub(crate) struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    reader: Mutex<Option<Connection>>,
    listeners: Vec<Listener>,
    config: Config,
    pub gate: Arc<gate::Gate>,
}

struct State {
    status: IndexStatus,
    workspace_dirty: bool,
    workspace_in_flight: bool,
    workspace_generation: u64,
    pending_bodies: BTreeMap<DocId, Instant>,
    busy: bool,
    shutdown: bool,
}

pub(crate) struct Index {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl Index {
    pub fn start(
        source: Arc<dyn Source>,
        path: PathBuf,
        config: Config,
        listener: Listener,
        gate: Arc<gate::Gate>,
    ) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                status: IndexStatus::Rebuilding,
                workspace_dirty: false,
                workspace_in_flight: false,
                workspace_generation: 0,
                pending_bodies: BTreeMap::new(),
                busy: true,
                shutdown: false,
            }),
            changed: Condvar::new(),
            reader: Mutex::new(None),
            listeners: vec![listener],
            config,
            gate,
        });
        let worker = shared.clone();
        let worker = std::thread::Builder::new()
            .name("aelomi-index".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || worker::run(&worker, &*source, &path))
            .expect("spawn the index worker");
        Self {
            shared,
            worker: Some(worker),
        }
    }

    pub fn mark(&self, doc: &DocId) {
        let mut state = self.shared.lock();
        if *doc == DocId::workspace() {
            state.workspace_dirty = true;
            state.workspace_generation += 1;
        } else {
            state
                .pending_bodies
                .entry(doc.clone())
                .or_insert_with(Instant::now);
        }
        drop(state);
        self.shared.changed.notify_all();
    }

    pub fn status(&self) -> IndexStatus {
        self.shared.lock().status.clone()
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Reply<Vec<SearchHit>>, QueryError> {
        let expression = query::match_expression(query);
        self.shared.read_if_current(|conn| match &expression {
            Some(expression) => query::search(conn, expression, limit.clamp(1, 200)),
            None => Ok(Vec::new()),
        })
    }

    pub fn backlinks(&self, target: &PageId) -> Result<Reply<Vec<Backlink>>, QueryError> {
        self.shared
            .read_if_current(|conn| query::backlinks(conn, target))
    }

    #[cfg(test)]
    pub fn shared(&self) -> &Shared {
        &self.shared
    }
}

impl Drop for Index {
    fn drop(&mut self) {
        self.shared.lock().shutdown = true;
        self.shared.changed.notify_all();
        self.shared.gate.open_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_reader(&self) -> MutexGuard<'_, Option<Connection>> {
        self.reader.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn read_if_current<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>,
    ) -> Result<Reply<T>, QueryError> {
        let Some(generation) = self.fresh()? else {
            return Ok(Reply::Indexing);
        };
        let mut reader = self.lock_reader();
        let Some(conn) = reader.as_mut() else {
            return Ok(Reply::Indexing);
        };
        let result = f(conn).map_err(|e| QueryError::Sql(e.to_string()))?;
        drop(reader);
        Ok(match self.fresh()? {
            Some(now) if now == generation => Reply::Ready(result),
            _ => Reply::Indexing,
        })
    }

    fn fresh(&self) -> Result<Option<u64>, QueryError> {
        let state = self.lock();
        match &state.status {
            IndexStatus::Failed { reason } => Err(QueryError::Failed(reason.clone())),
            IndexStatus::Rebuilding => Ok(None),
            IndexStatus::Ready if state.workspace_dirty || state.workspace_in_flight => Ok(None),
            IndexStatus::Ready => Ok(Some(state.workspace_generation)),
        }
    }

    fn emit(&self, event: &IndexEvent) {
        for listener in &self.listeners {
            let _ = panic::catch_unwind(AssertUnwindSafe(|| listener(event)));
        }
    }

    fn publish(&self, status: IndexStatus) {
        let mut state = self.lock();
        if state.status == status {
            return;
        }
        state.status = status.clone();
        drop(state);
        self.changed.notify_all();
        self.emit(&IndexEvent::Status(status));
    }

    #[cfg(test)]
    pub fn is_clean(&self) -> bool {
        let state = self.lock();
        !state.workspace_dirty && state.pending_bodies.is_empty()
    }

    #[cfg(test)]
    pub fn settle(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.lock();
        loop {
            let idle = !state.busy
                && !state.workspace_dirty
                && !state.workspace_in_flight
                && state.pending_bodies.is_empty();
            if idle {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}
