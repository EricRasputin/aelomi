use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::index_path;
use crate::index::HoldPoint;
use crate::index::db::{
    CacheOpen, CacheProblem, CacheState, Corrupt, KEY_SPACING, OrderKey, PAGE_KEYS, PageKey,
    ReplaceKind, SCHEMA, SCHEMA_VERSION, SLOTS, StoredSegments, Writer, allocate,
};
use crate::workspace::body::Segment;
use crate::workspace::records::PageId;

fn order(created_at: u64, id: &str) -> OrderKey {
    (created_at, Reverse(PageId::new(id).unwrap()))
}

fn keys(keys: &[i64]) -> Vec<PageKey> {
    keys.iter().map(|k| PageKey::new(*k).unwrap()).collect()
}

fn existing(pages: &[(OrderKey, i64)]) -> BTreeMap<OrderKey, PageKey> {
    pages
        .iter()
        .map(|(o, k)| (o.clone(), PageKey::new(*k).unwrap()))
        .collect()
}

#[test]
fn a_new_cache_spaces_its_keys_in_content_order() {
    assert_eq!(
        allocate(
            &BTreeMap::new(),
            &[order(1, "c"), order(2, "b"), order(2, "a")]
        ),
        Some(keys(&[KEY_SPACING, 2 * KEY_SPACING, 3 * KEY_SPACING]))
    );
    assert!(
        order(2, "b") < order(2, "a"),
        "a later page ID sorts first among pages created together"
    );
}

#[test]
fn new_pages_append_after_the_newest_or_split_a_gap() {
    let cached = existing(&[(order(10, "m"), 100), (order(20, "m"), 200)]);
    assert_eq!(
        allocate(&cached, &[order(30, "x")]),
        Some(keys(&[200 + KEY_SPACING]))
    );
    assert_eq!(allocate(&cached, &[order(15, "x")]), Some(keys(&[150])));
    assert_eq!(allocate(&cached, &[order(5, "x")]), Some(keys(&[50])));
    assert_eq!(
        allocate(&cached, &[order(5, "x"), order(15, "x"), order(30, "x")]),
        Some(keys(&[50, 150, 200 + KEY_SPACING]))
    );
    let batch: Vec<OrderKey> = (0..99).map(|n| order(15, &format!("p{n:03}"))).collect();
    let mut sorted = batch.clone();
    sorted.sort();
    assert_eq!(
        allocate(&cached, &sorted),
        Some(keys(&(101..200).collect::<Vec<_>>())),
        "99 pages fill the 99 free keys in order"
    );
}

#[test]
fn a_full_gap_or_key_space_has_no_key() {
    let cached = existing(&[(order(10, "m"), 100), (order(20, "m"), 200)]);
    let batch: Vec<OrderKey> = (0..100).map(|n| order(15, &format!("p{n:03}"))).collect();
    let mut sorted = batch;
    sorted.sort();
    assert_eq!(allocate(&cached, &sorted), None);
    assert_eq!(
        allocate(
            &existing(&[(order(1, "a"), 1), (order(3, "a"), 2)]),
            &[order(2, "a")]
        ),
        None
    );
    let last = existing(&[(order(1, "a"), PAGE_KEYS - 1)]);
    assert_eq!(allocate(&last, &[order(2, "a")]), None);
    let near = existing(&[(order(1, "a"), PAGE_KEYS - 3)]);
    assert_eq!(
        allocate(&near, &[order(2, "b"), order(2, "a")]),
        Some(keys(&[PAGE_KEYS - 2, PAGE_KEYS - 1])),
        "near the end of the key space new pages pack tighter"
    );
}

#[test]
fn a_thousand_pages_in_one_gap_spread_evenly_in_order() {
    let cached = existing(&[
        (order(10, "m"), KEY_SPACING),
        (order(20, "m"), 2 * KEY_SPACING),
    ]);
    let mut batch: Vec<OrderKey> = (0..1000).map(|n| order(15, &format!("p{n:04}"))).collect();
    batch.sort();
    let got = allocate(&cached, &batch).unwrap();
    assert_eq!(got.len(), 1000);
    assert_eq!(got[0], PageKey::new(KEY_SPACING + 65).unwrap());
    assert_eq!(got[999], PageKey::new(2 * KEY_SPACING - 66).unwrap());
    assert!(got.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn block_rowids_pack_a_page_key_and_a_slot_into_a_positive_integer() {
    assert_eq!(PageKey::new(0), None);
    assert_eq!(PageKey::new(PAGE_KEYS), None);
    let last = PageKey::new(PAGE_KEYS - 1).unwrap();
    assert_eq!(last.blocks(), (last.block(0), i64::MAX));
    assert_eq!(PageKey::of_block(i64::MAX), Ok(last));
    assert_eq!(PageKey::of_block(-1), Err(Corrupt::Key));
    assert_eq!(PageKey::of_block(SLOTS - 1), Err(Corrupt::Key));
    let page = PageKey::new(3).unwrap();
    assert_eq!(page.block(5), 3 * SLOTS + 5);
    assert_eq!(PageKey::of_block(page.block(SLOTS - 1)), Ok(page));
}

#[test]
fn a_backlink_from_a_page_s_last_block_slot_names_that_page() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    let page = |key: i64, id: &str, title: &str| {
        conn.execute(
            "INSERT INTO pages (rowid, page_id, title, sibling_order, created_at, updated_at, active)
             VALUES (?1, ?2, ?3, 0, ?1, ?1, 1)",
            rusqlite::params![key, id, title],
        )
        .unwrap();
    };
    for (key, id) in [(1, "decoy-1"), (6, "decoy-6"), (7, "decoy-7")] {
        page(key, id, "Decoy");
    }
    page(3, "source", "Source");
    page(9, "target", "Target");
    let rowid = PageKey::new(3).unwrap().block(SLOTS - 1);
    conn.execute_batch(&format!(
        "INSERT INTO blocks (rowid, block_id, ordinal, segments) VALUES ({rowid}, 'b-last', 0, '[]');
         INSERT INTO block_text (rowid, text) VALUES ({rowid}, 'see Target');
         INSERT INTO links (target_id, block_rowid) VALUES ('target', {rowid});"
    ))
    .unwrap();
    let target = PageId::new("target").unwrap();
    assert_eq!(
        crate::index::query::backlinks(&mut conn, &target).unwrap(),
        vec![crate::index::Backlink {
            page_id: PageId::new("source").unwrap(),
            title: "Source".into(),
            blocks: vec![crate::index::BacklinkBlock {
                block_id: crate::workspace::body::BlockId::new("b-last").unwrap(),
                text: "see Target".into(),
            }],
        }]
    );
}

#[test]
fn stored_segments_round_trip_and_reject_anything_else() {
    let segments = vec![
        Segment::Text("see ".into()),
        Segment::Link(PageId::new("aelomi").unwrap()),
        Segment::Text(" {\"page\":\"x\"} \u{E000}".into()),
    ];
    let json = StoredSegments::encode(&segments);
    assert_eq!(
        json,
        "[\"see \",{\"page\":\"aelomi\"},\" {\\\"page\\\":\\\"x\\\"} \u{E000}\"]"
    );
    assert_eq!(StoredSegments::decode(&json), Ok(segments));
    assert_eq!(StoredSegments::decode("[]"), Ok(vec![]));
    for bad in [
        "",
        "not json",
        "{}",
        "[1]",
        "[null]",
        r#"[{"page":"Not An ID"}]"#,
        r#"[{"page":"workspace"}]"#,
        r#"[{"page":"a","extra":1}]"#,
        r#"[{"pg":"a"}]"#,
        r#"["ok",{"page":7}]"#,
    ] {
        assert_eq!(StoredSegments::decode(bad), Err(Corrupt::Segments), "{bad}");
    }
}

#[test]
fn cache_problems_read_as_their_log_labels() {
    let labels: Vec<String> = [
        CacheProblem::SchemaVersion {
            found: 1,
            expected: 2,
        },
        CacheProblem::SchemaVersion {
            found: 3,
            expected: 2,
        },
        CacheProblem::ErasedPages { count: 2 },
        CacheProblem::Corrupt(Corrupt::Segments),
        CacheProblem::KeySpace,
        CacheProblem::OrderChanged,
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    assert_eq!(
        labels,
        [
            "schema-version (found 1, expected 2)",
            "schema-version (found 3, expected 2)",
            "erased-pages (count 2)",
            "corrupt (segments)",
            "key-space",
            "order-changed",
        ]
    );
}

#[test]
fn damage_backs_off_and_an_outdated_cache_does_not() {
    let kinds: Vec<(String, ReplaceKind)> = [
        CacheProblem::NotAFile,
        CacheProblem::NotADatabase("e".into()),
        CacheProblem::SchemaMismatch,
        CacheProblem::IntegrityCheck("e".into()),
        CacheProblem::FtsIntegrityCheck("e".into()),
        CacheProblem::Corrupt(Corrupt::Key),
        CacheProblem::SchemaVersion {
            found: 3,
            expected: 2,
        },
        CacheProblem::ErasedPages { count: 1 },
        CacheProblem::KeySpace,
        CacheProblem::OrderChanged,
    ]
    .iter()
    .map(|problem| (problem.to_string(), problem.kind()))
    .collect();
    let kind = |label: &str, kind| (label.to_owned(), kind);
    assert_eq!(
        kinds,
        [
            kind("not-a-file", ReplaceKind::Fault),
            kind("not-a-database (e)", ReplaceKind::Fault),
            kind("schema-mismatch", ReplaceKind::Fault),
            kind("integrity-check (e)", ReplaceKind::Fault),
            kind("fts-integrity-check (e)", ReplaceKind::Fault),
            kind("corrupt (key)", ReplaceKind::Fault),
            kind(
                "schema-version (found 3, expected 2)",
                ReplaceKind::Outdated
            ),
            kind("erased-pages (count 1)", ReplaceKind::Outdated),
            kind("key-space", ReplaceKind::Outdated),
            kind("order-changed", ReplaceKind::Outdated),
        ]
    );
}

#[test]
fn the_schema_text_is_pinned_to_its_version() {
    assert_eq!(
        (SCHEMA_VERSION, crc32fast::hash(SCHEMA.as_bytes())),
        (2, 0x045e_9df2),
        "SCHEMA's bytes changed. If the schema was not meant to change, restore the exact bytes. \
         A real schema change needs a SCHEMA_VERSION bump, a test that opens a cache written at \
         the old version, and then the new version and checksum pinned here together."
    );
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn held_open(path: &Path, opening: CacheOpen) -> (Result<CacheState, String>, Vec<HoldPoint>) {
    let held = RefCell::new(Vec::new());
    let opened = Writer::open(path, opening, &|point| held.borrow_mut().push(point));
    (
        opened.map(|o| o.state).map_err(|e| e.to_string()),
        held.into_inner(),
    )
}

const BOTH_HOLDS: [HoldPoint; 2] = [HoldPoint::BeforeRemove, HoldPoint::AfterRemovingMain];

#[test]
fn a_special_entry_outranks_a_requested_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let user_file = elsewhere.path().join("notes.txt");
    std::fs::write(&user_file, b"user data").unwrap();
    let path = index_path(dir.path());
    std::os::unix::fs::symlink(&user_file, sidecar(&path, "-shm")).unwrap();

    assert_eq!(
        held_open(&path, CacheOpen::ForceReplace(CacheProblem::KeySpace)),
        (
            Ok(CacheState::Replaced(CacheProblem::NotAFile)),
            BOTH_HOLDS.to_vec()
        )
    );
    assert!(std::fs::symlink_metadata(&path).unwrap().is_file());
    assert_eq!(std::fs::read(&user_file).unwrap(), b"user data");
}

#[test]
fn a_missing_main_file_still_passes_both_hold_points() {
    for (opening, state) in [
        (
            CacheOpen::ForceReplace(CacheProblem::OrderChanged),
            CacheState::Replaced(CacheProblem::OrderChanged),
        ),
        (CacheOpen::ReuseOrCreate, CacheState::Created),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = index_path(dir.path());
        assert_eq!(held_open(&path, opening), (Ok(state), BOTH_HOLDS.to_vec()));
        assert!(std::fs::symlink_metadata(&path).unwrap().is_file());
    }
}

#[test]
fn a_folder_at_a_sidecar_name_fails_and_is_never_removed() {
    for opening in [
        CacheOpen::ReuseOrCreate,
        CacheOpen::ForceReplace(CacheProblem::KeySpace),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = index_path(dir.path());
        std::fs::write(&path, b"kept").unwrap();
        std::fs::create_dir(sidecar(&path, "-wal")).unwrap();
        assert_eq!(
            held_open(&path, opening),
            (
                Err("index.sqlite-wal is a folder, so the index cannot be created".into()),
                vec![]
            )
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"kept");
        assert!(sidecar(&path, "-wal").is_dir());
    }

    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let user_file = elsewhere.path().join("notes.txt");
    std::fs::write(&user_file, b"user data").unwrap();
    let path = index_path(dir.path());
    std::os::unix::fs::symlink(&user_file, &path).unwrap();
    std::fs::create_dir(sidecar(&path, "-shm")).unwrap();
    assert_eq!(
        held_open(&path, CacheOpen::ReuseOrCreate),
        (
            Err("index.sqlite-shm is a folder, so the index cannot be created".into()),
            BOTH_HOLDS.to_vec()
        ),
        "a special main file is replaced before the folder is reached"
    );
    assert_eq!(
        std::fs::symlink_metadata(&path).map_err(|e| e.kind()).err(),
        Some(io::ErrorKind::NotFound)
    );
    assert_eq!(std::fs::read(&user_file).unwrap(), b"user data");
    assert!(sidecar(&path, "-shm").is_dir());
}
