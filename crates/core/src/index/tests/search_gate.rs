use std::sync::Arc;
use std::time::{Duration, Instant};

use super::search_fixture::{QUERIES, fixture_docs, oracle_pages, title_group};
use super::{Opened, all, content, index_path, open_with, ready};
use crate::index::query::{self, Plan, Scope};
use crate::index::{Config, SearchHit, SearchQuery};
use crate::storage::{Driver, Limits, SimFs, Store, SystemClock};
use crate::workspace::records::PageId;

const BUDGET_MS: f64 = 5.0;
const FIRST_SAMPLES: usize = 30;
const STEADY_SAMPLES: usize = 200;

fn setting(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn load() -> String {
    let mut loads = [0f64; 3];
    let n = unsafe { libc::getloadavg(loads.as_mut_ptr(), 3) };
    if n != 3 {
        return "unknown".into();
    }
    format!("{:.2} {:.2} {:.2}", loads[0], loads[1], loads[2])
}

fn indexed(big: bool) -> (Opened, tempfile::TempDir, Duration) {
    let dir = tempfile::tempdir().unwrap();
    let fs = SimFs::new(Arc::new(SystemClock::new()));
    let store = Store::open_with(
        std::path::Path::new("/data"),
        fs,
        Arc::new(SystemClock::new()),
        Driver::Threads,
        Limits::DATA_6,
    )
    .unwrap();
    let opened = open_with(
        store,
        index_path(dir.path()),
        Config::DEFAULT,
        Arc::default(),
    );
    let started = Instant::now();
    for (doc, update) in fixture_docs(big).0 {
        opened.workspace.append(&doc, &update).unwrap();
    }
    opened.settle();
    (opened, dir, started.elapsed())
}

fn consume(hits: &[SearchHit]) -> usize {
    hits.iter()
        .map(|hit| {
            hit.page_id.as_str().len()
                + hit.title.iter().map(|p| p.text.len()).sum::<usize>()
                + hit.block.as_ref().map_or(0, |b| {
                    b.block_id.as_str().len() + b.text.iter().map(|p| p.text.len()).sum::<usize>()
                })
        })
        .sum()
}

fn percentile(samples: &mut [Duration], p: usize) -> Duration {
    samples.sort();
    let rank = (samples.len() * p).div_ceil(100).max(1);
    samples[rank - 1]
}

struct Cardinality {
    blocks: i64,
    pages: i64,
    titles: i64,
}

fn cardinality(conn: &rusqlite::Connection, expression: &str) -> Cardinality {
    let count = |sql: &str| -> i64 { conn.query_row(sql, [expression], |row| row.get(0)).unwrap() };
    Cardinality {
        blocks: count("SELECT count(*) FROM block_fts WHERE block_fts MATCH ?1"),
        pages: count("SELECT count(DISTINCT rowid >> 20) FROM block_fts WHERE block_fts MATCH ?1"),
        titles: count("SELECT count(*) FROM title_fts WHERE title_fts MATCH ?1"),
    }
}

#[test]
#[ignore = "a release-build gate: cargo nextest run -p aelomi-core --cargo-profile release --run-ignored only --test-threads 1 index::tests::search_gate"]
fn search_answers_within_5_ms_p95_through_the_public_workspace() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let budget =
        Duration::from_secs_f64(setting("AELOMI_SEARCH_GATE_BUDGET_MS", BUDGET_MS) / 1000.0);
    let first_samples = (setting("AELOMI_SEARCH_GATE_FIRST_SAMPLES", FIRST_SAMPLES as f64)
        as usize)
        .max(FIRST_SAMPLES);
    let cores = std::thread::available_parallelism().map_or(0, |n| n.get());
    eprintln!(
        "gate start profile={profile} sqlite={} cores={cores} budget_ms={} \
         first_samples={first_samples} steady_samples={STEADY_SAMPLES} load={}",
        rusqlite::version(),
        budget.as_secs_f64() * 1000.0,
        load()
    );
    let mut worst = (Duration::ZERO, String::new());
    let mut failures = Vec::new();
    let mut buckets = 0;
    for big in [false, true] {
        let fixture = if big { "10k+5000" } else { "10k" };
        let (opened, dir, seeded) = indexed(big);
        let pages = oracle_pages(big);
        eprintln!(
            "gate fixture={fixture} indexed_ms={:.0} load={}",
            seeded.as_secs_f64() * 1000.0,
            load()
        );
        let index = opened.workspace.index();
        let mut walker = super::super::db::open_reader(&index_path(dir.path())).unwrap();
        for text in QUERIES {
            let group: Vec<PageId> = title_group(&pages, index, text);
            let group_refs: Vec<&PageId> = group.iter().collect();
            let words = crate::index::highlight::words(text);
            let expression = crate::index::highlight::match_expression(&words).unwrap();
            let count = cardinality(&walker, &expression);
            for limit in [20, 200] {
                let cases: [(&str, SearchQuery, Scope); 3] = [
                    ("all", all(text, limit), Scope::All),
                    (
                        "content-0",
                        content(text, limit, &[]),
                        Scope::Content { exclude: &[] },
                    ),
                    (
                        "content-200",
                        content(text, limit, &group),
                        Scope::Content {
                            exclude: &group_refs,
                        },
                    ),
                ];
                for (scope_name, search, scope) in cases {
                    let plan = Plan {
                        expression: &expression,
                        words: &words,
                        limit,
                        scope,
                    };
                    let (expected, walk) = query::search(&mut walker, &plan).unwrap();
                    let mut first = Vec::with_capacity(first_samples);
                    for _ in 0..first_samples {
                        index.shared().close_reader();
                        let started = Instant::now();
                        let hits = ready(opened.workspace.search(&search).unwrap());
                        std::hint::black_box(consume(&hits));
                        first.push(started.elapsed());
                        assert_eq!(hits, expected, "{fixture} {text:?} {scope_name} {limit}");
                    }
                    let mut steady = Vec::with_capacity(STEADY_SAMPLES);
                    let mut consumed = 0;
                    for _ in 0..STEADY_SAMPLES {
                        let started = Instant::now();
                        let hits = ready(opened.workspace.search(&search).unwrap());
                        consumed += std::hint::black_box(consume(&hits));
                        steady.push(started.elapsed());
                    }
                    let first_max = *first.iter().max().unwrap();
                    let steady_max = *steady.iter().max().unwrap();
                    let (first_p50, first_p95) =
                        (percentile(&mut first, 50), percentile(&mut first, 95));
                    let (steady_p50, steady_p95) =
                        (percentile(&mut steady, 50), percentile(&mut steady, 95));
                    let label = format!("{fixture} {text:?} {scope_name} limit={limit}");
                    eprintln!(
                        "gate fixture={fixture} query={text:?} scope={scope_name} limit={limit} \
                         hits={} matched_blocks={} matched_pages={} matched_titles={} \
                         walked_title_rows={} walked_block_rows={} consumed_bytes={consumed} \
                         first_p50_us={} first_p95_us={} first_max_us={} \
                         steady_p50_us={} steady_p95_us={} steady_max_us={}",
                        expected.len(),
                        count.blocks,
                        count.pages,
                        count.titles,
                        walk.title_rows,
                        walk.block_rows,
                        first_p50.as_micros(),
                        first_p95.as_micros(),
                        first_max.as_micros(),
                        steady_p50.as_micros(),
                        steady_p95.as_micros(),
                        steady_max.as_micros(),
                    );
                    for (kind, p95) in [("first", first_p95), ("steady", steady_p95)] {
                        if p95 > worst.0 {
                            worst = (p95, format!("{label} {kind}"));
                        }
                        if p95 >= budget {
                            failures.push(format!("{label} {kind} p95 {p95:?}"));
                        }
                    }
                    buckets += 1;
                }
            }
        }
    }
    eprintln!(
        "gate end profile={profile} buckets={buckets} worst_p95_us={} worst={} load={}",
        worst.0.as_micros(),
        worst.1,
        load()
    );
    assert!(
        failures.is_empty(),
        "CMD-5 budget {budget:?} p95: {failures:#?}"
    );
}
