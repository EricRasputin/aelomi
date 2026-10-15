use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use yrs::{WriteTxn, XmlElementPrelim, XmlFragment};

use super::{
    Docs, all, content, eventually, hit_ids, index_path, open, open_with, page, ready,
    ready_with_issues, snapshot,
};
use crate::index::{
    CacheProblem, CacheState, Config, Corrupt, Index, IndexEvent, IndexStatus, PageIssue,
    PageProblem, QueryError, Reply, Source,
};
use crate::storage::{Crash, DocId, Driver, Limits, OpKind, SimFs, Store, SystemClock};
use crate::workspace::TreeError;
use crate::workspace::fixture::{index_fixture, paragraph, text, write_body};

pub(super) fn failed(reason: &str) -> IndexStatus {
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
        opened.workspace.search(&all("roadm", 10)),
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
            .flush(Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
    assert!(index_path(dir.path()).is_dir(), "a folder is never removed");
}

#[test]
fn a_malformed_body_reports_an_issue_and_keeps_every_other_result() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    opened.settle();
    let daily = DocId::new("daily").unwrap();
    opened.edit_body("daily", |txn| {
        let fragment = txn.get_or_insert_xml_fragment("prosemirror");
        let Some(yrs::XmlOut::Element(group)) = fragment.get(txn, 0) else {
            unreachable!()
        };
        group.insert(txn, 0, XmlElementPrelim::empty("blockContainer"));
    });
    let source = opened.workspace.load(&daily).unwrap();
    opened.settle();
    let issue = PageIssue {
        page_id: page("daily"),
        problem: PageProblem::MissingBlockId,
        indexed_blocks: 8,
    };
    assert_eq!(opened.workspace.index_status(), IndexStatus::Ready);
    let (hits, issues) = ready_with_issues(
        opened
            .workspace
            .search(&content("quarterly", 10, &[]))
            .unwrap(),
    );
    assert_eq!(hit_ids(&hits), ["daily"]);
    assert_eq!(hits[0].block.as_ref().unwrap().block_id.as_str(), "d2");
    assert_eq!(issues, std::slice::from_ref(&issue));
    assert_eq!(
        ready_with_issues(opened.workspace.search(&all("", 10)).unwrap()),
        (vec![], vec![issue.clone()]),
        "a query without words still names the page"
    );
    let (backlinks, issues) =
        ready_with_issues(opened.workspace.backlinks(&page("reading")).unwrap());
    assert_eq!(
        backlinks
            .iter()
            .map(|b| b.page_id.as_str())
            .collect::<Vec<_>>(),
        ["daily"]
    );
    assert_eq!(issues, std::slice::from_ref(&issue));
    assert_eq!(opened.workspace.load(&daily).unwrap(), source);

    opened.edit_records(|records| {
        records.get_mut(&page("daily")).unwrap().trash = Some(crate::workspace::records::Trash {
            original_parent: None,
            original_order: 1,
            batch_id: "b".into(),
            trashed_at: 5,
        });
    });
    opened.settle();
    assert_eq!(
        ready_with_issues(opened.workspace.search(&all("", 10)).unwrap()).1,
        [],
        "a trashed page's issue is not shown"
    );
    opened.edit_records(|records| {
        records.get_mut(&page("daily")).unwrap().trash = None;
    });
    opened.settle();
    assert_eq!(
        ready_with_issues(opened.workspace.search(&all("", 10)).unwrap()).1,
        [issue]
    );

    opened.edit_body("daily", |txn| {
        write_body(txn, &[paragraph("d1", vec![text("repaired")])])
    });
    opened.settle();
    let (hits, issues) = ready_with_issues(opened.workspace.search(&all("repaired", 10)).unwrap());
    assert_eq!(hit_ids(&hits), ["daily"]);
    assert_eq!(issues, []);
    let statuses = opened.statuses();
    assert!(
        statuses
            .iter()
            .all(|s| matches!(s, IndexStatus::Rebuilding | IndexStatus::Ready)),
        "a partial page never fails the index: {statuses:?}"
    );
    assert_eq!(statuses.last(), Some(&IndexStatus::Ready));
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
    let hits = ready(opened.workspace.search(&all("again", 10)).unwrap());
    let mut ids = hit_ids(&hits);
    ids.sort();
    assert_eq!(
        ids,
        ["daily", "untitled"],
        "the rename reached the title and a link to it"
    );
}

#[test]
fn a_pages_root_holding_a_sequence_fails_without_erasing_the_cache() {
    use yrs::{Array, Map, Out, ReadTxn, Transact};
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    // The fixture ends by erasing a page. Settling after each update indexes that page first,
    // so setup replaces the cache, as a slow worker can; only what follows the sequence counts.
    for update in index_fixture() {
        opened.append(&[update]);
        opened.settle();
    }
    let before = snapshot(&opened.workspace);
    let workspace = DocId::workspace();
    let mut foreign = crate::workspace::fixture::Editor::new(42);
    foreign.apply(&workspace, &opened.workspace.load(&workspace).unwrap());
    let sequence = foreign.edit(&workspace, |txn| {
        txn.get_or_insert_array("pages").insert(txn, 0, "foreign");
    });
    let events = opened.events().len();
    opened.workspace.append(&workspace, &sequence).unwrap();
    assert_eq!(
        opened
            .workspace
            .flush(Instant::now() + Duration::from_secs(5))
            .docs,
        []
    );
    let source = super::source_files(dir.path());
    opened.settle();

    let reason = "workspace: workspace doc: the pages root is not a Y.Map, found a sequence";
    assert_eq!(opened.workspace.index_status(), failed(reason));
    assert_eq!(
        opened.workspace.search(&all("roadmap", 10)),
        Err(QueryError::Failed(reason.into()))
    );
    assert!(matches!(
        opened.workspace.tree(),
        Err(TreeError::Malformed(_))
    ));
    assert_eq!(
        opened.events()[events..],
        [
            IndexEvent::Status(failed(reason)),
            IndexEvent::Changed(crate::index::IndexChange {
                workspace: true,
                pages: vec![],
            }),
        ],
        "no cache replacement for erased pages"
    );
    let cached: i64 = rusqlite::Connection::open(index_path(dir.path()))
        .unwrap()
        .query_row("SELECT count(*) FROM pages", [], |row| row.get(0))
        .unwrap();
    assert_eq!(cached, 8, "every cached page row is kept");
    assert_eq!(super::source_files(dir.path()), source);
    let records = super::doc_of(&opened.workspace.load(&workspace).unwrap());
    let Some(Out::UndefinedRef(root)) = records.transact().get("pages") else {
        unreachable!()
    };
    assert_eq!(yrs::MapRef::from(root).len(&records.transact()), 8);

    let undo = foreign.edit(&workspace, |txn| {
        txn.get_or_insert_array("pages").remove(txn, 0);
    });
    opened.workspace.append(&workspace, &undo).unwrap();
    opened.settle();
    assert_eq!(opened.workspace.index_status(), IndexStatus::Ready);
    assert_eq!(snapshot(&opened.workspace), before);
    assert!(
        !opened.events()[events..]
            .iter()
            .any(|event| matches!(event, IndexEvent::Cache(CacheState::Replaced(_)))),
        "{:?}",
        opened.events()
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
        match workspace.search(&all("cafe", 5)) {
            Ok(Reply::Ready { value: hits, .. }) if !hits.is_empty() => Some(hits),
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
            hit_ids(&ready(index.search(&all("cafe", 5)).unwrap())),
            ["reading"]
        );
    }
}

#[test]
fn a_retry_wait_on_the_system_clock_sleeps_until_its_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let (index, _) = flaky_index(dir.path(), vec!["error"]);
    eventually(Duration::from_secs(10), || {
        (index.status() == IndexStatus::Ready).then_some(())
    });
    let shared = index.shared();
    assert_eq!(shared.waits(), [Duration::from_millis(100)]);
    assert!(
        shared.kept_waiting() <= 2,
        "the 100 ms wait checked the clock {} times",
        shared.kept_waiting()
    );
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
                .flush(Instant::now() + Duration::from_secs(10))
                .docs,
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
        hit_ids(&ready(
            opened.workspace.search(&all("behindword", 5)).unwrap()
        )),
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
                .flush(Instant::now() + Duration::from_secs(10))
                .docs,
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
            hit_ids(&ready(
                opened.workspace.search(&all("ghostword", 5)).unwrap()
            )),
            ["daily"]
        );
        assert_eq!(
            opened
                .workspace
                .flush(Instant::now() + Duration::from_millis(200))
                .docs,
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
    assert_eq!(
        ready(opened.workspace.search(&all("ghostword", 5)).unwrap()),
        []
    );
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

pub(super) fn tamper(dir: &Path, sql: &str) {
    let conn = rusqlite::Connection::open(index_path(dir)).unwrap();
    conn.busy_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(conn.execute(sql, []).unwrap(), 1, "{sql}");
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok", "the tampering passes SQLite's own check");
}

pub(super) fn after_settled(opened: &super::Opened) -> Vec<IndexEvent> {
    opened
        .events()
        .into_iter()
        .filter(|e| !matches!(e, IndexEvent::Startup(_)))
        .collect()
}

#[test]
fn a_query_that_finds_a_corrupt_stored_value_replaces_the_cache_in_the_same_session() {
    for (sql, corrupt) in [
        (
            "UPDATE pages SET title = x'00' WHERE page_id = 'reading'",
            Corrupt::Field,
        ),
        (
            "UPDATE pages SET title = CAST(x'80' AS TEXT) WHERE page_id = 'reading'",
            Corrupt::Field,
        ),
        (
            "UPDATE blocks SET block_id = x'726431' WHERE block_id = 'rd1'",
            Corrupt::Field,
        ),
        (
            "UPDATE pages SET page_id = 'Not An ID' WHERE page_id = 'reading'",
            Corrupt::PageId,
        ),
        (
            "UPDATE blocks SET block_id = '' WHERE block_id = 'rd1'",
            Corrupt::BlockId,
        ),
        (
            "UPDATE blocks SET ordinal = -1 WHERE block_id = 'rd1'",
            Corrupt::Ordinal,
        ),
        (
            "DELETE FROM blocks WHERE block_id = 'rd1'",
            Corrupt::MissingRow,
        ),
        (
            "UPDATE pages SET active = 0 WHERE page_id = 'reading'",
            Corrupt::Field,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let opened = open(dir.path());
        opened.append(&index_fixture());
        opened.settle();
        let expected = snapshot(&opened.workspace);
        assert_eq!(
            opened
                .workspace
                .flush(Instant::now() + Duration::from_secs(10))
                .docs,
            []
        );
        let source = super::source_files(dir.path());
        let before = after_settled(&opened).len();
        tamper(dir.path(), sql);

        assert_eq!(
            opened.workspace.search(&content("cafe", 10, &[])),
            Ok(Reply::Indexing),
            "{sql}"
        );
        opened.settle();
        assert_eq!(
            after_settled(&opened)[before..],
            [
                IndexEvent::Status(IndexStatus::Rebuilding),
                IndexEvent::Cache(CacheState::Replaced(CacheProblem::Corrupt(corrupt))),
                IndexEvent::Status(IndexStatus::Ready),
            ],
            "{sql}"
        );
        assert_eq!(snapshot(&opened.workspace), expected, "{sql}");
        assert_eq!(super::source_files(dir.path()), source, "{sql}");
    }
}

#[test]
fn a_wrong_sqlite_field_type_is_replaced_at_startup() {
    for (sql, reused_first) in [
        (
            "UPDATE pages SET title = x'00' WHERE page_id = 'reading'",
            false,
        ),
        (
            "UPDATE pages SET title = CAST(x'80' AS TEXT) WHERE page_id = 'reading'",
            false,
        ),
        (
            "UPDATE pages SET updated_at = 1.5 WHERE page_id = 'reading'",
            false,
        ),
        (
            "UPDATE blocks SET block_id = x'726431' WHERE block_id = 'rd1'",
            true,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let expected = {
            let opened = open(dir.path());
            opened.append(&index_fixture());
            opened.settle();
            assert_eq!(
                opened
                    .workspace
                    .flush(Instant::now() + Duration::from_secs(10))
                    .docs,
                []
            );
            snapshot(&opened.workspace)
        };
        let source = super::source_files(dir.path());
        tamper(dir.path(), sql);
        let opened = open(dir.path());
        let status = eventually(Duration::from_secs(10), || {
            let status = opened.workspace.index_status();
            (!matches!(status, IndexStatus::Rebuilding)).then_some(status)
        });
        assert_eq!(status, IndexStatus::Ready, "{sql}");
        opened.settle();
        assert_eq!(
            opened.statuses(),
            [IndexStatus::Rebuilding, IndexStatus::Ready],
            "{sql}"
        );
        let caches: Vec<_> = opened
            .events()
            .into_iter()
            .filter_map(|event| match event {
                IndexEvent::Cache(state) => Some(state),
                _ => None,
            })
            .collect();
        let mut expected_caches = Vec::new();
        if reused_first {
            expected_caches.push(CacheState::Reused);
        }
        expected_caches.push(CacheState::Replaced(CacheProblem::Corrupt(Corrupt::Field)));
        assert_eq!(caches, expected_caches, "{sql}");
        assert_eq!(snapshot(&opened.workspace), expected, "{sql}");
        assert_eq!(super::source_files(dir.path()), source, "{sql}");
    }
}

#[test]
fn backlinks_with_a_wrong_sqlite_field_type_rebuild_in_the_same_session() {
    for sql in [
        "UPDATE pages SET title = x'00' WHERE page_id = 'daily'",
        "UPDATE pages SET title = CAST(x'80' AS TEXT) WHERE page_id = 'daily'",
        "UPDATE blocks SET block_id = x'6434' WHERE block_id = 'd4'",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let opened = open(dir.path());
        opened.append(&index_fixture());
        opened.settle();
        let expected = snapshot(&opened.workspace);
        assert_eq!(
            opened
                .workspace
                .flush(Instant::now() + Duration::from_secs(10))
                .docs,
            []
        );
        let source = super::source_files(dir.path());
        let before = after_settled(&opened).len();
        tamper(dir.path(), sql);
        assert_eq!(
            opened.workspace.backlinks(&page("reading")),
            Ok(Reply::Indexing),
            "{sql}"
        );
        opened.settle();
        assert_eq!(
            after_settled(&opened)[before..],
            [
                IndexEvent::Status(IndexStatus::Rebuilding),
                IndexEvent::Cache(CacheState::Replaced(CacheProblem::Corrupt(Corrupt::Field))),
                IndexEvent::Status(IndexStatus::Ready),
            ],
            "{sql}"
        );
        assert_eq!(snapshot(&opened.workspace), expected, "{sql}");
        assert_eq!(super::source_files(dir.path()), source, "{sql}");
    }
}

#[test]
fn an_incremental_read_of_a_wrong_sqlite_field_type_rebuilds() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    opened.settle();
    let expected = snapshot(&opened.workspace);
    let before = after_settled(&opened).len();
    tamper(
        dir.path(),
        "UPDATE blocks SET block_id = x'726431' WHERE block_id = 'rd1'",
    );
    let reading = DocId::new("reading").unwrap();
    let update = opened.workspace.load(&reading).unwrap();
    opened.workspace.append(&reading, &update).unwrap();
    assert_eq!(
        opened
            .workspace
            .flush(Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
    let source = super::source_files(dir.path());
    let status = eventually(Duration::from_secs(10), || {
        let status = opened.workspace.index_status();
        (after_settled(&opened).len() > before).then_some(status)
    });
    assert!(!matches!(status, IndexStatus::Failed { .. }), "{status:?}");
    opened.settle();
    assert_eq!(opened.workspace.index_status(), IndexStatus::Ready);
    assert_eq!(
        after_settled(&opened)[before..],
        [
            IndexEvent::Status(IndexStatus::Rebuilding),
            IndexEvent::Cache(CacheState::Replaced(CacheProblem::Corrupt(Corrupt::Field))),
            IndexEvent::Status(IndexStatus::Ready),
        ]
    );
    assert_eq!(snapshot(&opened.workspace), expected);
    assert_eq!(super::source_files(dir.path()), source);
}

#[test]
fn the_worker_replaces_the_cache_when_it_reads_corrupt_segments() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    opened.settle();
    let before = after_settled(&opened).len();
    tamper(
        dir.path(),
        "UPDATE blocks SET segments = '[\"call \",{\"page\":7}]' WHERE block_id = 'd4'",
    );
    opened.edit_records(|records| {
        records.get_mut(&page("reading")).unwrap().title = "Reading queue".into();
    });
    opened.settle();
    assert_eq!(
        after_settled(&opened)[before..],
        [
            IndexEvent::Status(IndexStatus::Rebuilding),
            IndexEvent::Cache(CacheState::Replaced(CacheProblem::Corrupt(
                Corrupt::Segments
            ))),
            IndexEvent::Status(IndexStatus::Ready),
        ]
    );
    let links = ready(opened.workspace.backlinks(&page("reading")).unwrap());
    assert_eq!(links[0].blocks[0].text, "call Reading queue");
}

#[test]
fn a_cache_whose_keys_break_the_content_order_is_replaced_at_startup() {
    let dir = tempfile::tempdir().unwrap();
    {
        let opened = open(dir.path());
        opened.append(&index_fixture());
        opened.settle();
        assert_eq!(
            opened
                .workspace
                .flush(Instant::now() + Duration::from_secs(10))
                .docs,
            []
        );
    }
    {
        let conn = rusqlite::Connection::open(index_path(dir.path())).unwrap();
        conn.execute_batch(
            "UPDATE pages SET created_at = created_at + 1 WHERE page_id = 'untitled';
             PRAGMA journal_mode = DELETE;",
        )
        .unwrap();
    }
    let opened = open(dir.path());
    opened.settle();
    assert_eq!(
        after_settled(&opened)[..2],
        [
            IndexEvent::Status(IndexStatus::Rebuilding),
            IndexEvent::Cache(CacheState::Replaced(CacheProblem::Corrupt(Corrupt::Key))),
        ]
    );
    assert_eq!(
        hit_ids(&ready(
            opened.workspace.search(&content("cafe", 10, &[])).unwrap()
        )),
        ["reading"]
    );
}

#[test]
fn an_index_worker_that_cannot_start_fails_once_and_docs_stay_usable() {
    let dir = tempfile::tempdir().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let record = events.clone();
    let config = Config {
        spawn: |_, _| Err(io::Error::other("injected")),
        ..Config::DEFAULT
    };
    let workspace = crate::workspace::Workspace::start(
        Store::open(dir.path()).unwrap(),
        index_path(dir.path()),
        config,
        Arc::new(move |event: &IndexEvent| {
            record.lock().unwrap().push((
                event.clone(),
                std::thread::current().name().map(str::to_owned),
            ))
        }),
        Arc::default(),
    );
    let reason = "could not start the index worker: injected";
    assert_eq!(
        *events.lock().unwrap(),
        [(
            IndexEvent::Status(failed(reason)),
            std::thread::current().name().map(str::to_owned)
        )]
    );
    for (doc, update) in index_fixture() {
        workspace.append(&doc, &update).unwrap();
    }
    assert_eq!(workspace.index_status(), failed(reason));
    assert_eq!(
        workspace.search(&all("cafe", 5)),
        Err(QueryError::Failed(reason.into()))
    );
    assert_eq!(workspace.tree().unwrap().pages.len(), 6);
    assert!(
        !workspace
            .load(&DocId::new("reading").unwrap())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        workspace
            .flush(Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
    drop(workspace);
    assert_eq!(
        events.lock().unwrap().len(),
        1,
        "no retry and no later event"
    );
    assert!(!index_path(dir.path()).exists());
}
