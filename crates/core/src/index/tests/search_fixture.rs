use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::search_oracle::{self, Hit, Page, generated};
use super::{Docs, all, content, eventually, hit_ids, ready};
use crate::index::query::{self, Plan, Scope, Walk};
use crate::index::{Config, Index, IndexStatus, SearchHit, SearchQuery};
use crate::storage::DocId;
use crate::workspace::fixture::{
    Editor, SEARCH_CREATED_AT, blocknote_blocks, put_page, search_fixture, write_body,
};
use crate::workspace::records::{PageId, PageRecord, Placement};

pub(crate) const BIG_PAGE: &str = "big-page";
const BIG_TITLE: &str = "Kestrel field notes";

fn big_created_at() -> u64 {
    SEARCH_CREATED_AT + 10_000 * 1000
}

pub(crate) fn fixture_docs(big: bool) -> Docs {
    let mut docs: BTreeMap<DocId, Vec<u8>> = BTreeMap::new();
    for (doc, update) in search_fixture(&generated("search-10k-pages.jsonl"), 0) {
        docs.insert(doc, update);
    }
    if big {
        let mut editor = Editor::new(12);
        editor.apply(&DocId::workspace(), &docs[&DocId::workspace()]);
        let page = PageId::new(BIG_PAGE).unwrap();
        let record = PageRecord {
            title: BIG_TITLE.into(),
            placement: Placement {
                parent: None,
                order: 10_000,
            },
            favorite: None,
            trash: None,
            created_at: big_created_at(),
            updated_at: big_created_at(),
        };
        editor.edit(&DocId::workspace(), |txn| put_page(txn, &page, &record));
        docs.insert(DocId::workspace(), editor.state(&DocId::workspace()));
        let blocks =
            blocknote_blocks(&serde_json::from_str(&generated("page-5000-blocks.json")).unwrap());
        docs.insert(
            page.doc_id().clone(),
            editor.edit(page.doc_id(), |txn| write_body(txn, &blocks)),
        );
    }
    Docs(docs)
}

pub(crate) fn oracle_pages(big: bool) -> Vec<Page> {
    let mut pages = search_oracle::search_pages(SEARCH_CREATED_AT);
    if big {
        pages.push(search_oracle::big_page(
            BIG_PAGE,
            BIG_TITLE,
            big_created_at(),
        ));
    }
    pages
}

fn indexed(dir: &Path, big: bool) -> Index {
    let index = Index::start(
        Arc::new(fixture_docs(big)),
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

fn as_hits(hits: &[SearchHit]) -> Vec<Hit> {
    let parts = |parts: &[crate::index::TextPart]| {
        parts
            .iter()
            .map(|p| (p.text.clone(), p.highlighted))
            .collect()
    };
    hits.iter()
        .map(|hit| {
            (
                hit.page_id.as_str().to_owned(),
                parts(&hit.title),
                hit.block
                    .as_ref()
                    .map(|b| (b.block_id.as_str().to_owned(), parts(&b.text))),
            )
        })
        .collect()
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

pub(crate) const QUERIES: [&str; 16] = [
    "zorvak",
    "quillmere",
    "kestrel",
    "nonesuchword",
    "zitida",
    "rehada",
    "z",
    "zi",
    "zit",
    "ziti",
    "zitid",
    "dapoku go",
    "zitida kestrel",
    "Dapoku, GO!",
    "je",
    "s",
];

#[test]
fn the_10k_search_fixture_returns_the_expected_page_ids() {
    let dir = tempfile::tempdir().unwrap();
    let index = indexed(dir.path(), false);
    let expected = expected();
    assert_eq!(expected.len(), 4);
    for (query, ids) in expected {
        let hits = ready(index.search(&content(&query, 200, &[])).unwrap());
        let mut found: Vec<&str> = hit_ids(&hits);
        found.sort();
        assert_eq!(found, ids, "{query}");
    }
}

pub(crate) fn title_group(pages: &[Page], index: &Index, query: &str) -> Vec<PageId> {
    let mut ids: Vec<PageId> = ready(index.search(&all(query, 200)).unwrap())
        .into_iter()
        .map(|hit| hit.page_id)
        .collect();
    let mut order: Vec<&Page> = pages.iter().filter(|p| p.active).collect();
    order.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
    for page in order.iter().step_by(7) {
        if ids.len() == 200 {
            break;
        }
        let id = PageId::new(page.id.as_str()).unwrap();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids.truncate(200);
    ids
}

#[test]
fn every_query_scope_limit_and_exclusion_equals_the_independent_oracle() {
    let dir = tempfile::tempdir().unwrap();
    let index = indexed(dir.path(), true);
    let pages = oracle_pages(true);
    let mut compared = 0;
    for query in QUERIES {
        let group = title_group(&pages, &index, query);
        let group_ids: HashSet<String> = group.iter().map(|id| id.as_str().to_owned()).collect();
        let none = HashSet::new();
        for limit in [20, 200] {
            let cases: [(SearchQuery, search_oracle::Scope); 3] = [
                (all(query, limit), search_oracle::Scope::All),
                (
                    content(query, limit, &[]),
                    search_oracle::Scope::Content(&none),
                ),
                (
                    content(query, limit, &group),
                    search_oracle::Scope::Content(&group_ids),
                ),
            ];
            for (search, scope) in cases {
                let (want, _) = search_oracle::expected(&pages, query, limit, scope);
                let got = as_hits(&ready(index.search(&search).unwrap()));
                assert_eq!(got, want, "{query:?} limit {limit} {:?}", search.scope);
                compared += got.len();
            }
        }
    }
    assert!(compared > 2_000, "{compared} hits compared");
}

#[test]
fn a_walk_reads_only_the_matches_of_the_pages_it_returns_or_skips() {
    let dir = tempfile::tempdir().unwrap();
    let index = indexed(dir.path(), true);
    let pages = oracle_pages(true);
    let mut conn = super::super::db::open_reader(&super::index_path(dir.path())).unwrap();
    for query in ["zitida", "z", "je", "dapoku go"] {
        let group = title_group(&pages, &index, query);
        let refs: Vec<&PageId> = group.iter().collect();
        let group_ids: HashSet<String> = group.iter().map(|id| id.as_str().to_owned()).collect();
        let words = crate::index::highlight::words(query);
        let expression = crate::index::highlight::match_expression(&words).unwrap();
        for (limit, exclude) in [(20, false), (200, false), (20, true)] {
            let plan = Plan {
                expression: &expression,
                words: &words,
                limit,
                scope: Scope::Content {
                    exclude: if exclude { &refs } else { &[] },
                },
            };
            let (_, walk) = query::search(&mut conn, &plan).unwrap();
            let none = HashSet::new();
            let scope = match exclude {
                true => search_oracle::Scope::Content(&group_ids),
                false => search_oracle::Scope::Content(&none),
            };
            let (_, rows) = search_oracle::expected(&pages, query, limit, scope);
            assert_eq!(
                walk,
                Walk {
                    title_rows: 0,
                    block_rows: rows
                },
                "{query} limit {limit} exclude {exclude}"
            );
        }
    }
}

#[test]
fn the_walks_read_fts_rowids_in_order_without_sorting() {
    let dir = tempfile::tempdir().unwrap();
    let _index = indexed(dir.path(), false);
    let conn = super::super::db::open_reader(&super::index_path(dir.path())).unwrap();
    for sql in [
        "SELECT rowid FROM title_fts WHERE title_fts MATCH ?1 ORDER BY rowid DESC",
        "SELECT f.rowid, b.ordinal FROM block_fts f LEFT JOIN blocks b ON b.rowid = f.rowid
         WHERE block_fts MATCH ?1 ORDER BY f.rowid DESC",
    ] {
        let plan: Vec<String> = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map(["\"zitida\"*"], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(
            plan.iter().all(|step| !step.contains("TEMP B-TREE")),
            "{plan:?}"
        );
    }
}

const GOLDEN_FULL: [&str; 8] = [
    "zorvak",
    "quillmere",
    "kestrel",
    "zitida",
    "z",
    "zitid",
    "dapoku go",
    "je",
];
const GOLDEN_IDS: [&str; 2] = ["zitida", "z"];

fn golden_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/index/search-10k-golden.json")
}

fn golden_of(search: impl Fn(&str, usize, bool) -> Vec<Hit>) -> serde_json::Value {
    let mut cases = Vec::new();
    for query in GOLDEN_FULL {
        for scope in ["all", "content"] {
            cases.push(serde_json::json!({
                "query": query,
                "scope": scope,
                "limit": 20,
                "hits": search(query, 20, scope == "all"),
            }));
        }
    }
    for query in GOLDEN_IDS {
        for scope in ["all", "content"] {
            let ids: Vec<(String, Option<String>)> = search(query, 200, scope == "all")
                .into_iter()
                .map(|(page, _, block)| (page, block.map(|b| b.0)))
                .collect();
            cases.push(serde_json::json!({
                "query": query,
                "scope": scope,
                "limit": 200,
                "ids": ids,
            }));
        }
    }
    serde_json::Value::Array(cases)
}

#[test]
fn the_ordered_results_and_highlights_equal_the_reviewed_golden() {
    let pages = oracle_pages(true);
    let none = HashSet::new();
    let from_oracle = golden_of(|query, limit, all| {
        let scope = if all {
            search_oracle::Scope::All
        } else {
            search_oracle::Scope::Content(&none)
        };
        search_oracle::expected(&pages, query, limit, scope).0
    });
    if std::env::var_os("AELOMI_WRITE_SEARCH_GOLDEN").is_some_and(|v| v == "1") {
        let lines: Vec<String> = from_oracle
            .as_array()
            .unwrap()
            .iter()
            .map(|case| serde_json::to_string(case).unwrap())
            .collect();
        std::fs::write(golden_path(), format!("[\n{}\n]\n", lines.join(",\n"))).unwrap();
    }
    let golden: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(golden_path()).unwrap()).unwrap();
    assert_eq!(from_oracle, golden, "the oracle drifted from the golden");

    let dir = tempfile::tempdir().unwrap();
    let index = indexed(dir.path(), true);
    let from_index = golden_of(|query, limit, all_scope| {
        let search = if all_scope {
            all(query, limit)
        } else {
            content(query, limit, &[])
        };
        as_hits(&ready(index.search(&search).unwrap()))
    });
    assert_eq!(from_index, golden);
}

#[test]
fn match_cardinalities_equal_the_oracle_and_the_measured_figures() {
    let dir = tempfile::tempdir().unwrap();
    let _index = indexed(dir.path(), false);
    let pages = oracle_pages(false);
    let conn = super::super::db::open_reader(&super::index_path(dir.path())).unwrap();
    let mut measured = Vec::new();
    for query in QUERIES {
        let words = crate::index::highlight::words(query);
        let expression = crate::index::highlight::match_expression(&words).unwrap();
        let count = |sql: &str| -> usize {
            conn.query_row(sql, [&expression], |row| row.get::<_, i64>(0))
                .unwrap() as usize
        };
        let blocks = count("SELECT count(*) FROM block_fts WHERE block_fts MATCH ?1");
        let pages_matched =
            count("SELECT count(DISTINCT rowid >> 20) FROM block_fts WHERE block_fts MATCH ?1");
        let none = HashSet::new();
        let (all_pages, _) = search_oracle::expected(
            &pages,
            query,
            usize::MAX,
            search_oracle::Scope::Content(&none),
        );
        assert_eq!(pages_matched, all_pages.len(), "{query}");
        measured.push((query, blocks, pages_matched));
    }
    let find = |q: &str| measured.iter().find(|(query, ..)| *query == q).unwrap();
    assert_eq!(find("zitida"), &("zitida", 21_553, 9_412));
    assert_eq!(find("z"), &("z", 25_765, 9_660));
    assert_eq!(find("dapoku go"), &("dapoku go", 3_509, 3_097));
    assert_eq!(find("kestrel"), &("kestrel", 4, 4));
}

#[test]
fn the_search_oracle_uses_no_crate_code() {
    let oracle = include_str!("search_oracle.rs");
    assert!(oracle.contains("pub(crate) fn expected("));
    for path in ["crate::", "super::", "aelomi_core::"] {
        assert!(!oracle.contains(path), "the search oracle uses {path}");
    }
}
