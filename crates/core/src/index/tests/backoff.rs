use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::failures::{after_settled, failed, tamper};
use super::{all, content, eventually, hit_ids, index_path, open_with, page, ready, snapshot};
use crate::index::{
    CacheProblem, CacheState, Config, Corrupt, IndexEvent, IndexStatus, QueryError, Reply,
};
use crate::storage::Store;
use crate::storage::clock::{Clock, ManualClock};
use crate::workspace::fixture::{index_fixture, insert_block, paragraph, text};

const REPAIRED_REPEATEDLY: &str = "the index was repaired repeatedly: corrupt (field)";

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn on_a_manual_clock(dir: &Path) -> (super::Opened, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::default());
    let config = Config {
        clock: Some(clock.clone()),
        ..Config::DEFAULT
    };
    let opened = open_with(
        Store::open(dir).unwrap(),
        index_path(dir),
        config,
        Arc::default(),
    );
    opened.append(&index_fixture());
    opened.settle();
    (opened, clock)
}

/// Damages the cache and has a query find the damage.
fn fault(opened: &super::Opened, dir: &Path) {
    tamper(
        dir,
        "UPDATE pages SET title = x'00' WHERE page_id = 'reading'",
    );
    assert_eq!(
        opened.workspace.search(&content("cafe", 10, &[])),
        Ok(Reply::Indexing)
    );
}

fn waiting(opened: &super::Opened, waits: usize) {
    let index = opened.workspace.index().shared();
    eventually(Duration::from_secs(10), || {
        (index.waits().len() == waits).then_some(())
    });
}

/// Proves the worker waits `wait` on the index clock, then lets it go on.
fn wait_out(opened: &super::Opened, clock: &ManualClock, wait: Duration) {
    let index = opened.workspace.index().shared();
    let almost = clock.now() + wait - ms(1);
    clock.advance_to(almost);
    eventually(Duration::from_secs(10), || {
        (index.kept_waiting_at() == Some(almost)).then_some(())
    });
    assert_eq!(opened.workspace.index_status(), failed(REPAIRED_REPEATEDLY));
    clock.advance(ms(1));
    ready_again(opened);
}

fn ready_again(opened: &super::Opened) {
    eventually(Duration::from_secs(10), || {
        (opened.workspace.index_status() == IndexStatus::Ready).then_some(())
    });
    opened.settle();
}

fn repaired(first: IndexStatus) -> [IndexEvent; 3] {
    [
        IndexEvent::Status(first),
        IndexEvent::Cache(CacheState::Replaced(CacheProblem::Corrupt(Corrupt::Field))),
        IndexEvent::Status(IndexStatus::Ready),
    ]
}

#[test]
fn a_fault_that_recurs_after_every_ready_backs_off_fails_and_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let (opened, clock) = on_a_manual_clock(dir.path());
    let index = opened.workspace.index().shared();
    assert_eq!(
        opened
            .workspace
            .flush(Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
    let expected = snapshot(&opened.workspace);
    let source = super::source_files(dir.path());

    let before = after_settled(&opened).len();
    fault(&opened, dir.path());
    opened.settle();
    assert_eq!(
        after_settled(&opened)[before..],
        repaired(IndexStatus::Rebuilding),
        "the first repair runs at once"
    );
    assert_eq!(index.waits(), []);

    let waits = [
        ms(100),
        ms(200),
        ms(400),
        ms(800),
        ms(1_600),
        ms(3_200),
        ms(6_400),
        ms(12_800),
        ms(25_600),
        ms(30_000),
        ms(30_000),
    ];
    for (n, wait) in waits.into_iter().enumerate() {
        let before = after_settled(&opened).len();
        fault(&opened, dir.path());
        waiting(&opened, n + 1);
        assert_eq!(opened.workspace.index_status(), failed(REPAIRED_REPEATEDLY));
        assert_eq!(
            opened.workspace.search(&all("cafe", 5)),
            Err(QueryError::Failed(REPAIRED_REPEATEDLY.into()))
        );
        assert_eq!(
            opened.workspace.backlinks(&page("reading")),
            Err(QueryError::Failed(REPAIRED_REPEATEDLY.into()))
        );
        wait_out(&opened, &clock, wait);
        assert_eq!(
            after_settled(&opened)[before..],
            repaired(failed(REPAIRED_REPEATEDLY)),
            "repair {n}"
        );
    }
    assert_eq!(index.waits(), waits);
    assert_eq!(snapshot(&opened.workspace), expected);

    clock.advance(ms(30_000) - ms(1));
    fault(&opened, dir.path());
    waiting(&opened, waits.len() + 1);
    assert_eq!(
        index.waits()[waits.len()..],
        [ms(30_000)],
        "Ready for less than 30 s does not clear the fault"
    );
    wait_out(&opened, &clock, ms(30_000));

    clock.advance(ms(30_000));
    let before = after_settled(&opened).len();
    fault(&opened, dir.path());
    opened.settle();
    assert_eq!(
        after_settled(&opened)[before..],
        repaired(IndexStatus::Rebuilding),
        "a fault after 30 s Ready is new and repaired at once"
    );
    assert_eq!(index.waits().len(), waits.len() + 1);
    assert_eq!(snapshot(&opened.workspace), expected);
    assert_eq!(super::source_files(dir.path()), source);
}

#[test]
fn erasing_a_page_after_repeated_repairs_still_replaces_the_cache_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (opened, clock) = on_a_manual_clock(dir.path());
    let index = opened.workspace.index().shared();
    fault(&opened, dir.path());
    opened.settle();
    fault(&opened, dir.path());
    waiting(&opened, 1);
    wait_out(&opened, &clock, ms(100));

    let before = after_settled(&opened).len();
    opened.edit_records(|records| {
        records.remove(&page("daily"));
    });
    opened.settle();
    assert_eq!(
        after_settled(&opened)[before..],
        [
            IndexEvent::Status(IndexStatus::Rebuilding),
            IndexEvent::Cache(CacheState::Replaced(CacheProblem::ErasedPages { count: 1 })),
            IndexEvent::Status(IndexStatus::Ready),
        ]
    );
    assert_eq!(index.waits(), [ms(100)], "the erase waited for nothing");
    assert_eq!(
        ready(
            opened
                .workspace
                .search(&content("quarterly", 10, &[]))
                .unwrap()
        ),
        []
    );

    fault(&opened, dir.path());
    waiting(&opened, 2);
    assert_eq!(
        index.waits(),
        [ms(100), ms(200)],
        "the erase neither spent nor reset the repair backoff"
    );
    assert_eq!(opened.workspace.index_status(), failed(REPAIRED_REPEATEDLY));
    wait_out(&opened, &clock, ms(200));
}

#[test]
fn appends_and_flushes_are_acknowledged_while_a_repeated_repair_waits() {
    let dir = tempfile::tempdir().unwrap();
    let (opened, clock) = on_a_manual_clock(dir.path());
    let index = opened.workspace.index().shared();
    fault(&opened, dir.path());
    opened.settle();
    fault(&opened, dir.path());
    waiting(&opened, 1);

    opened.edit_body("daily", |txn| {
        insert_block(txn, 0, &paragraph("w1", vec![text("waitword")]))
    });
    assert_eq!(
        opened
            .workspace
            .flush(Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
    let almost = clock.now() + ms(99);
    clock.advance_to(almost);
    eventually(Duration::from_secs(10), || {
        (index.kept_waiting_at() == Some(almost)).then_some(())
    });
    assert_eq!(
        opened.workspace.index_status(),
        failed(REPAIRED_REPEATEDLY),
        "a body edit does not cut the wait short"
    );

    opened.edit_records(|records| {
        records.get_mut(&page("roadmap")).unwrap().title = "Roadmap waiting".into();
    });
    assert_eq!(
        opened
            .workspace
            .flush(Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
    let tree = opened.workspace.tree().unwrap();
    assert_eq!(
        tree.pages
            .iter()
            .find(|p| p.id == page("roadmap"))
            .map(|p| p.title.as_str()),
        Some("Roadmap waiting")
    );
    ready_again(&opened);
    assert_eq!(
        clock.now(),
        almost,
        "a workspace edit repaired before the deadline"
    );
    assert_eq!(index.waits(), [ms(100)]);
    assert_eq!(
        hit_ids(&ready(
            opened.workspace.search(&all("waitword", 5)).unwrap()
        )),
        ["daily"]
    );
    assert_eq!(
        hit_ids(&ready(opened.workspace.search(&all("waiting", 5)).unwrap())),
        ["roadmap", "aelomi"]
    );
}

#[test]
fn an_erasure_during_a_repair_wait_repairs_the_cache_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (opened, clock) = on_a_manual_clock(dir.path());
    let index = opened.workspace.index().shared();
    assert_eq!(
        hit_ids(&ready(
            opened
                .workspace
                .search(&content("quarterly", 10, &[]))
                .unwrap()
        )),
        ["daily"]
    );
    fault(&opened, dir.path());
    opened.settle();
    fault(&opened, dir.path());
    waiting(&opened, 1);
    let started = clock.now();
    let before = after_settled(&opened).len();

    opened.edit_records(|records| {
        records.remove(&page("daily"));
    });
    ready_again(&opened);
    assert_eq!(clock.now(), started, "the erasure waited for nothing");
    assert_eq!(
        after_settled(&opened)[before..],
        [
            IndexEvent::Cache(CacheState::Replaced(CacheProblem::Corrupt(Corrupt::Field))),
            IndexEvent::Status(IndexStatus::Ready),
        ]
    );
    assert_eq!(index.waits(), [ms(100)]);
    assert_eq!(
        ready(
            opened
                .workspace
                .search(&content("quarterly", 10, &[]))
                .unwrap()
        ),
        []
    );
    let cache = std::fs::read(index_path(dir.path())).unwrap();
    assert!(
        !cache.windows(b"quarterly".len()).any(|w| w == b"quarterly"),
        "the erased body left the cache file"
    );
}
