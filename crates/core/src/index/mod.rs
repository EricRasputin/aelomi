//! The SQLite index derived from the Y.Docs (DATA-5): page metadata, title and block text
//! search, and backlinks. See `ARCHITECTURE.md`, "The index".

mod db;
mod highlight;
mod query;
mod tokenizer;
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

use rusqlite::{Connection, ErrorCode};

use crate::storage::DocId;
use crate::storage::clock::{Clock, SystemClock};
use crate::workspace::body::BlockId;
use crate::workspace::records::PageId;
use query::QueryFault;

pub use db::{CacheProblem, CacheState, CheckTimings, Corrupt, FILE};

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
    /// How long the startup check and the first reconcile took, once per rebuild.
    Startup(Box<StartupReport>),
}

/// Content-free timings and counts of one startup or rebuild, from the worker's start to Ready.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartupReport {
    pub check: CheckTimings,
    pub workspace_load: Duration,
    pub workspace_decode: Duration,
    pub workspace_forest: Duration,
    pub workspace_sql: Duration,
    pub pages: usize,
    pub loaded_bytes: usize,
    pub page_load: Duration,
    pub page_decode: Duration,
    pub page_project: Duration,
    pub page_sql: Duration,
    pub publish: Duration,
    pub total: Duration,
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
    /// `issues` lists every active page whose body the index could read only in part, sorted by
    /// page ID, whatever the query.
    Ready {
        value: T,
        issues: Arc<[PageIssue]>,
    },
}

/// What [`Workspace::search`](crate::workspace::Workspace::search) looks for (CMD-3, CMD-5).
#[derive(Clone, Copy, Debug)]
pub struct SearchQuery<'a> {
    pub text: &'a str,
    /// Clamped to 1 through [`MAX_LIMIT`].
    pub limit: usize,
    pub scope: SearchScope<'a>,
}

#[derive(Clone, Copy, Debug)]
pub enum SearchScope<'a> {
    /// Pages whose title matches, then pages whose body matches.
    All,
    /// Pages whose body matches, except `exclude`: the palette's title group, at most
    /// [`MAX_EXCLUDED`] distinct pages.
    Content { exclude: &'a [PageId] },
}

pub const MAX_LIMIT: usize = 200;
pub const MAX_EXCLUDED: usize = 200;

/// An active page whose body the index holds only in part. The page's source doc is unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageIssue {
    pub page_id: PageId,
    /// The most serious problem found in the body.
    pub problem: PageProblem,
    /// The blocks the index could read.
    pub indexed_blocks: u32,
}

/// Ordered most serious first.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageProblem {
    /// The doc is not a valid Yjs update.
    Undecodable,
    /// The body is under this root, not `prosemirror`.
    UnrecognizedRoot(String),
    /// The `prosemirror` body holds content that is not XML. That content was skipped.
    NotAFragment,
    /// A block has no string ID. Its children were read.
    MissingBlockId,
    /// A page link has no valid target. It reads as a space.
    InvalidPageLink,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryError {
    /// The index status is Failed, for this reason.
    Failed(String),
    InvalidQuery(InvalidQuery),
    Sql(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvalidQuery {
    TooManyExcluded { max: usize },
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(reason) => write!(f, "the index failed: {reason}"),
            Self::InvalidQuery(InvalidQuery::TooManyExcluded { max }) => {
                write!(f, "a search can exclude at most {max} pages")
            }
            Self::Sql(e) => write!(f, "index query error: {e}"),
        }
    }
}

impl std::error::Error for QueryError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub page_id: PageId,
    /// The raw title, highlighted where the query matched it.
    pub title: Vec<TextPart>,
    /// The page's first matching block in document order. Always present in a content search;
    /// absent for a page that matched by title.
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
    BeforeRemove,
    AfterRemovingMain,
    TreeBeforeLoad,
    TreeBeforeInstall,
}

pub(crate) trait Source: Send + Sync + 'static {
    fn load(&self, doc: &DocId) -> io::Result<Vec<u8>>;
}

type Listener = Arc<dyn Fn(&IndexEvent) + Send + Sync>;

pub(crate) type Spawn =
    fn(std::thread::Builder, Box<dyn FnOnce() + Send>) -> io::Result<JoinHandle<()>>;

#[derive(Clone)]
pub(crate) struct Config {
    pub first_edit_delay: Duration,
    pub spawn: Spawn,
    /// Times retry waits and how long the index stayed Ready. `None` is the system clock.
    pub clock: Option<Arc<dyn Clock>>,
}

impl Config {
    pub const DEFAULT: Self = Self {
        first_edit_delay: Duration::from_millis(25),
        spawn: |builder, f| builder.spawn(f),
        clock: None,
    };
}

pub(crate) struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    reader: Mutex<Option<Connection>>,
    path: PathBuf,
    listeners: Vec<Listener>,
    config: Config,
    clock: Arc<dyn Clock>,
    pub gate: Arc<gate::Gate>,
}

struct State {
    status: IndexStatus,
    issues: Arc<[PageIssue]>,
    workspace_dirty: bool,
    workspace_in_flight: bool,
    workspace_generation: u64,
    pending_bodies: BTreeMap<DocId, Instant>,
    replace: Option<CacheProblem>,
    busy: bool,
    shutdown: bool,
    #[cfg(test)]
    waits: Vec<Duration>,
    #[cfg(test)]
    kept_waiting_at: Option<Duration>,
}

struct Current {
    generation: u64,
    issues: Arc<[PageIssue]>,
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
                issues: Arc::from([]),
                workspace_dirty: false,
                workspace_in_flight: false,
                workspace_generation: 0,
                pending_bodies: BTreeMap::new(),
                replace: None,
                busy: true,
                shutdown: false,
                #[cfg(test)]
                waits: Vec::new(),
                #[cfg(test)]
                kept_waiting_at: None,
            }),
            changed: Condvar::new(),
            reader: Mutex::new(None),
            path: path.clone(),
            listeners: vec![listener],
            clock: config
                .clock
                .clone()
                .unwrap_or_else(|| Arc::new(SystemClock::new())),
            config,
            gate,
        });
        let worker = shared.clone();
        let spawned = (shared.config.spawn)(
            std::thread::Builder::new()
                .name("aelomi-index".into())
                .stack_size(8 * 1024 * 1024),
            Box::new(move || worker::run(&worker, &*source, &path)),
        );
        let worker = match spawned {
            Ok(worker) => Some(worker),
            Err(e) => {
                shared.lock().busy = false;
                shared.publish(IndexStatus::Failed {
                    reason: format!("could not start the index worker: {e}"),
                });
                None
            }
        };
        Self { shared, worker }
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

    pub fn search(&self, query: &SearchQuery) -> Result<Reply<Vec<SearchHit>>, QueryError> {
        let mut exclude: Vec<&PageId> = Vec::new();
        if let SearchScope::Content { exclude: ids } = query.scope {
            exclude.extend(ids);
            exclude.sort();
            exclude.dedup();
            if exclude.len() > MAX_EXCLUDED {
                return Err(QueryError::InvalidQuery(InvalidQuery::TooManyExcluded {
                    max: MAX_EXCLUDED,
                }));
            }
        }
        let words = highlight::words(query.text);
        let expression = highlight::match_expression(&words);
        let plan = |expression| query::Plan {
            expression,
            words: &words,
            limit: query.limit.clamp(1, MAX_LIMIT),
            scope: match query.scope {
                SearchScope::All => query::Scope::All,
                SearchScope::Content { .. } => query::Scope::Content { exclude: &exclude },
            },
        };
        self.shared.read_if_current(|conn| match &expression {
            Some(expression) => Ok(query::search(conn, &plan(expression))?.0),
            None => Ok(Vec::new()),
        })
    }

    pub fn backlinks(&self, target: &PageId) -> Result<Reply<Vec<Backlink>>, QueryError> {
        self.shared
            .read_if_current(|conn| query::backlinks(conn, target))
    }

    pub(crate) fn hold(&self, point: HoldPoint) {
        self.shared.gate.pass(point);
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
        f: impl FnOnce(&mut Connection) -> Result<T, QueryFault>,
    ) -> Result<Reply<T>, QueryError> {
        let Some(Current { generation, .. }) = self.fresh()? else {
            return Ok(Reply::Indexing);
        };
        let mut reader = self.lock_reader();
        if self.fresh()?.map(|now| now.generation) != Some(generation) {
            return Ok(Reply::Indexing);
        }
        let conn = match reader.as_mut() {
            Some(conn) => conn,
            None => reader
                .insert(db::open_reader(&self.path).map_err(|e| QueryError::Sql(e.to_string()))?),
        };
        let result = f(conn);
        drop(reader);
        let value = match result {
            Ok(value) => value,
            Err(QueryFault::Corrupt(what)) => {
                self.request_replace(CacheProblem::Corrupt(what));
                return Ok(Reply::Indexing);
            }
            Err(QueryFault::Sql(e)) if is_corruption(&e) => {
                self.request_replace(CacheProblem::IntegrityCheck(e.to_string()));
                return Ok(Reply::Indexing);
            }
            Err(QueryFault::Sql(e)) => return Err(QueryError::Sql(e.to_string())),
            Err(QueryFault::Tokenizer(e)) => return Err(QueryError::Sql(e.0)),
        };
        Ok(match self.fresh()? {
            Some(now) if now.generation == generation => Reply::Ready {
                value,
                issues: now.issues,
            },
            _ => Reply::Indexing,
        })
    }

    fn fresh(&self) -> Result<Option<Current>, QueryError> {
        let state = self.lock();
        match &state.status {
            IndexStatus::Failed { reason } => Err(QueryError::Failed(reason.clone())),
            IndexStatus::Rebuilding => Ok(None),
            IndexStatus::Ready
                if state.workspace_dirty
                    || state.workspace_in_flight
                    || state.replace.is_some() =>
            {
                Ok(None)
            }
            IndexStatus::Ready => Ok(Some(Current {
                generation: state.workspace_generation,
                issues: state.issues.clone(),
            })),
        }
    }

    fn request_replace(&self, problem: CacheProblem) {
        self.lock().replace.get_or_insert(problem);
        self.changed.notify_all();
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

    /// Every retry wait the worker started, in order.
    #[cfg(test)]
    pub fn waits(&self) -> Vec<Duration> {
        self.lock().waits.clone()
    }

    /// The latest index-clock time at which a retry wait checked and kept waiting.
    #[cfg(test)]
    pub fn kept_waiting_at(&self) -> Option<Duration> {
        self.lock().kept_waiting_at
    }

    #[cfg(test)]
    pub fn close_reader(&self) {
        self.lock_reader().take();
    }

    #[cfg(test)]
    pub fn settle(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.lock();
        loop {
            let idle = !state.busy
                && !state.workspace_dirty
                && !state.workspace_in_flight
                && state.replace.is_none()
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

fn is_corruption(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
    )
}
