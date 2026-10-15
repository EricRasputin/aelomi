mod backoff;
mod bodies;
mod cache;
mod failures;
mod highlight_parity;
mod independent_oracle;
mod latency;
mod races;
mod rebuild;
mod scrub;
mod search_fixture;
mod search_gate;
mod search_oracle;
mod search_rules;
mod tree_memo;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use yrs::Transact;

use super::gate::Gate;
use super::{
    Backlink, Config, IndexEvent, IndexStatus, PageIssue, Reply, SearchHit, SearchQuery,
    SearchScope,
};
use crate::storage::{DocId, Store};
use crate::workspace::Workspace;
use crate::workspace::fixture::{Editor, erase_page, put_page};
use crate::workspace::records::{PageId, PageRecord, read_pages};
use crate::workspace::tree::Tree;

pub(crate) struct Opened {
    pub workspace: Workspace,
    pub events: Arc<Mutex<Vec<IndexEvent>>>,
    pub gate: Arc<Gate>,
}

impl Opened {
    pub fn events(&self) -> Vec<IndexEvent> {
        self.events.lock().unwrap().clone()
    }

    pub fn statuses(&self) -> Vec<IndexStatus> {
        self.events()
            .into_iter()
            .filter_map(|e| match e {
                IndexEvent::Status(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    pub fn settle(&self) {
        assert!(
            self.workspace
                .index()
                .shared()
                .settle(Duration::from_secs(30)),
            "the index never settled; events {:?}",
            self.events()
        );
    }

    pub fn edit_records(&self, f: impl FnOnce(&mut BTreeMap<PageId, PageRecord>)) {
        let workspace = DocId::workspace();
        let mut editor = Editor::new(99);
        editor.apply(&workspace, &self.workspace.load(&workspace).unwrap());
        let before = read_pages(&doc_of(&editor.state(&workspace)).transact()).unwrap();
        let mut after = before.clone();
        f(&mut after);
        let update = editor.edit(&workspace, |txn| {
            for (id, record) in &after {
                if before.get(id) != Some(record) {
                    put_page(txn, id, record);
                }
            }
            for id in before.keys().filter(|id| !after.contains_key(*id)) {
                erase_page(txn, id);
            }
        });
        self.workspace.append(&workspace, &update).unwrap();
    }

    pub fn edit_body(&self, page: &str, f: impl FnOnce(&mut yrs::TransactionMut)) {
        let doc = DocId::new(page).unwrap();
        let mut editor = Editor::new(99);
        editor.apply(&doc, &self.workspace.load(&doc).unwrap());
        let update = editor.edit(&doc, f);
        self.workspace.append(&doc, &update).unwrap();
    }

    pub fn append(&self, updates: &[(DocId, Vec<u8>)]) {
        for (doc, update) in updates {
            self.workspace.append(doc, update).unwrap();
        }
    }
}

pub(crate) struct Docs(pub BTreeMap<DocId, Vec<u8>>);

impl super::Source for Docs {
    fn load(&self, doc: &DocId) -> std::io::Result<Vec<u8>> {
        Ok(self.0.get(doc).cloned().unwrap_or_else(|| vec![0, 0]))
    }
}

pub(crate) fn index_path(dir: &Path) -> PathBuf {
    std::fs::canonicalize(dir).unwrap().join(super::FILE)
}

pub(crate) fn open(dir: &Path) -> Opened {
    open_with(
        Store::open(dir).unwrap(),
        index_path(dir),
        Config::DEFAULT,
        Arc::default(),
    )
}

pub(crate) fn open_with(store: Store, index: PathBuf, config: Config, gate: Arc<Gate>) -> Opened {
    let events = Arc::new(Mutex::new(Vec::new()));
    let record = events.clone();
    let workspace = Workspace::start(
        store,
        index,
        config,
        Arc::new(move |event: &IndexEvent| record.lock().unwrap().push(event.clone())),
        gate.clone(),
    );
    Opened {
        workspace,
        events,
        gate,
    }
}

pub(crate) fn doc_of(update: &[u8]) -> yrs::Doc {
    use yrs::updates::decoder::Decode;
    let doc = yrs::Doc::new();
    doc.transact_mut()
        .apply_update(yrs::Update::decode_v1(update).unwrap())
        .unwrap();
    doc
}

pub(crate) fn page(id: &str) -> PageId {
    PageId::new(id).unwrap()
}

pub(crate) fn ready<T>(reply: Reply<T>) -> T {
    ready_with_issues(reply).0
}

pub(crate) fn ready_with_issues<T>(reply: Reply<T>) -> (T, Vec<PageIssue>) {
    match reply {
        Reply::Ready { value, issues } => (value, issues.to_vec()),
        Reply::Indexing => panic!("the index answered Indexing"),
    }
}

pub(crate) fn all(text: &str, limit: usize) -> SearchQuery<'_> {
    SearchQuery {
        text,
        limit,
        scope: SearchScope::All,
    }
}

pub(crate) fn content<'a>(text: &'a str, limit: usize, exclude: &'a [PageId]) -> SearchQuery<'a> {
    SearchQuery {
        text,
        limit,
        scope: SearchScope::Content { exclude },
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Snapshot {
    pub tree: Arc<Tree>,
    pub backlinks: BTreeMap<PageId, Vec<Backlink>>,
    pub searches: Vec<(&'static str, Vec<SearchHit>)>,
}

pub(crate) const QUERIES: [&str; 14] = [
    "roadm",
    "quillmere",
    "orphanword",
    "cafe",
    "kestrel",
    "deleted page",
    "line two",
    "diagram",
    "archived",
    "aelomi app",
    "untitled",
    "",
    "\"",
    "Mileston*",
];

pub(crate) const FIXTURE_PAGES: [&str; 9] = [
    "projects",
    "aelomi",
    "roadmap",
    "reading",
    "daily",
    "archive",
    "old-notes",
    "untitled",
    "erased",
];

pub(crate) fn snapshot(workspace: &Workspace) -> Snapshot {
    Snapshot {
        tree: workspace.tree().unwrap(),
        backlinks: FIXTURE_PAGES
            .iter()
            .map(|id| (page(id), ready(workspace.backlinks(&page(id)).unwrap())))
            .collect(),
        searches: QUERIES
            .iter()
            .map(|q| (*q, ready(workspace.search(&all(q, 50)).unwrap())))
            .collect(),
    }
}

pub(crate) fn hit_ids(hits: &[SearchHit]) -> Vec<&str> {
    hits.iter().map(|h| h.page_id.as_str()).collect()
}

pub(crate) fn eventually<T>(timeout: Duration, mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(value) = f() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub(crate) fn source_files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in std::fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let name = entry.path();
            let relative = name
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if relative.starts_with("index.sqlite") || relative == ".lock" {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                stack.push(name);
            } else {
                files.insert(relative, std::fs::read(&name).unwrap());
            }
        }
    }
    files
}
