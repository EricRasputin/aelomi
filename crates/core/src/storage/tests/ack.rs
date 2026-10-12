use std::sync::mpsc;
use std::time::Duration;

use super::{Env, assert_doc, doc, soon};
use crate::storage::clock::Clock;
use crate::storage::doc_id::DocId;
use crate::storage::sim_fs::OpKind;
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
        Duration::from_micros(7_500),
        "a new doc's first batch: 3 fsyncs, 1 full sync"
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
    assert_eq!(store.flush(soon()), []);
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
        Duration::from_micros(7_500),
        "a's first batch waits on no read of b's: 3 fsyncs, 1 full sync"
    );
    assert_doc(store, &b, &on_b);
}
