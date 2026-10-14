use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{hit_ids, index_path, open, page, ready, snapshot};
use crate::index::{
    Backlink, BacklinkBlock, CacheProblem, CacheState, Config, Index, IndexEvent, IndexStatus,
    SearchHit, TextPart,
};
use crate::workspace::body::BlockId;
use crate::workspace::fixture::{
    BrokenIndex, break_index, index_fixture, link, paragraph, text, write_body,
};
use crate::workspace::records::{PageId, PageRecord, Trash};

fn backlink(source: &str, title: &str, blocks: &[(&str, &str)]) -> Backlink {
    Backlink {
        page_id: page(source),
        title: title.into(),
        blocks: blocks
            .iter()
            .map(|(id, text)| BacklinkBlock {
                block_id: BlockId::new(*id).unwrap(),
                text: (*text).into(),
            })
            .collect(),
    }
}

fn source_files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
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

fn cache_state(events: &[IndexEvent]) -> CacheState {
    events
        .iter()
        .find_map(|e| match e {
            IndexEvent::Cache(state) => Some(state.clone()),
            _ => None,
        })
        .expect("the worker reported the cache")
}

#[test]
fn startup_reports_rebuilding_on_the_worker_before_opening_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let path = index_path(dir.path());
    let events = Arc::new(Mutex::new(Vec::new()));
    let record = events.clone();
    let observed_path = path.clone();
    let index = Index::start(
        Arc::new(super::Docs(BTreeMap::new())),
        path,
        Config::DEFAULT,
        Arc::new(move |event| {
            record.lock().unwrap().push((
                event.clone(),
                std::thread::current().name().map(str::to_owned),
                observed_path.exists(),
            ));
        }),
        Arc::default(),
    );
    assert!(index.shared().settle(Duration::from_secs(10)));
    assert_eq!(
        *events.lock().unwrap(),
        [
            (
                IndexEvent::Status(IndexStatus::Rebuilding),
                Some("aelomi-index".into()),
                false,
            ),
            (
                IndexEvent::Cache(CacheState::Created),
                Some("aelomi-index".into()),
                true,
            ),
            (
                IndexEvent::Status(IndexStatus::Ready),
                Some("aelomi-index".into()),
                true,
            ),
        ]
    );
}

#[test]
fn the_fixture_indexes_to_its_exact_tree_backlinks_and_searches() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    opened.settle();
    let snap = snapshot(&opened.workspace);

    let tree: Vec<(&str, Option<&str>, &str)> = snap
        .tree
        .pages
        .iter()
        .map(|p| {
            (
                p.id.as_str(),
                p.parent.as_ref().map(|p| p.as_str()),
                p.title.as_str(),
            )
        })
        .collect();
    assert_eq!(
        tree,
        [
            ("projects", None, "Projects"),
            ("aelomi", Some("projects"), "Aelomi"),
            ("roadmap", Some("aelomi"), "Roadmap"),
            ("reading", Some("projects"), "Reading list"),
            ("daily", None, "Daily"),
            ("untitled", None, ""),
        ]
    );
    assert_eq!(snap.tree.favorites, [page("reading"), page("aelomi")]);
    let trash: Vec<(&str, &str, u64)> = snap
        .tree
        .trash
        .iter()
        .map(|t| (t.id.as_str(), t.batch_id.as_str(), t.trashed_at))
        .collect();
    assert_eq!(trash, [("archive", "batch-archive", 1_700_000_900_000)]);

    let none: Vec<Backlink> = Vec::new();
    let expected = BTreeMap::from([
        (
            page("aelomi"),
            vec![
                backlink(
                    "daily",
                    "Daily",
                    &[("d1", "see Aelomi today"), ("d7", "Aelomi and Aelomi")],
                ),
                backlink("roadmap", "Roadmap", &[("r1", "Milestones for Aelomi")]),
            ],
        ),
        (
            page("roadmap"),
            vec![backlink(
                "aelomi",
                "Aelomi",
                &[("a3", "links to Roadmap and Untitled")],
            )],
        ),
        (
            page("untitled"),
            vec![backlink(
                "aelomi",
                "Aelomi",
                &[("a3", "links to Roadmap and Untitled")],
            )],
        ),
        (
            page("reading"),
            vec![backlink("daily", "Daily", &[("d4", "call Reading list")])],
        ),
        (
            page("daily"),
            vec![backlink(
                "untitled",
                "",
                &[("u1", "untitled body mentions Daily")],
            )],
        ),
        (page("projects"), none.clone()),
        (page("archive"), none.clone()),
        (page("old-notes"), none.clone()),
        (page("erased"), none),
    ]);
    assert_eq!(snap.backlinks, expected);

    let searches: Vec<(&str, Vec<&str>)> = snap
        .searches
        .iter()
        .map(|(q, hits)| (*q, hit_ids(hits)))
        .collect();
    assert_eq!(
        searches,
        [
            ("roadm", vec!["daily", "roadmap", "aelomi"]),
            ("quillmere", vec![]),
            ("orphanword", vec![]),
            ("cafe", vec!["reading"]),
            ("kestrel", vec!["aelomi"]),
            ("deleted page", vec!["daily"]),
            ("line two", vec!["roadmap"]),
            ("diagram", vec!["projects"]),
            ("archived", vec!["daily"]),
            ("aelomi app", vec!["aelomi"]),
            ("untitled", vec!["untitled", "aelomi"]),
            ("", vec![]),
            ("\"", vec![]),
            ("Mileston*", vec!["roadmap"]),
        ]
    );
    type Parts = Vec<(String, bool)>;
    let parts = |hit: &SearchHit| -> (Parts, Option<(String, Parts)>) {
        let parts = |parts: &[TextPart]| {
            parts
                .iter()
                .map(|p| (p.text.clone(), p.highlighted))
                .collect()
        };
        (
            parts(&hit.title),
            hit.block
                .as_ref()
                .map(|b| (b.block_id.as_str().to_owned(), parts(&b.text))),
        )
    };
    let s = |t: &str, h: bool| (t.to_owned(), h);
    let roadm: Vec<_> = snap.searches[0].1.iter().map(parts).collect();
    assert_eq!(
        roadm,
        [
            (
                vec![s("Daily", false)],
                Some((
                    "d2".into(),
                    vec![
                        s("quarterly ", false),
                        s("roadmap", true),
                        s(" review", false)
                    ]
                )),
            ),
            (vec![s("Roadmap", true)], None),
            (
                vec![s("Aelomi", false)],
                Some((
                    "a3".into(),
                    vec![
                        s("links to ", false),
                        s("Roadmap", true),
                        s(" and Untitled", false)
                    ]
                )),
            ),
        ]
    );
    let cafe: Vec<_> = snap.searches[3].1.iter().map(parts).collect();
    assert_eq!(
        cafe,
        [(
            vec![s("Reading list", false)],
            Some((
                "rd1".into(),
                vec![
                    s("Books about ", false),
                    s("café", true),
                    s(" culture", false)
                ]
            )),
        )]
    );
    let untitled = &snap.searches[10].1[0];
    assert_eq!(untitled.title, []);
    assert_eq!(
        snap.searches[10].1[1]
            .block
            .as_ref()
            .unwrap()
            .block_id
            .as_str(),
        "a3"
    );
}

fn reopen_and_compare(dir: &Path, expected: &super::Snapshot) -> (CacheState, Vec<IndexStatus>) {
    let opened = open(dir);
    assert_eq!(opened.workspace.index_status(), IndexStatus::Rebuilding);
    assert!(
        matches!(
            opened.workspace.search("roadm", 10),
            Ok(crate::index::Reply::Indexing) | Ok(crate::index::Reply::Ready(_))
        ),
        "a query never waits for the rebuild"
    );
    opened.settle();
    assert_eq!(&snapshot(&opened.workspace), expected);
    (cache_state(&opened.events()), opened.statuses())
}

fn corrupt_a_btree_page(path: &Path) {
    let (root, page_size): (i64, i64) = {
        let conn = rusqlite::Connection::open(path).unwrap();
        let root = conn
            .query_row(
                "SELECT rootpage FROM sqlite_schema WHERE name = 'blocks_by_page'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let size = conn
            .pragma_query_value(None, "page_size", |row| row.get(0))
            .unwrap();
        conn.pragma_update(None, "journal_mode", "DELETE").unwrap();
        (root, size)
    };
    let mut bytes = std::fs::read(path).unwrap();
    let end = (root * page_size) as usize;
    for byte in &mut bytes[end - 64..end] {
        *byte = 0x5a;
    }
    std::fs::write(path, bytes).unwrap();
}

fn plant_a_ghost_fts_row(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute(
        "INSERT INTO block_fts (rowid, text) VALUES (999999, 'ghost')",
        [],
    )
    .unwrap();
    conn.pragma_update(None, "journal_mode", "DELETE").unwrap();
}

#[test]
fn every_broken_index_rebuilds_to_the_same_results_and_source_files_never_change() {
    let dir = tempfile::tempdir().unwrap();
    let expected = {
        let opened = open(dir.path());
        opened.append(&index_fixture());
        opened.settle();
        assert_eq!(
            opened
                .workspace
                .flush(Instant::now() + Duration::from_secs(10)),
            []
        );
        snapshot(&opened.workspace)
    };
    let source = source_files(dir.path());
    assert!(
        source.keys().any(|name| name.starts_with("docs/erased/")),
        "the erased page's body files remain"
    );
    let path = index_path(dir.path());

    let (cache, statuses) = reopen_and_compare(dir.path(), &expected);
    assert_eq!(cache, CacheState::Reused);
    assert_eq!(statuses, [IndexStatus::Rebuilding, IndexStatus::Ready]);

    break_index(&path, BrokenIndex::Missing).unwrap();
    assert_eq!(
        reopen_and_compare(dir.path(), &expected).0,
        CacheState::Created
    );

    break_index(&path, BrokenIndex::Garbage).unwrap();
    assert_eq!(
        reopen_and_compare(dir.path(), &expected).0,
        CacheState::Replaced(CacheProblem::NotADatabase("file is not a database".into()))
    );

    break_index(&path, BrokenIndex::OldSchema).unwrap();
    assert_eq!(
        reopen_and_compare(dir.path(), &expected).0,
        CacheState::Replaced(CacheProblem::OldSchema { version: 0 })
    );

    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE extra (x); PRAGMA journal_mode = DELETE;")
            .unwrap();
    }
    assert_eq!(
        reopen_and_compare(dir.path(), &expected).0,
        CacheState::Replaced(CacheProblem::SchemaMismatch)
    );

    corrupt_a_btree_page(&path);
    let cache = reopen_and_compare(dir.path(), &expected).0;
    assert!(
        matches!(cache, CacheState::Replaced(CacheProblem::IntegrityCheck(_))),
        "{cache:?}"
    );

    plant_a_ghost_fts_row(&path);
    let cache = reopen_and_compare(dir.path(), &expected).0;
    assert!(
        matches!(
            cache,
            CacheState::Replaced(CacheProblem::FtsIntegrityCheck(_))
        ),
        "{cache:?}"
    );

    assert_eq!(
        source_files(dir.path()),
        source,
        "the index never writes a source file"
    );
}

fn trash_batch(records: &mut BTreeMap<PageId, PageRecord>, ids: &[&str], batch: &str) {
    for id in ids {
        let record = records.get_mut(&page(id)).unwrap();
        record.trash = Some(Trash {
            original_parent: record.placement.parent.clone(),
            original_order: record.placement.order,
            batch_id: batch.into(),
            trashed_at: 1_700_001_000_000,
        });
    }
}

#[test]
fn trashed_and_erased_pages_never_appear_as_source_or_target() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    let ws = &opened.workspace;
    opened.append(&index_fixture());
    opened.settle();
    let search = |q: &str| {
        let hits = ready(ws.search(q, 50).unwrap());
        let mut ids = hit_ids(&hits);
        ids.sort();
        ids.join(",")
    };
    let sources = |target: &str| {
        ready(ws.backlinks(&page(target)).unwrap())
            .iter()
            .map(|b| b.page_id.as_str().to_owned())
            .collect::<Vec<_>>()
            .join(",")
    };

    opened.edit_records(|records| {
        for id in ["archive", "old-notes"] {
            records.get_mut(&page(id)).unwrap().trash = None;
        }
    });
    opened.settle();
    assert_eq!(search("quillmere"), "old-notes");
    assert_eq!(search("archived"), "archive,daily");
    assert_eq!(sources("aelomi"), "daily,archive,roadmap");
    assert_eq!(sources("daily"), "old-notes,untitled");
    assert_eq!(sources("archive"), "daily");
    assert_eq!(
        ws.tree().unwrap().favorites,
        [page("reading"), page("aelomi"), page("old-notes")]
    );

    opened.edit_records(|records| trash_batch(records, &["archive", "old-notes"], "batch-again"));
    opened.settle();
    assert_eq!(search("quillmere"), "");
    assert_eq!(search("archived"), "daily");
    assert_eq!(sources("aelomi"), "daily,roadmap");
    assert_eq!(sources("daily"), "untitled");
    assert_eq!(sources("archive"), "");
    assert_eq!(sources("old-notes"), "");

    opened.edit_records(|records| {
        records.get_mut(&page("old-notes")).unwrap().trash = None;
    });
    opened.settle();
    assert_eq!(
        search("quillmere"),
        "",
        "a descendant of a trashed page stays hidden"
    );
    assert_eq!(sources("daily"), "untitled");

    opened.edit_records(|records| {
        let mut gone = records[&page("reading")].clone();
        gone.title = "Vanishing".into();
        gone.favorite = None;
        records.insert(page("gone"), gone);
    });
    opened.edit_body("gone", |txn| {
        write_body(
            txn,
            &[paragraph("g1", vec![text("vanishword "), link("roadmap")])],
        )
    });
    opened.settle();
    assert_eq!(search("vanishword"), "gone");
    assert_eq!(search("vanishing"), "gone");
    assert_eq!(sources("roadmap"), "aelomi,gone");

    opened.edit_records(|records| {
        records.remove(&page("gone"));
    });
    opened.settle();
    assert_eq!(search("vanishword"), "");
    assert_eq!(search("vanishing"), "");
    assert_eq!(sources("roadmap"), "aelomi");
    assert_eq!(sources("gone"), "");
    assert_eq!(search("orphanword"), "");
    assert_eq!(ws.flush(Instant::now() + Duration::from_secs(10)), []);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    assert!(
        root.join("docs/gone").is_dir(),
        "the erased page's body files remain"
    );
    assert!(root.join("docs/erased").is_dir());
}

#[test]
fn a_symlink_at_an_index_name_is_removed_and_its_target_never_touched() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let user_file = elsewhere.path().join("notes.txt");
    std::fs::write(&user_file, b"user data").unwrap();
    let path = index_path(dir.path());
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    std::os::unix::fs::symlink(&user_file, &path).unwrap();
    std::os::unix::fs::symlink(&user_file, &wal).unwrap();

    let opened = open(dir.path());
    opened.append(&index_fixture());
    opened.settle();

    assert_eq!(
        cache_state(&opened.events()),
        CacheState::Replaced(CacheProblem::NotAFile)
    );
    assert_eq!(
        hit_ids(&ready(opened.workspace.search("cafe", 5).unwrap())),
        ["reading"]
    );
    assert!(std::fs::symlink_metadata(&path).unwrap().is_file());
    assert_eq!(std::fs::read(&user_file).unwrap(), b"user data");
}

#[test]
fn sidecars_left_without_an_index_are_deleted_before_it_is_created() {
    let dir = tempfile::tempdir().unwrap();
    let path = index_path(dir.path());
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    std::fs::write(&wal, b"a write-ahead log from another index").unwrap();

    let opened = open(dir.path());
    opened.append(&index_fixture());
    opened.settle();

    assert_eq!(cache_state(&opened.events()), CacheState::Created);
    assert_eq!(opened.workspace.index_status(), IndexStatus::Ready);
    assert_eq!(
        hit_ids(&ready(opened.workspace.search("cafe", 5).unwrap())),
        ["reading"]
    );
}
