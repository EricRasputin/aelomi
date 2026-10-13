use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use yrs::{WriteTxn, XmlElementPrelim, XmlFragment};

use super::{Docs, eventually, hit_ids, index_path, open, open_with, page, ready, snapshot};
use crate::index::{CacheState, Config, Index, IndexEvent, IndexStatus, QueryError, Reply, Source};
use crate::storage::{Crash, DocId, Driver, Limits, OpKind, SimFs, Store, SystemClock};
use crate::workspace::TreeError;
use crate::workspace::fixture::{index_fixture, paragraph, text, write_body};

fn failed(reason: &str) -> IndexStatus {
    IndexStatus::Failed {
        reason: reason.into(),
    }
}

#[test]
fn an_index_that_cannot_open_fails_while_docs_load_append_and_flush() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(index_path(dir.path())).unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    let reason = "index.sqlite is a folder, so the index cannot be created";
    eventually(Duration::from_secs(10), || {
        (opened.workspace.index_status() == failed(reason)).then_some(())
    });
    assert_eq!(
        opened.workspace.search("roadm", 10),
        Err(QueryError::Failed(reason.into()))
    );
    assert_eq!(
        opened.workspace.backlinks(&page("aelomi")),
        Err(QueryError::Failed(reason.into()))
    );
    let tree = opened.workspace.tree().unwrap();
    assert_eq!(tree.pages.len(), 6);
    assert!(
        !opened
            .workspace
            .load(&DocId::new("daily").unwrap())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        opened
            .workspace
            .flush(Instant::now() + Duration::from_secs(10)),
        []
    );
    assert!(index_path(dir.path()).is_dir(), "a folder is never removed");
}

#[test]
fn a_malformed_body_fails_the_index_until_an_edit_fixes_it() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    opened.settle();
    opened.edit_body("daily", |txn| {
        let fragment = txn.get_or_insert_xml_fragment("prosemirror");
        let Some(yrs::XmlOut::Element(group)) = fragment.get(txn, 0) else {
            unreachable!()
        };
        group.insert(txn, 0, XmlElementPrelim::empty("blockContainer"));
    });
    opened.settle();
    let reason = "daily: a blockContainer has no string id";
    assert_eq!(opened.workspace.index_status(), failed(reason));
    assert_eq!(
        opened.workspace.search("cafe", 10),
        Err(QueryError::Failed(reason.into()))
    );
    assert!(
        !opened
            .workspace
            .load(&DocId::new("daily").unwrap())
            .unwrap()
            .is_empty()
    );

    opened.edit_body("daily", |txn| {
        write_body(txn, &[paragraph("d1", vec![text("repaired")])])
    });
    opened.settle();
    assert_eq!(opened.workspace.index_status(), IndexStatus::Ready);
    assert_eq!(
        hit_ids(&ready(opened.workspace.search("repaired", 10).unwrap())),
        ["daily"]
    );
    assert_eq!(
        opened.statuses(),
        [
            IndexStatus::Rebuilding,
            IndexStatus::Ready,
            failed(reason),
            IndexStatus::Ready,
        ]
    );
}

#[test]
fn a_malformed_record_fails_the_index_and_the_tree_until_fixed() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    opened.settle();
    let workspace = DocId::workspace();
    let mut editor = crate::workspace::fixture::Editor::new(42);
    editor.apply(&workspace, &opened.workspace.load(&workspace).unwrap());
    let breaks = editor.edit(&workspace, |txn| {
        use yrs::{Map, Out};
        let pages = txn.get_or_insert_map("pages");
        let Some(Out::YMap(daily)) = pages.get(txn, "daily") else {
            unreachable!()
        };
        daily.insert(txn, "title", 7);
    });
    opened.workspace.append(&workspace, &breaks).unwrap();
    opened.settle();
    let reason = "workspace: page record \"daily\": title is not a string, found a number";
    assert_eq!(opened.workspace.index_status(), failed(reason));
    assert!(matches!(
        opened.workspace.tree(),
        Err(TreeError::Malformed(_))
    ));

    let fixes = editor.edit(&workspace, |txn| {
        use yrs::{Map, Out};
        let pages = txn.get_or_insert_map("pages");
        let Some(Out::YMap(daily)) = pages.get(txn, "daily") else {
            unreachable!()
        };
        daily.insert(txn, "title", "Daily again");
    });
    opened.workspace.append(&workspace, &fixes).unwrap();
    opened.settle();
    assert_eq!(opened.workspace.index_status(), IndexStatus::Ready);
    let hits = ready(opened.workspace.search("again", 10).unwrap());
    let mut ids = hit_ids(&hits);
    ids.sort();
    assert_eq!(
        ids,
        ["daily", "untitled"],
        "the rename reached the title and a link to it"
    );
}

#[test]
fn a_panicking_listener_stops_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicU32::new(0));
    let counted = calls.clone();
    let workspace = crate::workspace::Workspace::open(dir.path(), move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        panic!("a listener bug");
    })
    .unwrap();
    for (doc, update) in index_fixture() {
        workspace.append(&doc, &update).unwrap();
    }
    let hits = eventually(Duration::from_secs(10), || {
        match workspace.search("cafe", 5) {
            Ok(Reply::Ready(hits)) if !hits.is_empty() => Some(hits),
            _ => None,
        }
    });
    assert_eq!(hit_ids(&hits), ["reading"]);
    assert!(calls.load(Ordering::SeqCst) >= 2);
}

struct Flaky {
    store: Arc<Store>,
    fail: Mutex<Vec<&'static str>>,
}

impl Source for Flaky {
    fn load(&self, doc: &DocId) -> io::Result<Vec<u8>> {
        let next = self.fail.lock().unwrap().pop();
        match next {
            Some("panic") => panic!("a bug while loading"),
            Some(_) => Err(io::Error::other("disk read failed")),
            None => self.store.load(doc),
        }
    }
}

fn flaky_index(dir: &Path, fail: Vec<&'static str>) -> (Index, Arc<Mutex<Vec<IndexEvent>>>) {
    let store = Arc::new(Store::open(dir).unwrap());
    for (doc, update) in index_fixture() {
        store.append(&doc, &update).unwrap();
    }
    let events = Arc::new(Mutex::new(Vec::new()));
    let record = events.clone();
    let index = Index::start(
        Arc::new(Flaky {
            store,
            fail: Mutex::new(fail),
        }),
        index_path(dir),
        Config::DEFAULT,
        Arc::new(move |e: &IndexEvent| record.lock().unwrap().push(e.clone())),
        Arc::default(),
    );
    (index, events)
}

#[test]
fn a_worker_panic_or_read_error_reports_failed_then_retries_to_ready() {
    for (fail, reason) in [
        ("panic", "the index worker panicked"),
        ("error", "could not load doc workspace: disk read failed"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (index, events) = flaky_index(dir.path(), vec![fail]);
        eventually(Duration::from_secs(10), || {
            (index.status() == IndexStatus::Ready).then_some(())
        });
        let statuses: Vec<IndexStatus> = events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                IndexEvent::Status(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            statuses,
            [IndexStatus::Rebuilding, failed(reason), IndexStatus::Ready]
        );
        assert_eq!(
            hit_ids(&ready(index.search("cafe", 5).unwrap())),
            ["reading"]
        );
    }
}

#[test]
fn a_valid_index_that_lacks_a_flushed_edit_is_repaired_at_startup() {
    let dir = tempfile::tempdir().unwrap();
    {
        let opened = open(dir.path());
        opened.append(&index_fixture());
        opened.settle();
        assert_eq!(
            opened
                .workspace
                .flush(Instant::now() + Duration::from_secs(10)),
            []
        );
    }
    {
        let store = Store::open(dir.path()).unwrap();
        let daily = DocId::new("daily").unwrap();
        let mut editor = crate::workspace::fixture::Editor::new(8);
        editor.apply(&daily, &store.load(&daily).unwrap());
        let update = editor.edit(&daily, |txn| {
            crate::workspace::fixture::insert_block(
                txn,
                0,
                &paragraph("b1", vec![text("behindword")]),
            )
        });
        store.append(&daily, &update).unwrap();
        store
            .close(Instant::now() + Duration::from_secs(10))
            .unwrap();
    }
    let opened = open(dir.path());
    opened.settle();
    assert_eq!(
        &opened.events()[..2],
        [
            IndexEvent::Status(IndexStatus::Rebuilding),
            IndexEvent::Cache(CacheState::Reused),
        ]
    );
    assert_eq!(
        hit_ids(&ready(opened.workspace.search("behindword", 5).unwrap())),
        ["daily"]
    );
}

#[test]
fn a_valid_index_that_holds_an_edit_lost_in_a_crash_is_repaired_at_startup() {
    let index_dir = tempfile::tempdir().unwrap();
    let fs = SimFs::new(Arc::new(SystemClock::new()));
    let root = Path::new("/data");
    let open_store = || {
        Store::open_with(
            root,
            fs.clone(),
            Arc::new(SystemClock::new()),
            Driver::Threads,
            Limits::DATA_6,
        )
        .unwrap()
    };
    let expected = {
        let opened = open_with(
            open_store(),
            index_path(index_dir.path()),
            Config::DEFAULT,
            Arc::default(),
        );
        opened.append(&index_fixture());
        opened.settle();
        assert_eq!(
            opened
                .workspace
                .flush(Instant::now() + Duration::from_secs(10)),
            []
        );
        let expected = snapshot(&opened.workspace);
        fs.fail(OpKind::Append, "docs/daily/", libc::EIO);
        let daily = DocId::new("daily").unwrap();
        let mut editor = crate::workspace::fixture::Editor::new(8);
        editor.apply(&daily, &opened.workspace.load(&daily).unwrap());
        let update = editor.edit(&daily, |txn| {
            crate::workspace::fixture::insert_block(
                txn,
                0,
                &paragraph("g1", vec![text("ghostword")]),
            )
        });
        opened.workspace.append(&daily, &update).unwrap();
        opened.settle();
        assert_eq!(
            hit_ids(&ready(opened.workspace.search("ghostword", 5).unwrap())),
            ["daily"]
        );
        assert_eq!(
            opened
                .workspace
                .flush(Instant::now() + Duration::from_millis(200)),
            [daily]
        );
        expected
    };
    fs.crash(Crash::DropUnsynced);
    fs.clear_faults();
    let opened = open_with(
        open_store(),
        index_path(index_dir.path()),
        Config::DEFAULT,
        Arc::default(),
    );
    opened.settle();
    assert_eq!(
        &opened.events()[..2],
        [
            IndexEvent::Status(IndexStatus::Rebuilding),
            IndexEvent::Cache(CacheState::Reused),
        ]
    );
    assert_eq!(ready(opened.workspace.search("ghostword", 5).unwrap()), []);
    assert_eq!(snapshot(&opened.workspace), expected);
}

struct Slow(Docs);

impl Source for Slow {
    fn load(&self, doc: &DocId) -> io::Result<Vec<u8>> {
        std::thread::sleep(Duration::from_millis(10));
        self.0.load(doc)
    }
}

#[test]
fn dropping_the_index_mid_reconcile_returns_between_pages() {
    let dir = tempfile::tempdir().unwrap();
    let mut editor = crate::workspace::fixture::Editor::new(4);
    let workspace = DocId::workspace();
    let pages: Vec<String> = (0..500).map(|n| format!("page-{n}")).collect();
    let mut docs = std::collections::BTreeMap::new();
    let records = editor.edit(&workspace, |txn| {
        for id in &pages {
            let record = crate::workspace::records::PageRecord {
                title: id.clone(),
                placement: crate::workspace::records::Placement {
                    parent: None,
                    order: 0,
                },
                favorite: None,
                trash: None,
                created_at: 1,
                updated_at: 1,
            };
            crate::workspace::fixture::put_page(txn, &page(id), &record);
        }
    });
    docs.insert(workspace, records);
    let index = Index::start(
        Arc::new(Slow(Docs(docs))),
        index_path(dir.path()),
        Config::DEFAULT,
        Arc::new(|_| {}),
        Arc::default(),
    );
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        index.status(),
        IndexStatus::Rebuilding,
        "500 slow pages take 5 s"
    );
    let started = Instant::now();
    drop(index);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "{:?}",
        started.elapsed()
    );
}
