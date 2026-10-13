use std::collections::{BTreeMap, HashMap};

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{Docs, eventually, hit_ids, ready};
use crate::index::{Config, Index, IndexStatus};
use crate::storage::DocId;
use crate::workspace::fixture::{Editor, Inline, paragraph, put_page, write_body};
use crate::workspace::records::{PageId, PageRecord, Placement, Trash};

fn generated(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/generated")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

fn fixture_docs() -> Docs {
    let mut editor = Editor::new(11);
    let mut docs = BTreeMap::new();
    let mut records = Vec::new();
    let mut siblings: HashMap<Option<String>, u64> = HashMap::new();
    for line in generated("search-10k-pages.jsonl").lines() {
        let page: serde_json::Value = serde_json::from_str(line).unwrap();
        let id = PageId::new(page["id"].as_str().unwrap()).unwrap();
        let parent = page["parentId"].as_str().map(|p| PageId::new(p).unwrap());
        let order = siblings
            .entry(parent.as_ref().map(|p| p.as_str().to_owned()))
            .or_default();
        let trash = page["trash"].as_object().map(|trash| Trash {
            original_parent: parent.clone(),
            original_order: *order,
            batch_id: trash["batchId"].as_str().unwrap().to_owned(),
            trashed_at: trash["at"].as_u64().unwrap(),
        });
        records.push((
            id.clone(),
            PageRecord {
                title: page["title"].as_str().unwrap().to_owned(),
                placement: Placement {
                    parent,
                    order: *order,
                },
                favorite: None,
                trash,
                created_at: 1_700_000_000_000,
                updated_at: 1_700_000_000_000,
            },
        ));
        *order += 1;
        let blocks: Vec<_> = page["body"]
            .as_str()
            .unwrap()
            .split('\n')
            .enumerate()
            .map(|(i, text)| paragraph(&format!("{id}-{i}"), vec![Inline::Text(text.to_owned())]))
            .collect();
        let doc = id.doc_id().clone();
        docs.insert(
            doc.clone(),
            editor.edit(&doc, |txn| write_body(txn, &blocks)),
        );
    }
    let workspace = DocId::workspace();
    let update = editor.edit(&workspace, |txn| {
        for (id, record) in &records {
            put_page(txn, id, record);
        }
    });
    docs.insert(workspace, update);
    Docs(docs)
}

fn indexed(dir: &Path) -> Index {
    let index = Index::start(
        Arc::new(fixture_docs()),
        super::index_path(dir),
        Config::DEFAULT,
        Arc::new(|_| {}),
        Arc::default(),
    );
    eventually(Duration::from_secs(120), || {
        (index.status() == IndexStatus::Ready).then_some(())
    });
    index
}

fn expected() -> Vec<(String, Vec<String>)> {
    let expected: serde_json::Value =
        serde_json::from_str(&generated("search-10k-expected.json")).unwrap();
    expected["queries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| {
            (
                q["query"].as_str().unwrap().to_owned(),
                q["expectedIds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| id.as_str().unwrap().to_owned())
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn the_10k_search_fixture_returns_the_expected_page_ids() {
    let dir = tempfile::tempdir().unwrap();
    let index = indexed(dir.path());
    let expected = expected();
    assert_eq!(expected.len(), 4);
    for (query, ids) in expected {
        let hits = ready(index.search(&query, 200).unwrap());
        let mut found: Vec<&str> = hit_ids(&hits);
        found.sort();
        assert_eq!(found, ids, "{query}");
    }
}

#[test]
#[ignore = "a release-build measurement: cargo nextest run -p aelomi-core --cargo-profile release --run-ignored only measure"]
fn measure_content_query_p95_on_the_10k_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let index = indexed(dir.path());
    eprintln!(
        "indexed 10,000 pages from memory in {:?}",
        started.elapsed()
    );
    let report = |name: &str, queries: &[&str], rounds: usize| -> Duration {
        for q in queries {
            ready(index.search(q, 20).unwrap());
        }
        let mut samples: Vec<Duration> = (0..rounds)
            .flat_map(|_| queries.iter())
            .map(|q| {
                let started = Instant::now();
                let hits = ready(index.search(q, 20).unwrap());
                let elapsed = started.elapsed();
                std::hint::black_box(hits);
                elapsed
            })
            .collect();
        samples.sort();
        let at = |p: usize| samples[(samples.len() * p / 100).saturating_sub(1)];
        eprintln!(
            "{name}: {} queries, p50 {:?}, p95 {:?}, max {:?}",
            samples.len(),
            at(50),
            at(95),
            samples[samples.len() - 1]
        );
        at(95)
    };
    let fixture: Vec<String> = expected().into_iter().map(|(q, _)| q).collect();
    let fixture: Vec<&str> = fixture.iter().map(String::as_str).collect();
    let p95 = report("fixture queries", &fixture, 250);
    report(
        "common words, 20 of thousands of hits",
        &["zitida", "rehada", "zi", "dapoku go"],
        50,
    );
    eprintln!("SQLite {}", rusqlite::version());
    assert!(p95 < Duration::from_millis(5), "CMD-5 budget: p95 {p95:?}");
}
