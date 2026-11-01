#![cfg(feature = "test-support")]

use std::time::{Duration, Instant};

use aelomi_core::index::IndexStatus;
use aelomi_core::storage::Store;
use aelomi_core::workspace::fixture::search_fixture;
use aelomi_core::workspace::{PageEdit, Workspace};

#[test]
#[ignore]
fn measure_edit_page_rename_on_10000_pages() {
    assert!(
        !cfg!(debug_assertions),
        "run this measurement in release mode"
    );
    let folder = tempfile::tempdir().unwrap();
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/generated/search-10k-pages.jsonl"
    ))
    .unwrap();
    let store = Store::open(folder.path()).unwrap();
    for (id, bytes) in search_fixture(&fixture, 0) {
        store.append(&id, &bytes).unwrap();
    }
    assert!(
        store
            .close(Instant::now() + Duration::from_secs(120))
            .is_ok()
    );
    let workspace = Workspace::open(folder.path(), |_| {}).unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    while workspace.index_status() != IndexStatus::Ready {
        assert!(Instant::now() < deadline, "index did not become ready");
        std::thread::sleep(Duration::from_millis(10));
    }
    let tree = workspace.tree().unwrap();
    assert_eq!(fixture.lines().count(), 10000);
    assert_eq!(tree.pages.len(), 9700);
    let target = tree.pages[0].id.clone();
    for round in 0..5 {
        let mut accepted = Vec::new();
        let mut durable = Vec::new();
        let mut successes = 0;
        let mut failures = 0;
        for sample in 0..23 {
            let title = format!("D1 title round {round} sample {sample}");
            let start = Instant::now();
            let edited = workspace.edit_page(PageEdit::Rename {
                page: target.clone(),
                title: title.clone(),
            });
            let acceptance = start.elapsed().as_secs_f64() * 1000.0;
            let flushed_ok = workspace
                .flush(Instant::now() + Duration::from_secs(10))
                .is_empty();
            let flushed = start.elapsed().as_secs_f64() * 1000.0;
            let tree = workspace.tree().unwrap();
            let matches = tree
                .pages
                .iter()
                .find(|page| page.id == target)
                .unwrap()
                .title
                == title;
            let succeeded = edited.is_ok() && flushed_ok && matches;
            if sample < 3 {
                assert!(succeeded, "warm-up rename failed");
            } else if succeeded {
                successes += 1;
                accepted.push(acceptance);
                durable.push(flushed);
            } else {
                failures += 1;
            }
        }
        assert_eq!(failures, 0, "rename samples failed");
        assert_eq!(successes, 20);
        accepted.sort_by(f64::total_cmp);
        durable.sort_by(f64::total_cmp);
        println!(
            "rename pages=10000 round={round} successes={successes} failures={failures} accepted_p50_ms={:.3} accepted_p95_ms={:.3} durable_p50_ms={:.3} durable_p95_ms={:.3}",
            accepted[9], accepted[18], durable[9], durable[18]
        );
    }
}
