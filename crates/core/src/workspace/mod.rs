pub mod body;
mod edit;
pub mod records;
mod seed;
pub mod tree;
mod write;

pub use edit::{PageChange, PageEdit, PageEditError, PageEditReply};

#[cfg(any(test, feature = "test-support"))]
pub mod fixture;
#[cfg(test)]
mod tests;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
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
    tree: Mutex<TreeMemo>,
    tree_projected: Condvar,
    record_edits: Mutex<()>,
    record_client: yrs::ClientID,
    settings: Mutex<Option<crate::settings::Settings>>,
}

#[derive(Debug)]
pub enum SettingsError {
    Read(io::Error),
    Write(crate::storage::FailReason),
}

pub struct SettingsUpdate {
    workspace: Arc<Workspace>,
    patch: crate::settings::SettingsPatch,
    _admission: SettingsAdmission,
}

struct SettingsAdmission(Arc<Store>);

impl Drop for SettingsAdmission {
    fn drop(&mut self) {
        self.0.release_settings();
    }
}

impl SettingsUpdate {
    pub fn run(self) -> Result<crate::settings::Settings, SettingsError> {
        self.workspace.update_settings(self.patch)
    }
}

/// An open store before any index reader starts. The shell may seed it, then start the workspace.
pub struct PreparedWorkspace {
    store: Store,
    index_path: PathBuf,
    record_client: yrs::ClientID,
}

impl PreparedWorkspace {
    fn new(store: Store, index_path: PathBuf) -> Self {
        Self {
            store,
            index_path,
            record_client: yrs::ClientID::random(),
        }
    }

    pub fn start(self, on_index: impl Fn(&IndexEvent) + Send + Sync + 'static) -> Workspace {
        Workspace::start_prepared(
            self,
            index::Config::DEFAULT,
            Arc::new(on_index),
            Arc::default(),
        )
    }
}

/// The newest tree and the revision being projected, so callers for that revision wait for
/// its projection instead of repeating it.
#[derive(Default)]
struct TreeMemo {
    known: Option<(u64, Arc<Tree>)>,
    projecting: Option<u64>,
    #[cfg(test)]
    projections: usize,
    #[cfg(test)]
    waits: usize,
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
        Ok(Self::prepare(root)?.start(on_index))
    }

    /// Opens only the store, before the shell's optional first-run seed and index startup.
    pub fn prepare(root: impl AsRef<Path>) -> Result<PreparedWorkspace, OpenError> {
        let store = Store::open(root.as_ref())?;
        let root = std::fs::canonicalize(root)?;
        Ok(PreparedWorkspace::new(store, root.join(index::FILE)))
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn prepare_with_store(store: Store, index_path: PathBuf) -> PreparedWorkspace {
        PreparedWorkspace::new(store, index_path)
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

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn start(
        store: Store,
        index_path: PathBuf,
        config: index::Config,
        on_index: Arc<dyn Fn(&IndexEvent) + Send + Sync>,
        gate: Arc<index::gate::Gate>,
    ) -> Self {
        Self::start_prepared(
            PreparedWorkspace::new(store, index_path),
            config,
            on_index,
            gate,
        )
    }

    fn start_prepared(
        prepared: PreparedWorkspace,
        config: index::Config,
        on_index: Arc<dyn Fn(&IndexEvent) + Send + Sync>,
        gate: Arc<index::gate::Gate>,
    ) -> Self {
        let PreparedWorkspace {
            store,
            index_path,
            record_client,
        } = prepared;
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
            tree: Mutex::default(),
            tree_projected: Condvar::new(),
            settings: Mutex::default(),
            record_edits: Mutex::new(()),
            record_client,
        }
    }

    pub fn settings(&self) -> io::Result<crate::settings::Settings> {
        let mut current = self.settings.lock().unwrap_or_else(PoisonError::into_inner);
        if current.is_none() {
            *current = Some(self.store.load_settings()?);
        }
        Ok(current.as_ref().expect("settings loaded").clone())
    }

    pub fn prepare_settings_update(
        self: &Arc<Self>,
        patch: crate::settings::SettingsPatch,
    ) -> SettingsUpdate {
        self.store.admit_settings();
        SettingsUpdate {
            workspace: self.clone(),
            patch,
            _admission: SettingsAdmission(self.store.clone()),
        }
    }

    fn update_settings(
        &self,
        patch: crate::settings::SettingsPatch,
    ) -> Result<crate::settings::Settings, SettingsError> {
        let mut current = self.settings.lock().unwrap_or_else(PoisonError::into_inner);
        if current.is_none() {
            *current = Some(self.store.load_settings().map_err(SettingsError::Read)?);
        }
        let settings = current.as_mut().expect("settings loaded");
        settings.apply(patch);
        let snapshot = settings.clone();
        let seq = self.store.queue_settings(snapshot.clone());
        drop(current);
        self.store
            .wait_settings(seq)
            .map_err(SettingsError::Write)?;
        Ok(snapshot)
    }

    /// See [`Store::load`].
    pub fn load(&self, doc: &DocId) -> io::Result<Vec<u8>> {
        self.store.load(doc)
    }

    /// See [`Store::append`]. Once the store accepts the update, marks the doc for the index.
    pub fn append(&self, doc: &DocId, update: &[u8]) -> Result<Seq, AppendError> {
        let _held = (*doc == DocId::workspace()).then(|| {
            self.record_edits
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
        });
        self.append_unlocked(doc, update)
    }

    fn append_record_update(
        &self,
        _held: &MutexGuard<'_, ()>,
        update: &[u8],
    ) -> Result<Seq, AppendError> {
        self.append_unlocked(&DocId::workspace(), update)
    }

    fn append_unlocked(&self, doc: &DocId, update: &[u8]) -> Result<Seq, AppendError> {
        let seq = self.store.append(doc, update)?;
        if *doc == DocId::workspace() {
            self.revision.fetch_add(1, Ordering::SeqCst);
        }
        self.index.mark(doc);
        Ok(seq)
    }

    /// See [`Store::flush`]. Never waits for the index.
    pub fn flush(&self, deadline: Instant) -> crate::storage::Pending {
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
        let mut memo = self.lock_tree();
        let revision = loop {
            let revision = self.revision.load(Ordering::SeqCst);
            if let Some((known, tree)) = &memo.known
                && *known == revision
            {
                return Ok(tree.clone());
            }
            if memo.projecting != Some(revision) {
                break revision;
            }
            #[cfg(test)]
            {
                memo.waits += 1;
            }
            memo = self
                .tree_projected
                .wait(memo)
                .unwrap_or_else(PoisonError::into_inner);
        };
        memo.projecting = Some(revision);
        #[cfg(test)]
        {
            memo.projections += 1;
        }
        drop(memo);
        let projecting = Projecting {
            workspace: self,
            revision,
        };
        self.index.hold(HoldPoint::TreeBeforeLoad);
        let tree = Arc::new(self.project_tree()?);
        self.index.hold(HoldPoint::TreeBeforeInstall);
        let mut memo = self.lock_tree();
        if self.revision.load(Ordering::SeqCst) == revision
            && memo
                .known
                .as_ref()
                .is_none_or(|(known, _)| *known < revision)
        {
            memo.known = Some((revision, tree.clone()));
        }
        drop(memo);
        drop(projecting);
        Ok(tree)
    }

    fn lock_tree(&self) -> MutexGuard<'_, TreeMemo> {
        self.tree.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[cfg(test)]
    pub(crate) fn tree_projections(&self) -> (usize, usize) {
        let memo = self.lock_tree();
        (memo.projections, memo.waits)
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

/// Ends a projection however it ends, so callers waiting for its revision go on, and project
/// it themselves if it failed or was overtaken by an append.
struct Projecting<'a> {
    workspace: &'a Workspace,
    revision: u64,
}

impl Drop for Projecting<'_> {
    fn drop(&mut self) {
        let mut memo = self.workspace.lock_tree();
        if memo.projecting == Some(self.revision) {
            memo.projecting = None;
        }
        drop(memo);
        self.workspace.tree_projected.notify_all();
    }
}

impl fmt::Debug for Workspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workspace")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}
