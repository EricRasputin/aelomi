//! SHELL-9: while writes succeed, every update is acknowledged within 100 ms of `append`.
//! `SimFs` charges 6 ms per full sync and 0.5 ms per fsync to a manual clock, and the test
//! steps the writer and compactor itself, so every number here is exact and repeatable.

use std::time::Duration;

use super::{Env, doc};
use crate::storage::clock::Clock;
use crate::storage::doc_id::DocId;
use crate::storage::sim_fs::OpKind;
use crate::storage::state::Limits;
use crate::storage::store::Store;
use crate::storage::testkit::UpdateGen;
use crate::storage::writer::Step;

const BUDGET: Duration = Duration::from_millis(100);

fn slow_disk() -> Env {
    let env = Env::manual();
    env.fs
        .set_latency(OpKind::FullSync, Duration::from_millis(6));
    env.fs
        .set_latency(OpKind::Fsync, Duration::from_micros(500));
    env
}

/// An append due at a time after the test starts.
struct Due {
    at: Duration,
    doc: DocId,
    update: Vec<u8>,
}

/// Appends each update once the clock reaches its time, and steps the writer, then the
/// compactor, as a real store's threads would. Returns how long each append took to be
/// acknowledged, and how many compactions ran.
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
        let wrote = matches!(store.run_writer_once(), Step::Wrote(_));
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
    assert!(max(&latencies) <= BUDGET, "slowest: {:?}", max(&latencies));
}

#[test]
fn appends_across_50_docs_are_acknowledged_within_100_ms() {
    let env = slow_disk();
    let store = env.open();
    let docs: Vec<DocId> = (0..50).map(|i| doc(&format!("page-{i}"))).collect();
    let schedule = spaced(&docs, 1_000, Duration::from_micros(100), 3);
    let (latencies, _) = acknowledgements(&env, &store, schedule);
    assert_eq!(latencies.len(), 1_000);
    assert!(max(&latencies) <= BUDGET, "slowest: {:?}", max(&latencies));
}

#[test]
fn appends_during_compaction_are_acknowledged_within_100_ms() {
    let mut env = slow_disk();
    env.limits = Limits {
        bytes: 3_000,
        records: 10_000,
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
    assert!(max(&latencies) <= BUDGET, "slowest: {:?}", max(&latencies));
}
