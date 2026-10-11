use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{Env, assert_doc, doc, soon};
use crate::storage::health::{FailReason, Health};
use crate::storage::log::read_log;
use crate::storage::sim_fs::OpKind;
use crate::storage::store::Store;
use crate::storage::testkit::UpdateGen;
use crate::storage::writer::{Batch, Step};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn wrote(step: Step) -> Batch {
    match step {
        Step::Wrote(batch) => batch,
        Step::Idle { retry_at } => panic!("expected a batch, got Idle until {retry_at:?}"),
    }
}

fn record_health(store: &Store) -> Arc<Mutex<Vec<Health>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    store.subscribe_health(move |health| sink.lock().unwrap().push(health.clone()));
    seen
}

#[test]
fn a_failing_doc_stays_pending_while_other_docs_are_acknowledged() {
    let env = Env::manual();
    let store = env.open();
    let seen = record_health(&store);
    let (a, b) = (doc("a"), doc("b"));
    let (mut gen_a, mut gen_b) = (UpdateGen::new(1, 2), UpdateGen::new(2, 2));
    let mut sent_a = gen_a.updates(3);
    let mut sent_b = Vec::new();
    env.fs.fail(OpKind::Append, "/docs/a/", libc::ENOSPC);
    env.clock.advance(ms(1000));

    let a_last = sent_a
        .iter()
        .map(|u| store.append(&a, u).unwrap())
        .last()
        .unwrap();
    sent_b.push(gen_b.next_update());
    let b_first = store.append(&b, &sent_b[0]).unwrap();
    let batch = wrote(store.run_writer_once());
    assert_eq!(batch.durable, [(b.clone(), b_first)], "b is acknowledged");
    assert_eq!(batch.failed, [(a.clone(), FailReason::DiskFull)], "a fails");
    assert!(!store.is_durable(&a, a_last), "a stays pending");
    let failing = Health::Failing {
        reason: FailReason::DiskFull,
        since: ms(1000),
        pending_docs: vec![a.clone()],
    };
    assert_eq!(store.health(), failing);

    sent_b.push(gen_b.next_update());
    let b_second = store.append(&b, &sent_b[1]).unwrap();
    let batch = wrote(store.run_writer_once());
    assert_eq!(
        batch.durable,
        [(b.clone(), b_second)],
        "b keeps going while a waits"
    );
    assert_eq!(batch.failed, []);

    let mut retries = Vec::new();
    for _ in 0..4 {
        let Step::Idle { retry_at: Some(at) } = store.run_writer_once() else {
            panic!("a waits for its retry")
        };
        retries.push(at);
        env.clock.advance_to(at);
        let batch = wrote(store.run_writer_once());
        assert_eq!(batch.failed, [(a.clone(), FailReason::DiskFull)]);
    }
    assert_eq!(
        retries,
        [ms(1100), ms(1300), ms(1700), ms(2500)],
        "100 ms, doubling"
    );
    assert_eq!(
        store.health(),
        failing,
        "still failing, since the first failure"
    );

    sent_a.push(gen_a.next_update());
    let a_more = store.append(&a, sent_a.last().unwrap()).unwrap();
    env.fs.clear_faults();
    let Step::Idle { retry_at: Some(at) } = store.run_writer_once() else {
        panic!("a waits for its retry")
    };
    assert_eq!(at, ms(4100));
    env.clock.advance_to(at);
    let batch = wrote(store.run_writer_once());
    assert_eq!(
        batch.durable,
        [(a.clone(), a_more)],
        "a recovers with everything queued"
    );
    assert_eq!(store.health(), Health::Healthy);
    assert_eq!(*seen.lock().unwrap(), [failing, Health::Healthy]);

    assert_eq!(
        env.doc_files(&a),
        (0..=5)
            .map(|n| format!("log-0000000000-{n:010}.ylog"))
            .collect::<Vec<_>>(),
        "each attempt wrote a new segment"
    );
    drop(store);
    let store = env.open();
    assert_doc(&store, &a, &sent_a);
    assert_doc(&store, &b, &sent_b);
}

#[test]
fn queued_updates_reach_disk_in_order_once_writes_recover() {
    let env = Env::manual();
    let store = env.open();
    let seen = record_health(&store);
    let a = doc("a");
    let sent = UpdateGen::new(5, 3).updates(100);
    env.fs.fail(OpKind::Append, "/docs/a/", libc::EIO);

    for update in &sent {
        store.append(&a, update).unwrap();
    }
    let batch = wrote(store.run_writer_once());
    assert_eq!(batch.failed.len(), 1);
    env.fs.clear_faults();
    env.clock.advance(ms(100));
    let batch = wrote(store.run_writer_once());
    assert_eq!(batch.failed, []);
    assert_eq!(store.health(), Health::Healthy);
    let health = seen.lock().unwrap().clone();
    assert!(matches!(
        &health[..],
        [
            Health::Failing {
                reason: FailReason::Io(_),
                ..
            },
            Health::Healthy
        ]
    ));

    let segments: Vec<Vec<Vec<u8>>> = env
        .doc_files(&a)
        .iter()
        .map(|name| {
            let bytes = &env.fs.files()[&env.path("docs/a").join(name)];
            read_log(bytes).records.iter().map(|r| r.to_vec()).collect()
        })
        .collect();
    assert_eq!(segments.len(), 2, "the failed segment, then the retry's");
    assert!(
        sent.starts_with(&segments[0]),
        "the failed write left a prefix at most"
    );
    assert!(segments[0].len() < sent.len());
    assert_eq!(segments[1], sent, "the retry wrote every update, in order");
}

#[test]
fn a_failed_full_sync_leaves_the_whole_batch_pending() {
    let env = Env::manual();
    let store = env.open();
    let docs = [doc("a"), doc("b")];
    let mut updates = UpdateGen::new(9, 2);
    let mut sent = vec![Vec::new(), Vec::new()];
    for (i, d) in docs.iter().enumerate() {
        sent[i].push(updates.next_update());
        store.append(d, &sent[i][0]).unwrap();
    }
    env.fs.fail(OpKind::FullSync, "/docs/", libc::EIO);
    let batch = wrote(store.run_writer_once());
    assert_eq!(batch.durable, []);
    assert_eq!(
        batch
            .failed
            .iter()
            .map(|(d, _)| d.clone())
            .collect::<Vec<_>>(),
        docs
    );
    let Health::Failing { pending_docs, .. } = store.health() else {
        panic!("failing")
    };
    assert_eq!(pending_docs, docs);

    env.fs.clear_faults();
    env.clock.advance(ms(100));
    let batch = wrote(store.run_writer_once());
    assert_eq!(batch.durable.len(), 2);
    assert_eq!(store.health(), Health::Healthy);
    drop(store);
    let store = env.open();
    for (d, sent) in docs.iter().zip(&sent) {
        assert_doc(&store, d, sent);
    }
}

#[test]
fn the_writer_thread_retries_on_its_own() {
    let env = Env::threaded();
    let store = env.open();
    let a = doc("a");
    let sent = UpdateGen::new(4, 2).updates(5);
    env.fs.fail(OpKind::Append, "/docs/a/", libc::EROFS);
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while store.health() == Health::Healthy {
        assert!(Instant::now() < deadline, "health turns Failing");
        std::thread::sleep(ms(1));
    }
    let Health::Failing { reason, .. } = store.health() else {
        unreachable!()
    };
    assert_eq!(reason, FailReason::ReadOnlyVolume);
    env.fs.clear_faults();
    let started = Instant::now();
    assert_eq!(store.flush(soon()), [], "the retry lands without new work");
    assert!(
        started.elapsed() < Duration::from_secs(6),
        "retries are at most 5 s apart"
    );
    assert_eq!(store.health(), Health::Healthy);
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}
