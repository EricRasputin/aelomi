use std::time::{Duration, Instant};

use super::{assert_doc, doc};
use crate::storage::store::Store;
use crate::storage::testkit::UpdateGen;

#[test]
fn every_append_is_acknowledged_on_a_real_disk() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let a = doc("a");
    let sent = UpdateGen::new(51, 2).updates(200);
    let mut latencies: Vec<Duration> = sent
        .iter()
        .map(|update| {
            let started = Instant::now();
            store.append(&a, update).unwrap();
            assert_eq!(
                store.flush(started + Duration::from_secs(5)),
                [],
                "acknowledged"
            );
            started.elapsed()
        })
        .collect();
    latencies.sort();
    let p95 = latencies[latencies.len() * 95 / 100 - 1];
    eprintln!(
        "real disk: 200 appends acknowledged; p50 {:.2?}, p95 {p95:.2?}, max {:.2?}",
        latencies[latencies.len() / 2 - 1],
        latencies[latencies.len() - 1]
    );
    assert!(
        p95 < Duration::from_secs(1),
        "p95 {p95:?}; CI disks are noisy, so the gate is loose"
    );

    assert_eq!(store.close(), []);
    assert_doc(&Store::open(dir.path()).unwrap(), &a, &sent);
}
