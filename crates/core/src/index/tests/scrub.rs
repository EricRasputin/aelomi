use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{Opened, all, hit_ids, index_path, open, open_with, page, ready, source_files};
use crate::index::gate::Gate;
use crate::index::{CacheProblem, CacheState, Config, HoldPoint, IndexEvent};
use crate::storage::{DocId, Store};
use crate::workspace::fixture::{
    Editor, erase_page, index_fixture, link, paragraph, put_page, text, write_body,
};
use crate::workspace::records::{PageRecord, Placement};

const RAW: &str = "Zqxjvbetadelta";
const FOLDED: &str = "zqxjvbetadelta";
const INNER: &str = "betadel";
const NAMES: [&str; 4] = [
    "index.sqlite",
    "index.sqlite-wal",
    "index.sqlite-shm",
    "index.sqlite-journal",
];

fn secret_updates() -> Vec<(DocId, Vec<u8>)> {
    let mut updates = index_fixture();
    let mut editor = Editor::new(31);
    let workspace = DocId::workspace();
    for (doc, update) in &updates {
        if *doc == workspace {
            editor.apply(doc, update);
        }
    }
    let record = PageRecord {
        title: format!("Secret {RAW} plans"),
        placement: Placement {
            parent: None,
            order: 9,
        },
        favorite: None,
        trash: None,
        created_at: 1_700_000_000_500,
        updated_at: 1_700_000_000_500,
    };
    updates.push((
        workspace.clone(),
        editor.edit(&workspace, |txn| put_page(txn, &page("secret"), &record)),
    ));
    let secret = DocId::new("secret").unwrap();
    updates.push((
        secret.clone(),
        editor.edit(&secret, |txn| {
            write_body(
                txn,
                &[paragraph(
                    "s1",
                    vec![text(&format!("the {FOLDED} body and zqxjvalpha"))],
                )],
            )
        }),
    ));
    let neighbor = DocId::new("neighbor").unwrap();
    let mut other = record.clone();
    other.title = "Neighbor".into();
    updates.push((
        workspace.clone(),
        editor.edit(&workspace, |txn| put_page(txn, &page("neighbor"), &other)),
    ));
    updates.push((
        neighbor.clone(),
        editor.edit(&neighbor, |txn| {
            write_body(
                txn,
                &[paragraph(
                    "n1",
                    vec![text("zqxjvalpha stays "), link("secret")],
                )],
            )
        }),
    ));
    updates
}

fn erase_secret(workspace: &crate::workspace::Workspace) {
    let id = DocId::workspace();
    let mut editor = Editor::new(32);
    editor.apply(&id, &workspace.load(&id).unwrap());
    let update = editor.edit(&id, |txn| erase_page(txn, &page("secret")));
    workspace.append(&id, &update).unwrap();
    assert_eq!(
        workspace
            .flush(Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
}

fn marker_traces(dir: &Path) -> Vec<String> {
    let mut traces = Vec::new();
    let root = std::fs::canonicalize(dir).unwrap();
    for name in NAMES {
        let Ok(bytes) = std::fs::read(root.join(name)) else {
            continue;
        };
        for needle in [RAW, FOLDED, INNER] {
            if bytes.windows(needle.len()).any(|w| w == needle.as_bytes()) {
                traces.push(format!("{name} holds {needle}"));
            }
        }
    }
    if root.join(NAMES[0]).exists() {
        let copy = tempfile::tempdir().unwrap();
        for name in NAMES {
            if root.join(name).exists() {
                std::fs::copy(root.join(name), copy.path().join(name)).unwrap();
            }
        }
        let conn = rusqlite::Connection::open(copy.path().join(NAMES[0])).unwrap();
        for table in ["title_fts", "block_fts"] {
            conn.execute_batch(&format!(
                "CREATE VIRTUAL TABLE temp.v_{table} USING fts5vocab(main, '{table}', 'row');"
            ))
            .unwrap();
            let terms: i64 = conn
                .query_row(
                    &format!("SELECT count(*) FROM temp.v_{table} WHERE term LIKE '%{INNER}%'"),
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            if terms > 0 {
                traces.push(format!("{table} vocabulary holds {INNER}"));
            }
        }
    }
    traces
}

fn copy_folder(from: &Path) -> tempfile::TempDir {
    let to = tempfile::tempdir().unwrap();
    let mut stack: Vec<PathBuf> = vec![from.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let relative = entry.path().strip_prefix(from).unwrap().to_path_buf();
            if entry.file_type().unwrap().is_dir() {
                std::fs::create_dir_all(to.path().join(&relative)).unwrap();
                stack.push(entry.path());
            } else {
                std::fs::copy(entry.path(), to.path().join(&relative)).unwrap();
            }
        }
    }
    to
}

fn seeded(dir: &Path, gate: Arc<Gate>) -> Opened {
    let opened = open_with(
        Store::open(dir).unwrap(),
        index_path(dir),
        Config::DEFAULT,
        gate,
    );
    opened.append(&secret_updates());
    opened.settle();
    assert_eq!(
        opened
            .workspace
            .flush(Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
    opened
}

fn cache_states(opened: &Opened) -> Vec<CacheState> {
    opened
        .events()
        .into_iter()
        .filter_map(|e| match e {
            IndexEvent::Cache(state) => Some(state),
            _ => None,
        })
        .collect()
}

fn assert_converged(dir: &Path) {
    let source = source_files(dir);
    let reopened = open(dir);
    reopened.settle();
    assert_eq!(marker_traces(dir), Vec::<String>::new());
    assert_eq!(
        hit_ids(&ready(
            reopened.workspace.search(&all("zqxjvbeta", 10)).unwrap()
        )),
        Vec::<&str>::new()
    );
    let links = ready(reopened.workspace.backlinks(&page("secret")).unwrap());
    assert!(links.is_empty());
    assert_eq!(
        hit_ids(&ready(
            reopened
                .workspace
                .search(&all("stays deleted", 10))
                .unwrap()
        )),
        ["neighbor"],
        "the inbound link renders Deleted page"
    );
    drop(reopened);
    assert_eq!(
        source_files(dir),
        source,
        "the index never writes a source file"
    );
}

#[test]
fn erasing_a_page_replaces_the_cache_and_leaves_none_of_its_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let opened = seeded(dir.path(), Arc::default());
    assert_eq!(
        hit_ids(&ready(
            opened.workspace.search(&all("zqxjvbeta", 10)).unwrap()
        )),
        ["secret", "neighbor"],
        "the title, and the link that renders it"
    );
    assert!(
        marker_traces(dir.path()).len() >= 3,
        "the scan finds the marker before the erase: {:?}",
        marker_traces(dir.path())
    );

    let seeded_states = cache_states(&opened).len();
    erase_secret(&opened.workspace);
    let source = source_files(dir.path());
    opened.settle();
    assert_eq!(
        cache_states(&opened)[seeded_states..],
        [CacheState::Replaced(CacheProblem::ErasedPages { count: 1 })]
    );
    assert_eq!(marker_traces(dir.path()), Vec::<String>::new());
    let targets: i64 = rusqlite::Connection::open(index_path(dir.path()))
        .unwrap()
        .query_row(
            "SELECT count(*) FROM links WHERE target_id = 'secret'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(targets, 1, "the inbound link keeps its target");
    assert_eq!(
        hit_ids(&ready(
            opened.workspace.search(&all("stays deleted", 10)).unwrap()
        )),
        ["neighbor"]
    );
    assert_eq!(source_files(dir.path()), source);
    drop(opened);
    assert_converged(dir.path());
}

fn erase_without_an_index(dir: &Path) {
    let store = Store::open(dir).unwrap();
    let id = DocId::workspace();
    let mut editor = Editor::new(33);
    editor.apply(&id, &store.load(&id).unwrap());
    store
        .append(
            &id,
            &editor.edit(&id, |txn| erase_page(txn, &page("secret"))),
        )
        .unwrap();
    store
        .close(Instant::now() + Duration::from_secs(10))
        .unwrap();
}

#[test]
fn startup_replaces_a_cache_holding_a_page_with_no_record() {
    let dir = tempfile::tempdir().unwrap();
    drop(seeded(dir.path(), Arc::default()));
    erase_without_an_index(dir.path());
    assert!(!marker_traces(dir.path()).is_empty());
    let source = source_files(dir.path());
    let opened = open(dir.path());
    opened.settle();
    assert_eq!(
        cache_states(&opened),
        [
            CacheState::Reused,
            CacheState::Replaced(CacheProblem::ErasedPages { count: 1 })
        ],
        "the file passes its checks, then the reconcile finds a page without a record"
    );
    drop(opened);
    assert_eq!(source_files(dir.path()), source);
    assert_converged(dir.path());
}

#[test]
fn an_erase_interrupted_at_any_step_converges_on_restart() {
    for point in [
        HoldPoint::BeforeRemove,
        HoldPoint::AfterRemovingMain,
        HoldPoint::BeforeLoad(DocId::new("daily").unwrap()),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let gate = Arc::new(Gate::default());
        let opened = seeded(dir.path(), gate.clone());
        gate.arm(point.clone());
        erase_secret(&opened.workspace);
        gate.wait_held(&point);
        let image = copy_folder(dir.path());
        gate.release();
        opened.settle();
        drop(opened);
        if point == HoldPoint::BeforeRemove {
            assert!(!marker_traces(image.path()).is_empty(), "{point:?}");
        }
        assert_converged(image.path());
        assert_converged(dir.path());
    }
}

const V1_SCHEMA: &str = r#"
CREATE TABLE pages (
  rowid INTEGER PRIMARY KEY,
  page_id TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  parent_id TEXT,
  sibling_order INTEGER NOT NULL,
  favorite_order INTEGER,
  trash_batch_id TEXT,
  trashed_at INTEGER,
  trash_original_parent_id TEXT,
  trash_original_order INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  active INTEGER NOT NULL
);
CREATE TABLE blocks (
  rowid INTEGER PRIMARY KEY,
  page_id TEXT NOT NULL,
  block_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  segments TEXT NOT NULL,
  text TEXT NOT NULL
);
CREATE INDEX blocks_by_page ON blocks (page_id, ordinal);
CREATE TABLE links (
  target_id TEXT NOT NULL,
  block_rowid INTEGER NOT NULL,
  source_id TEXT NOT NULL,
  PRIMARY KEY (target_id, block_rowid)
) WITHOUT ROWID;
CREATE INDEX links_by_source ON links (source_id);
CREATE INDEX links_by_block ON links (block_rowid);
CREATE VIRTUAL TABLE title_fts USING fts5 (
  title, content = 'pages', content_rowid = 'rowid',
  tokenize = 'unicode61 remove_diacritics 2'
);
CREATE VIRTUAL TABLE block_fts USING fts5 (
  text, content = 'blocks', content_rowid = 'rowid',
  tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER pages_insert AFTER INSERT ON pages BEGIN
  INSERT INTO title_fts (rowid, title) VALUES (new.rowid, new.title);
END;
CREATE TRIGGER pages_delete AFTER DELETE ON pages BEGIN
  INSERT INTO title_fts (title_fts, rowid, title) VALUES ('delete', old.rowid, old.title);
END;
CREATE TRIGGER pages_title AFTER UPDATE OF title ON pages BEGIN
  INSERT INTO title_fts (title_fts, rowid, title) VALUES ('delete', old.rowid, old.title);
  INSERT INTO title_fts (rowid, title) VALUES (new.rowid, new.title);
END;
CREATE TRIGGER blocks_insert AFTER INSERT ON blocks BEGIN
  INSERT INTO block_fts (rowid, text) VALUES (new.rowid, new.text);
END;
CREATE TRIGGER blocks_delete AFTER DELETE ON blocks BEGIN
  INSERT INTO block_fts (block_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
END;
CREATE TRIGGER blocks_text AFTER UPDATE OF text ON blocks BEGIN
  INSERT INTO block_fts (block_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
  INSERT INTO block_fts (rowid, text) VALUES (new.rowid, new.text);
END;
"#;

#[test]
fn a_legacy_cache_with_freed_marker_bytes_is_replaced_whole() {
    let dir = tempfile::tempdir().unwrap();
    drop(seeded(dir.path(), Arc::default()));
    erase_without_an_index(dir.path());
    let path = index_path(dir.path());
    crate::workspace::fixture::break_index(&path, crate::workspace::fixture::BrokenIndex::Missing)
        .unwrap();
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(V1_SCHEMA).unwrap();
        conn.execute(
            "INSERT INTO blocks (page_id, block_id, ordinal, segments, text) VALUES ('gone', 'g1', 0, '[]', ?1)",
            [format!("{RAW} {FOLDED}")],
        )
        .unwrap();
        conn.execute("DELETE FROM blocks WHERE page_id = 'gone'", [])
            .unwrap();
        conn.execute_batch("PRAGMA user_version = 1; PRAGMA journal_mode = DELETE;")
            .unwrap();
    }
    assert!(
        !marker_traces(dir.path()).is_empty(),
        "the freed bytes hold the marker"
    );
    let opened = open(dir.path());
    opened.settle();
    assert_eq!(
        cache_states(&opened),
        [CacheState::Replaced(CacheProblem::SchemaVersion {
            found: 1,
            expected: 2
        })]
    );
    drop(opened);
    assert_converged(dir.path());
}

#[test]
fn sidecars_left_without_the_main_file_are_removed_before_sqlite_opens_them() {
    let dir = tempfile::tempdir().unwrap();
    let opened = seeded(dir.path(), Arc::default());
    let image = copy_folder(dir.path());
    drop(opened);
    let wal = std::fs::read(image.path().join("index.sqlite-wal")).unwrap();
    assert!(
        wal.windows(FOLDED.len()).any(|w| w == FOLDED.as_bytes()),
        "the live write-ahead log holds the marker"
    );
    std::fs::remove_file(image.path().join("index.sqlite")).unwrap();
    erase_without_an_index(image.path());
    let source = source_files(image.path());
    let reopened = open(image.path());
    reopened.settle();
    assert_eq!(cache_states(&reopened), [CacheState::Created]);
    drop(reopened);
    assert_eq!(source_files(image.path()), source);
    assert_converged(image.path());
}
