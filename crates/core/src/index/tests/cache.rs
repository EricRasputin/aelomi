use std::cmp::Reverse;
use std::collections::BTreeMap;

use crate::index::db::{
    CacheProblem, Corrupt, KEY_SPACING, OrderKey, PAGE_KEYS, PageKey, SCHEMA, SCHEMA_VERSION,
    SLOTS, StoredSegments, allocate,
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

/// `check()` compares a cache with this schema, so an edit to `SCHEMA` without a version bump
/// would silently rebuild every cache as a schema mismatch. Bump `SCHEMA_VERSION` with any
/// schema change, then update this pin.
#[test]
fn the_schema_text_is_pinned_to_its_version() {
    assert_eq!(
        (SCHEMA_VERSION, crc32fast::hash(SCHEMA.as_bytes())),
        (2, 0x045e_9df2)
    );
}
