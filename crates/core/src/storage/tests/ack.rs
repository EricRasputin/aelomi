use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{Delivery, Env, applied, assert_doc, doc, ops_of_updates, out_of_order, soon};
use crate::storage::clock::Clock;
use crate::storage::doc_id::DocId;
use crate::storage::sim_fs::{Crash, OpKind};
use crate::storage::state::Limits;
use crate::storage::store::Store;
use crate::storage::testkit::UpdateGen;
use crate::storage::writer::Step;

const SHELL_9_BUDGET: Duration = Duration::from_millis(100);

fn slow_disk() -> Env {
    let env = Env::manual();
    env.fs
        .set_latency(OpKind::FullSync, Duration::from_millis(6));
    env.fs
        .set_latency(OpKind::Fsync, Duration::from_micros(500));
    env
}

struct Due {
    at: Duration,
    doc: DocId,
    update: Vec<u8>,
}

fn acknowledgements(env: &Env, store: &Store, schedule: Vec<Due>) -> (Vec<Duration>, usize) {
    let start = env.clock.now();
    let mut schedule = schedule
        .into_iter()
        .map(|due| Due {
            at: start + due.at,
            ..due
        })
        .peekable();
    let mut waiting = Vec::new();
    let mut latencies = Vec::new();
    let mut compactions = 0;
    loop {
        while let Some(due) = schedule.next_if(|due| due.at <= env.clock.now()) {
            let seq = store.append(&due.doc, &due.update).unwrap();
            waiting.push((due.doc, seq, due.at));
        }
        let wrote = matches!(store.run_writer_once(), Step::Wrote);
        let now = env.clock.now();
        waiting.retain(|(doc, seq, at)| {
            let durable = store.is_durable(doc, *seq);
            if durable {
                latencies.push(now - *at);
            }
            !durable
        });
        if let Some((_, result)) = store.run_compactor_once() {
            result.unwrap();
            compactions += 1;
        } else if !wrote {
            match schedule.peek() {
                Some(due) => env.clock.advance_to(due.at),
                None => break,
            }
        }
    }
    assert_eq!(waiting.len(), 0, "every append was acknowledged");
    (latencies, compactions)
}

fn spaced(docs: &[DocId], n: usize, every: Duration, seed: u64) -> Vec<Due> {
    let mut gens: Vec<UpdateGen> = (0..docs.len() as u64)
        .map(|i| UpdateGen::new(seed + i, 2))
        .collect();
    (0..n)
        .map(|i| Due {
            at: every * i as u32,
            doc: docs[i % docs.len()].clone(),
            update: gens[i % docs.len()].next_update(),
        })
        .collect()
}

fn max(latencies: &[Duration]) -> Duration {
    latencies.iter().copied().max().unwrap()
}

#[test]
fn single_appends_are_acknowledged_within_100_ms() {
    let env = slow_disk();
    let store = env.open();
    let schedule = spaced(&[doc("a")], 20, Duration::from_millis(50), 1);
    let (latencies, _) = acknowledgements(&env, &store, schedule);
    assert_eq!(latencies.len(), 20);
    assert_eq!(
        latencies[0],
        Duration::from_micros(20_500),
        "publishing format 2: 2 fsyncs, 2 full syncs; a new doc's first batch: 3 fsyncs, 1 full sync"
    );
    assert!(
        latencies[1..]
            .iter()
            .all(|l| *l == Duration::from_micros(6_500)),
        "then 1 fsync, 1 full sync"
    );
}

#[test]
fn a_burst_of_1_000_appends_in_10_ms_is_acknowledged_within_100_ms() {
    let env = slow_disk();
    let store = env.open();
    let schedule = spaced(&[doc("a")], 1_000, Duration::from_micros(10), 2);
    let (latencies, _) = acknowledgements(&env, &store, schedule);
    assert_eq!(latencies.len(), 1_000);
    assert!(
        max(&latencies) <= SHELL_9_BUDGET,
        "slowest: {:?}",
        max(&latencies)
    );
}

#[test]
fn appends_across_50_docs_are_acknowledged_within_100_ms() {
    let env = slow_disk();
    let store = env.open();
    let docs: Vec<DocId> = (0..50).map(|i| doc(&format!("page-{i}"))).collect();
    let schedule = spaced(&docs, 1_000, Duration::from_micros(100), 3);
    let (latencies, _) = acknowledgements(&env, &store, schedule);
    assert_eq!(latencies.len(), 1_000);
    assert!(
        max(&latencies) <= SHELL_9_BUDGET,
        "slowest: {:?}",
        max(&latencies)
    );
}

#[test]
fn appends_during_compaction_are_acknowledged_within_100_ms() {
    let mut env = slow_disk();
    env.limits = Limits {
        bytes: 3_000,
        records: 10_000,
        ..Limits::DATA_6
    };
    let store = env.open();
    let docs = [doc("a"), doc("b")];
    let schedule = spaced(&docs, 600, Duration::from_millis(1), 4);
    let (latencies, compactions) = acknowledgements(&env, &store, schedule);
    assert!(
        compactions >= 2,
        "{compactions} compactions ran among the appends"
    );
    assert_eq!(latencies.len(), 600);
    assert!(
        max(&latencies) <= SHELL_9_BUDGET,
        "slowest: {:?}",
        max(&latencies)
    );
}

#[test]
fn a_docs_first_load_from_a_slow_disk_never_delays_another_docs_acknowledgement() {
    let env = slow_disk();
    let (a, b) = (doc("a"), doc("b"));
    let on_b = UpdateGen::new(5, 2).updates(3);
    let store = env.open();
    for update in &on_b {
        store.append(&b, update).unwrap();
    }
    assert_eq!(store.flush(soon()).docs, []);
    drop(store);

    let store = env.open();
    let hold = env
        .fs
        .hold_once(OpKind::Read, "/docs/b/", Duration::from_millis(500));
    let on_a = UpdateGen::new(6, 2).next_update();
    let (store, env) = (&store, &env);
    let (acked, latency) = std::thread::scope(|s| {
        let loading = s.spawn(|| store.load(&b).unwrap());
        hold.wait_until_reached();
        let start = env.clock.now();
        let seq = store.append(&a, &on_a).unwrap();
        let (done, writer_done) = mpsc::channel();
        s.spawn(move || {
            store.run_writer_once();
            done.send(env.clock.now()).unwrap();
        });
        let finished = writer_done.recv_timeout(Duration::from_secs(10));
        hold.release();
        loading.join().unwrap();
        let finished = finished.expect("the writer finishes while b's load is held");
        (store.is_durable(&a, seq), finished - start)
    });
    assert!(acked, "a is acknowledged");
    assert_eq!(
        latency,
        Duration::from_micros(14_500),
        "a's first batch waits on no read of b's: confirming format 2, 2 fsyncs, 1 full sync; \
         the batch, 3 fsyncs, 1 full sync"
    );
    assert_doc(store, &b, &on_b);
}

#[test]
fn a_doc_whose_files_are_busy_never_delays_another_docs_acknowledgement() {
    let env = slow_disk();
    let (a, b) = (doc("a"), doc("b"));
    let on_a = UpdateGen::new(7, 2).next_update();
    let on_b = out_of_order(8, 40, Delivery::OneWithheld);
    let (last, stored) = on_b.split_last().unwrap();
    let store = env.open();
    for update in stored {
        store.append(&b, update).unwrap();
    }
    assert_eq!(store.flush(soon()).docs, []);
    let queued_on_b = store.append(&b, last).unwrap();
    assert!(store.has_pending_structs(&b), "b's load merges its files");

    let hold = env
        .fs
        .hold_once(OpKind::Read, "/docs/b/", Duration::from_millis(500));
    let (store, env) = (&store, &env);
    let (steps, latency, queued_on_a) = std::thread::scope(|s| {
        let loading = s.spawn(|| store.load(&b).unwrap());
        hold.wait_until_reached();
        let start = env.clock.now();
        let seq = store.append(&a, &on_a).unwrap();
        let (done, writer_done) = mpsc::channel();
        s.spawn(move || {
            let first = store.run_writer_once();
            let finished = env.clock.now();
            done.send((first, finished, store.run_writer_once()))
                .unwrap();
        });
        let wrote = writer_done.recv_timeout(Duration::from_secs(5));
        hold.release();
        loading.join().unwrap();
        let (first, finished, second) = wrote
            .or_else(|_| writer_done.recv_timeout(Duration::from_secs(10)))
            .expect("the writer finishes once b's read is released");
        ((first, second), finished - start, seq)
    });
    assert_eq!(
        latency,
        Duration::from_micros(7_500),
        "a's batch skips b's busy files: 3 fsyncs, 1 full sync"
    );
    assert!(matches!(steps.0, Step::Wrote));
    assert!(
        matches!(steps.1, Step::Idle { retry_at: None }),
        "with b's files busy the writer sleeps until woken: {:?}",
        steps.1
    );
    assert!(store.is_durable(&a, queued_on_a));
    assert!(
        !store.is_durable(&b, queued_on_b),
        "b's update stays queued"
    );

    assert!(matches!(store.run_writer_once(), Step::Wrote));
    assert!(
        store.is_durable(&b, queued_on_b),
        "b's freed files go in the next batch"
    );
    env.fs.crash(Crash::DropUnsynced);
    let store = env.open();
    assert_doc(&store, &a, &[on_a]);
    let (held, _) = applied(&store.load(&b).unwrap(), "b");
    assert_eq!(held, ops_of_updates(&on_b));
}

fn within_5_s(done: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    true
}

#[test]
fn the_writer_thread_writes_a_skipped_doc_once_its_files_free() {
    let env = Env::threaded();
    let (a, b) = (doc("a"), doc("b"));
    let on_a = UpdateGen::new(9, 2).next_update();
    let on_b = out_of_order(10, 40, Delivery::OneWithheld);
    let (last, stored) = on_b.split_last().unwrap();
    let store = env.open();
    for update in stored {
        store.append(&b, update).unwrap();
    }
    assert_eq!(store.flush(soon()).docs, []);
    assert!(store.has_pending_structs(&b), "b's load merges its files");

    let hold = env.fs.hold_once(OpKind::Read, "/docs/b/", Duration::ZERO);
    let (a_acked, pending, b_acked) = std::thread::scope(|s| {
        let loading = s.spawn(|| store.load(&b).unwrap());
        hold.wait_until_reached();
        let queued_on_b = store.append(&b, last).unwrap();
        let queued_on_a = store.append(&a, &on_a).unwrap();
        let a_acked = within_5_s(|| store.is_durable(&a, queued_on_a));
        let pending = store
            .flush(Instant::now() + Duration::from_millis(100))
            .docs;
        hold.release();
        loading.join().unwrap();
        let b_acked = within_5_s(|| store.is_durable(&b, queued_on_b));
        (a_acked, pending, b_acked)
    });
    assert!(a_acked, "a is acknowledged while b's files are busy");
    assert_eq!(
        pending,
        std::slice::from_ref(&b),
        "flush reports b at its deadline"
    );
    assert!(
        b_acked,
        "b is written once its files free, with no append or flush"
    );
    store.close(soon()).unwrap();
    env.fs.crash(Crash::DropUnsynced);
    let store = env.open();
    assert_doc(&store, &a, &[on_a]);
    let (held, _) = applied(&store.load(&b).unwrap(), "b");
    assert_eq!(held, ops_of_updates(&on_b));
}
