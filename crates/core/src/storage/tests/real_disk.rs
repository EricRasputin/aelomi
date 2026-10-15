use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{
    assert_doc, doc, frames, log_name, soon, v1_snapshot_name, v2_snapshot_name,
    with_a_loner_sixth, without,
};
use crate::storage::clock::SystemClock;
use crate::storage::fs::OsFs;
use crate::storage::state::Limits;
use crate::storage::store::{Driver, Store};
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
                store.flush(started + Duration::from_secs(5)).docs,
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

    store
        .close(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert_doc(&Store::open(dir.path()).unwrap(), &a, &sent);
}

#[test]
fn compaction_and_quarantine_work_on_a_real_disk() {
    let dir = tempfile::tempdir().unwrap();
    let open = || {
        let limits = Limits {
            bytes: u64::MAX,
            records: 20,
            ..Limits::DATA_6
        };
        let clock = Arc::new(SystemClock::new());
        Store::open_with(dir.path(), Arc::new(OsFs), clock, Driver::Manual, limits).unwrap()
    };
    let files = |dir: &Path| {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    };
    let a = doc("a");
    let docs_a = dir.path().join("docs/a");
    let mut updates = UpdateGen::new(61, 2);
    let compacted = updates.updates(21);
    let store = open();
    for update in &compacted {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()).docs, []);
    let (queued, result) = store.run_compactor_once().expect("over the limit");
    assert_eq!((queued, result.is_ok()), (a.clone(), true));
    assert_eq!(files(&docs_a), [v2_snapshot_name(1)]);
    assert_doc(&store, &a, &compacted);

    let logged = with_a_loner_sixth(updates.updates(10));
    for update in &logged {
        store.append(&a, update).unwrap();
    }
    store.close(soon()).unwrap();
    let name = log_name(1, 0);
    let path = docs_a.join(&name);
    let mut damaged = std::fs::read(&path).unwrap();
    damaged[frames(&logged)[5].body.start + 1] ^= 0x10;
    std::fs::write(&path, &damaged).unwrap();

    let intact = [compacted, without(&logged, &[5])].concat();
    let store = open();
    assert_doc(&store, &a, &intact);
    store.compact(&a).unwrap();
    assert_eq!(files(&docs_a), [v1_snapshot_name(2)]);
    assert_eq!(
        std::fs::read(dir.path().join("quarantine/a").join(&name)).unwrap(),
        damaged,
        "moved byte for byte"
    );
    store.close(soon()).unwrap();
    assert_doc(&open(), &a, &intact);
}
