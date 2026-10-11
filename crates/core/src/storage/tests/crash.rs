use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use yrs::{Doc, Transact};

use super::{Delivery, Env, applied, doc, ops_of_updates, out_of_order, soon};
use crate::storage::doc_id::DocId;
use crate::storage::equality::Ops;
use crate::storage::log::{LogEnd, encode_record, read_log};
use crate::storage::sim_fs::{Crash, OpKind};
use crate::storage::state::{Limits, Seq};
use crate::storage::testkit::{SplitMix64, UpdateGen, apply, doc_from, fingerprint};

const DOCS: usize = 3;

#[derive(Clone, Debug)]
enum Action {
    Append { doc: usize, count: usize },
    Write,
    Compact { doc: usize },
    RunCompactor,
    FailWrites { doc: usize },
    Heal,
    Wait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Arrival {
    InOrder,
    Shuffled,
    OneAfterCrash,
}

#[derive(Clone, Debug)]
struct Scenario {
    seed: u64,
    arrivals: Vec<Arrival>,
    actions: Vec<Action>,
    crash_fraction: u16,
    torn: Option<u64>,
}

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        6 => (0..DOCS, 1..4usize).prop_map(|(doc, count)| Action::Append { doc, count }),
        4 => Just(Action::Write),
        1 => (0..DOCS).prop_map(|doc| Action::Compact { doc }),
        1 => Just(Action::RunCompactor),
        1 => (0..DOCS).prop_map(|doc| Action::FailWrites { doc }),
        1 => Just(Action::Heal),
        1 => Just(Action::Wait),
    ]
}

fn arrival() -> impl Strategy<Value = Arrival> {
    prop_oneof![
        2 => Just(Arrival::InOrder),
        1 => Just(Arrival::Shuffled),
        1 => Just(Arrival::OneAfterCrash),
    ]
}

fn scenario() -> impl Strategy<Value = Scenario> {
    (
        any::<u64>(),
        prop::collection::vec(arrival(), DOCS),
        prop::collection::vec(action(), 1..40),
        any::<u16>(),
        any::<Option<u64>>(),
    )
        .prop_map(|(seed, arrivals, actions, crash_fraction, torn)| Scenario {
            seed,
            arrivals,
            actions,
            crash_fraction,
            torn,
        })
}

struct Sent {
    planned: VecDeque<Vec<u8>>,
    withheld: Option<Vec<u8>>,
    updates: Vec<Vec<u8>>,
    seqs: Vec<Seq>,
    acked: usize,
    refused: Option<Vec<u8>>,
}

fn ids() -> Vec<DocId> {
    (0..DOCS).map(|i| doc(&format!("doc-{i}"))).collect()
}

fn seed_of(scenario: &Scenario, doc: usize) -> u64 {
    scenario.seed.wrapping_add(doc as u64)
}

fn plan(scenario: &Scenario, doc: usize) -> Sent {
    let n = scenario
        .actions
        .iter()
        .map(|action| match *action {
            Action::Append { doc: d, count } if d == doc => count,
            _ => 0,
        })
        .sum();
    let seed = seed_of(scenario, doc);
    let (planned, withheld) = match scenario.arrivals[doc] {
        Arrival::InOrder => (UpdateGen::new(seed, 2).updates(n), None),
        Arrival::Shuffled => (out_of_order(seed, n, Delivery::Shuffled), None),
        Arrival::OneAfterCrash => {
            let mut updates = UpdateGen::new(seed, 3).updates(n + 1);
            let at = SplitMix64::new(seed).below(n as u64 / 2 + 1) as usize;
            let withheld = updates.remove(at);
            (updates, Some(withheld))
        }
    };
    Sent {
        planned: planned.into(),
        withheld,
        updates: Vec::new(),
        seqs: Vec::new(),
        acked: 0,
        refused: None,
    }
}

fn play(scenario: &Scenario, die_after: usize) -> (Env, Vec<Sent>, usize) {
    let mut env = Env::manual();
    env.limits = Limits {
        bytes: 1_500,
        records: 12,
        ..Limits::DATA_6
    };
    let ids = ids();
    let mut sent: Vec<Sent> = (0..DOCS).map(|doc| plan(scenario, doc)).collect();

    let store = env.open();
    let opened = env.fs.ops().len();
    env.fs.die_after(die_after);
    for action in &scenario.actions {
        match *action {
            Action::Append { doc, count } => {
                for _ in 0..count {
                    let s = &mut sent[doc];
                    let update = s.refused.take().or_else(|| s.planned.pop_front());
                    let update = update.expect("one update is planned for each append");
                    match store.append(&ids[doc], &update) {
                        Ok(seq) => {
                            s.updates.push(update);
                            s.seqs.push(seq);
                        }
                        Err(_) => s.refused = Some(update),
                    }
                }
            }
            Action::Write => {
                store.run_writer_once();
            }
            Action::Compact { doc } => {
                let _ = store.compact(&ids[doc]);
            }
            Action::RunCompactor => {
                let _ = store.run_compactor_once();
            }
            Action::FailWrites { doc } => {
                env.fs
                    .fail(OpKind::Append, &format!("/docs/doc-{doc}/"), libc::EIO);
            }
            Action::Heal => env.fs.clear_faults(),
            Action::Wait => env.clock.advance(Duration::from_millis(150)),
        }
        for (i, s) in sent.iter_mut().enumerate() {
            s.acked = s
                .seqs
                .iter()
                .take_while(|seq| store.is_durable(&ids[i], **seq))
                .count();
        }
    }
    drop(store);
    let ops = env.fs.ops().len() - opened;
    (env, sent, ops)
}

fn integrated_ops(doc: &Doc) -> Ops {
    Ops::of_doc(&doc.transact()).expect("yrs decodes its own encoding")
}

fn run(scenario: &Scenario) -> Result<(), TestCaseError> {
    let (_, _, ops) = play(scenario, usize::MAX);
    let crash_after = scenario.crash_fraction as usize * ops / u16::MAX as usize;
    let (env, sent, _) = play(scenario, crash_after);
    let ids = ids();
    env.fs.clear_faults();
    env.fs.crash(
        scenario
            .torn
            .map_or(Crash::DropUnsynced, Crash::KeepTornPrefix),
    );

    let store = env.open();
    let mut after_crash = Vec::new();
    for (i, s) in sent.iter().enumerate() {
        let case = format!(
            "doc {i} ({:?}, {} of {} acked)",
            scenario.arrivals[i],
            s.acked,
            s.updates.len()
        );
        let (held, loaded) = applied(&store.load(&ids[i]).unwrap(), &case);
        prop_assert!(
            held.contains(&ops_of_updates(&s.updates[..s.acked])),
            "{} lost an acknowledged update",
            case
        );
        prop_assert!(
            held.only_from(&ops_of_updates(&s.updates)),
            "{} holds something never appended",
            case
        );
        if scenario.arrivals[i] == Arrival::InOrder {
            let with_acked = doc_from([store.load(&ids[i]).unwrap().as_slice()]);
            for update in &s.updates[..s.acked] {
                apply(&with_acked, update);
            }
            prop_assert_eq!(fingerprint(&with_acked), fingerprint(&loaded), "{}", case);
            let everything = doc_from(s.updates.iter().map(Vec::as_slice));
            let with_loaded = doc_from(s.updates.iter().map(Vec::as_slice));
            apply(&with_loaded, &store.load(&ids[i]).unwrap());
            prop_assert_eq!(
                fingerprint(&with_loaded),
                fingerprint(&everything),
                "{}",
                case
            );
        }
        after_crash.push((integrated_ops(&loaded), loaded));
    }

    let more: Vec<Vec<Vec<u8>>> = (0..DOCS)
        .map(|i| {
            let state = store.load(&ids[i]).unwrap();
            let resumed = UpdateGen::resume(seed_of(scenario, i), 2, &state).updates(3);
            sent[i].withheld.iter().cloned().chain(resumed).collect()
        })
        .collect();
    for (i, updates) in more.iter().enumerate() {
        for update in updates {
            store.append(&ids[i], update).unwrap();
            if scenario.arrivals[i] == Arrival::InOrder {
                apply(&after_crash[i].1, update);
            }
        }
    }
    prop_assert_eq!(
        store.flush(soon()),
        Vec::<DocId>::new(),
        "writes work after the crash"
    );
    drop(store);
    let store = env.open();
    for (i, (ops, expected)) in after_crash.iter().enumerate() {
        let case = format!("doc {i} ({:?}) after reopening", scenario.arrivals[i]);
        let (held, reloaded) = applied(&store.load(&ids[i]).unwrap(), &case);
        prop_assert!(
            integrated_ops(&reloaded).contains(ops),
            "{} lost an update it held after the crash",
            case
        );
        prop_assert!(
            held.contains(&ops_of_updates(&more[i])),
            "{} lost an update appended after the crash",
            case
        );
        let appended = [&sent[i].updates[..], &more[i]].concat();
        prop_assert!(
            held.only_from(&ops_of_updates(&appended)),
            "{} holds something never appended",
            case
        );
        if scenario.arrivals[i] == Arrival::InOrder {
            prop_assert_eq!(fingerprint(&reloaded), fingerprint(expected), "{}", case);
        }
    }
    while let Some((_, result)) = store.run_compactor_once() {
        prop_assert!(result.is_ok(), "compaction after the crash: {:?}", result);
    }
    for (path, bytes) in env.quarantine() {
        prop_assert_eq!(
            read_log(&bytes).end,
            LogEnd::Torn,
            "{:?}: a torn write is never corruption",
            path
        );
    }
    Ok(())
}

#[test]
fn every_acknowledged_update_survives_a_crash_at_any_point() {
    let started = Instant::now();
    let config = Config {
        cases: 1_000,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner = TestRunner::new(config);
    let result = runner.run(&scenario(), |s| run(&s));
    eprintln!("crash points: 1,000 cases in {:.1?}", started.elapsed());
    result.unwrap();
}

fn assert_in_durable_layer(env: &Env, doc: &DocId, update: &[u8], reported_by: &str) {
    let mut record = Vec::new();
    encode_record(update, &mut record);
    let dir = env.path("docs").join(doc.as_str());
    let durable = env.fs.durable_files().into_iter().any(|(path, bytes)| {
        path.parent() == Some(&dir) && bytes.windows(record.len()).any(|w| w == record)
    });
    assert!(
        durable,
        "{reported_by} reported an update durable before a full sync covered it"
    );
}

#[test]
fn the_threaded_writer_reports_an_update_durable_only_after_a_full_sync() {
    let env = Env::threaded();
    env.fs
        .set_latency(OpKind::FullSync, Duration::from_millis(20));
    let a = doc("a");
    let sent = UpdateGen::new(81, 2).updates(40);
    let store = env.open();
    let appended: Mutex<Vec<(Seq, Vec<u8>)>> = Mutex::default();
    let finished = AtomicBool::new(false);
    std::thread::scope(|s| {
        s.spawn(|| {
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut checked = 0;
            loop {
                let next = appended.lock().unwrap().get(checked).cloned();
                match next {
                    Some((seq, update)) if store.is_durable(&a, seq) => {
                        assert_in_durable_layer(&env, &a, &update, "is_durable");
                        checked += 1;
                    }
                    None if finished.load(Ordering::SeqCst) => break,
                    _ => {
                        assert!(Instant::now() < deadline, "{checked} of 40 acknowledged");
                        std::thread::yield_now();
                    }
                }
            }
        });
        for (i, update) in sent.iter().enumerate() {
            let seq = store.append(&a, update).unwrap();
            appended.lock().unwrap().push((seq, update.clone()));
            if i % 4 == 3 {
                assert_eq!(store.flush(soon()), []);
                for update in &sent[..=i] {
                    assert_in_durable_layer(&env, &a, update, "flush");
                }
            }
            std::thread::sleep(Duration::from_millis(3));
        }
        assert_eq!(store.flush(soon()), []);
        finished.store(true, Ordering::SeqCst);
    });
}
