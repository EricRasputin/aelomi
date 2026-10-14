pub mod body;
pub mod records;
pub mod tree;

#[cfg(any(test, feature = "test-support"))]
pub mod fixture;
#[cfg(test)]
mod tests;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

use crate::index::{
    self, Backlink, HoldPoint, Index, IndexEvent, IndexStatus, QueryError, Reply, SearchHit,
    SearchQuery,
};
use crate::storage::{AppendError, DocId, Health, OpenError, Seq, Store};
use records::{PageId, RecordError, read_pages};
use tree::Tree;

pub struct Workspace {
    index: Index,
    store: Arc<Store>,
    revision: AtomicU64,
    tree: Mutex<Option<(u64, Arc<Tree>)>>,
}

#[derive(Debug)]
pub enum TreeError {
    Io(io::Error),
    Malformed(RecordError),
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not load the workspace doc: {e}"),
            Self::Malformed(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for TreeError {}

struct StoreSource(Arc<Store>);

impl index::Source for StoreSource {
    fn load(&self, doc: &DocId) -> io::Result<Vec<u8>> {
        self.0.load(doc)
    }
}

impl Workspace {
    /// Opens the store at `root`, the resolved data folder, and starts the index in
    /// `root`/`index.sqlite`. Returns before the index does any work. `on_index` hears every
    /// index event on the index's thread, except a worker-spawn failure on this thread. A
    /// panic in the callback is caught.
    pub fn open(
        root: impl AsRef<Path>,
        on_index: impl Fn(&IndexEvent) + Send + Sync + 'static,
    ) -> Result<Self, OpenError> {
        let store = Store::open(root.as_ref())?;
        let root = std::fs::canonicalize(root)?;
        Ok(Self::start(
            store,
            root.join(index::FILE),
            index::Config::DEFAULT,
            Arc::new(on_index),
            Arc::default(),
        ))
    }

    /// Indexes `store`, such as one on a simulated disk, into the file at `index_path`.
    /// SQLite opens the index only by a path without symlinks.
    #[cfg(any(test, feature = "test-support"))]
    pub fn with_store(
        store: Store,
        index_path: PathBuf,
        on_index: impl Fn(&IndexEvent) + Send + Sync + 'static,
    ) -> Self {
        Self::start(
            store,
            index_path,
            index::Config::DEFAULT,
            Arc::new(on_index),
            Arc::default(),
        )
    }

    pub(crate) fn start(
        store: Store,
        index_path: PathBuf,
        config: index::Config,
        on_index: Arc<dyn Fn(&IndexEvent) + Send + Sync>,
        gate: Arc<index::gate::Gate>,
    ) -> Self {
        let store = Arc::new(store);
        let index = Index::start(
            Arc::new(StoreSource(store.clone())),
            index_path,
            config,
            on_index,
            gate,
        );
        Self {
            index,
            store,
            revision: AtomicU64::new(0),
            tree: Mutex::new(None),
        }
    }

    /// See [`Store::load`].
    pub fn load(&self, doc: &DocId) -> io::Result<Vec<u8>> {
        self.store.load(doc)
    }

    /// See [`Store::append`]. Once the store accepts the update, marks the doc for the index.
    pub fn append(&self, doc: &DocId, update: &[u8]) -> Result<Seq, AppendError> {
        let seq = self.store.append(doc, update)?;
        if *doc == DocId::workspace() {
            self.revision.fetch_add(1, Ordering::SeqCst);
        }
        self.index.mark(doc);
        Ok(seq)
    }

    /// See [`Store::flush`]. Never waits for the index.
    pub fn flush(&self, deadline: Instant) -> Vec<DocId> {
        self.store.flush(deadline)
    }

    pub fn health(&self) -> Health {
        self.store.health()
    }

    pub fn subscribe_health(&self, f: impl Fn(&Health) + Send + Sync + 'static) {
        self.store.subscribe_health(f);
    }

    /// The page tree, read from the workspace doc, whatever the index status. Reused until the
    /// next accepted workspace append.
    pub fn tree(&self) -> Result<Arc<Tree>, TreeError> {
        let revision = self.revision.load(Ordering::SeqCst);
        if let Some(tree) = self.memo(revision)
            && self.revision.load(Ordering::SeqCst) == revision
        {
            return Ok(tree);
        }
        self.index.hold(HoldPoint::TreeBeforeLoad);
        let tree = Arc::new(self.project_tree()?);
        self.index.hold(HoldPoint::TreeBeforeInstall);
        if self.revision.load(Ordering::SeqCst) == revision {
            let mut memo = self.tree.lock().unwrap_or_else(PoisonError::into_inner);
            if memo.as_ref().is_none_or(|(known, _)| *known < revision) {
                *memo = Some((revision, tree.clone()));
            }
        }
        Ok(tree)
    }

    fn memo(&self, revision: u64) -> Option<Arc<Tree>> {
        match &*self.tree.lock().unwrap_or_else(PoisonError::into_inner) {
            Some((known, tree)) if *known == revision => Some(tree.clone()),
            _ => None,
        }
    }

    fn project_tree(&self) -> Result<Tree, TreeError> {
        let bytes = self
            .store
            .load(&DocId::workspace())
            .map_err(TreeError::Io)?;
        let doc = Doc::new();
        let update = Update::decode_v1(&bytes)
            .map_err(|e| TreeError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;
        doc.transact_mut()
            .apply_update(update)
            .map_err(|e| TreeError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;
        let records = read_pages(&doc.transact()).map_err(TreeError::Malformed)?;
        Ok(tree::tree(&records))
    }

    pub fn index_status(&self) -> IndexStatus {
        self.index.status()
    }

    /// Active pages that match `query`: matching titles first, then matching bodies, each newest
    /// created first. See [`SearchQuery`].
    pub fn search(&self, query: &SearchQuery) -> Result<Reply<Vec<SearchHit>>, QueryError> {
        self.index.search(query)
    }

    /// Active pages that link to `target`, most recently updated first. Empty if the target
    /// is not active.
    pub fn backlinks(&self, target: &PageId) -> Result<Reply<Vec<Backlink>>, QueryError> {
        self.index.backlinks(target)
    }

    #[cfg(test)]
    pub(crate) fn index(&self) -> &Index {
        &self.index
    }
}

impl fmt::Debug for Workspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workspace")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}
