use std::sync::Arc;
use std::time::{Duration, Instant};

use yrs::updates::decoder::Decode;
use yrs::{Doc, Text, Transact, Update};

use super::{Delivery, Env, doc, in_order, ops_of_updates, out_of_order, soon};
use crate::storage::clock::SystemClock;
use crate::storage::equality::Ops;
use crate::storage::fs::OsFs;
use crate::storage::recover::merge;
use crate::storage::snapshot::read_snapshot;
use crate::storage::state::Limits;
use crate::storage::store::{Driver, Store};
use crate::storage::testkit::{SplitMix64, fingerprint};

#[derive(Clone, Copy, Debug)]
enum Case {
    Shuffled,
    OneWithheld,
    CompactedHalfway,
}

#[derive(PartialEq)]
enum Outcome {
    NeverPending,
    Accepted,
    Refused,
}

fn compact_and_reopen(seed: u64, n: usize, case: Case) -> Outcome {
    let mut sent = out_of_order(seed, n, Delivery::Shuffled);
    if let Case::OneWithheld = case {
        sent.remove(SplitMix64::new(seed).below(n as u64) as usize);
    }
    let halves: Vec<&[Vec<u8>]> = match case {
        Case::CompactedHalfway => vec![&sent[..n / 2], &sent[n / 2..]],
        Case::Shuffled | Case::OneWithheld => vec![&sent],
    };
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let (mut had_pending, mut merged, mut compacted) = (false, false, true);
    for half in &halves {
        for update in *half {
            store.append(&a, update).unwrap();
        }
        merged = store.has_pending_structs(&a);
        had_pending |= merged;
        assert_eq!(store.flush(soon()), []);
        compacted &= store.compact(&a).is_ok();
    }
    if !had_pending {
        return Outcome::NeverPending;
    }
    let snapshot = format!("snapshot-{:010}.ysnap", halves.len());
    if !compacted || env.doc_files(&a) != [snapshot.clone()] {
        return Outcome::Refused;
    }
    let stored = &env.fs.files()[&env.path("docs/a").join(&snapshot)];
    let held = Ops::of_update(&Update::decode_v1(read_snapshot(stored).unwrap()).unwrap());
    let union = ops_of_updates(&sent);
    // yrs's encoding of a doc whose pending structs all integrated also holds the deletions yrs
    // derived from them.
    let holds = if merged {
        held == union
    } else {
        held.contains(&union)
    };
    drop(store);
    let loaded = Doc::new();
    let update = Update::decode_v1(&env.open().load(&a).unwrap()).unwrap();
    let applies = loaded.transact_mut().apply_update(update).is_ok();
    let reopened = match case {
        Case::OneWithheld => {
            Ops::of_doc(&loaded.transact()).is_some_and(|ops| ops.contains(&union))
        }
        Case::Shuffled | Case::CompactedHalfway => {
            fingerprint(&loaded) == fingerprint(&in_order(seed, n))
        }
    };
    if holds && applies && reopened {
        Outcome::Accepted
    } else {
        Outcome::Refused
    }
}

#[test]
#[ignore = "a measurement"]
fn measure_compaction_of_docs_with_pending_structs() {
    let mut refused = 0;
    for case in [Case::Shuffled, Case::OneWithheld, Case::CompactedHalfway] {
        for n in [20, 40, 80] {
            let outcomes: Vec<Outcome> = (0..400)
                .map(|seed| compact_and_reopen(seed, n, case))
                .filter(|outcome| *outcome != Outcome::NeverPending)
                .collect();
            let accepted = outcomes.iter().filter(|o| **o == Outcome::Accepted).count();
            refused += outcomes.len() - accepted;
            eprintln!(
                "{case:?}, {n} updates, 400 seeds: accepted {accepted} of {} docs with pending structs",
                outcomes.len()
            );
        }
    }
    assert_eq!(refused, 0);
}

fn each_needing_the_one_before(n: usize) -> Vec<Vec<u8>> {
    let client = Doc::with_client_id(1);
    let text = client.get_or_insert_text("text");
    (0..n)
        .map(|i| {
            let mut txn = client.transact_mut();
            text.insert(&mut txn, 0, &format!("{i:090}"));
            txn.encode_update_v1()
        })
        .collect()
}

fn median_load(dir: &std::path::Path, updates: &[Vec<u8>], pending: bool) -> (Duration, usize) {
    let limits = Limits {
        bytes: u64::MAX,
        records: u64::MAX,
        ..Limits::DATA_6
    };
    let open = || {
        let clock = Arc::new(SystemClock::new());
        Store::open_with(dir, Arc::new(OsFs), clock, Driver::Manual, limits).unwrap()
    };
    let a = doc("a");
    let store = open();
    for update in updates {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    drop(store);
    let store = open();
    assert_eq!(store.has_pending_structs(&a), pending);
    let bytes = store.load(&a).unwrap().len();
    let mut times: Vec<Duration> = (0..9)
        .map(|_| {
            let started = Instant::now();
            store.load(&a).unwrap();
            started.elapsed()
        })
        .collect();
    times.sort();
    (times[4], bytes)
}

#[test]
#[ignore = "a measurement"]
fn measure_load_of_a_10_000_record_doc() {
    let updates = each_needing_the_one_before(10_001);
    let (complete, pending) = (&updates[..10_000], &updates[1..]);
    let size: usize = complete.iter().map(Vec::len).sum();
    for (name, sent, pending) in [
        ("without pending structs", complete, false),
        ("with pending structs", pending, true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (median, bytes) = median_load(dir.path(), sent, pending);
        eprintln!(
            "load, {name}, 10,000 records of {size} bytes in all: median {median:.2?}, {bytes} bytes"
        );
    }
}

#[test]
#[ignore = "a measurement"]
fn measure_merge_of_10_000_records() {
    let updates = each_needing_the_one_before(10_000);
    let started = Instant::now();
    let once = yrs::merge_updates_v1(&updates).unwrap();
    let in_one_merge = started.elapsed();
    let started = Instant::now();
    let merged = merge(updates.iter().map(Vec::as_slice));
    let fan_in = started.elapsed();
    assert_eq!(merged.payload.len(), once.len());
    eprintln!(
        "merge of 10,000 records: {in_one_merge:.2?} in one merge, {fan_in:.2?} 32 at a time"
    );
}
