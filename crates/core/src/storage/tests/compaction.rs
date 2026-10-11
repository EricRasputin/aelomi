use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact, Update};

use super::{
    Delivery, Env, applied, assert_doc, doc, in_order, ops_of_updates, out_of_order, record_at,
    soon, typed, with_a_loner_sixth, without,
};
use crate::storage::compact::CompactError;
use crate::storage::equality::Ops;
use crate::storage::log::{LOG_MAGIC, LogEnd, encode_segment, read_log};
use crate::storage::sim_fs::{Crash, OpKind};
use crate::storage::snapshot::{encode_snapshot, read_snapshot};
use crate::storage::state::Limits;
use crate::storage::store::Store;
use crate::storage::testkit::{UpdateGen, apply, doc_from, fingerprint};

fn two_generations(env: &Env) -> (Store, Vec<Vec<u8>>) {
    let store = env.open();
    let a = doc("a");
    let mut updates = UpdateGen::new(21, 3);
    let mut sent = Vec::new();
    for round in 0..4 {
        for update in updates.updates(10) {
            store.append(&a, &update).unwrap();
            sent.push(update);
        }
        store.run_writer_once();
        if round == 1 {
            store.compact(&a).unwrap();
        }
    }
    assert_eq!(
        env.doc_files(&a),
        [
            "log-0000000001-0000000000.ylog",
            "snapshot-0000000001.ysnap"
        ]
    );
    (store, sent)
}

/// Inserting at the start keeps yrs from merging them into one ever-longer string.
fn typing(n: usize, chunk: usize) -> Vec<Vec<u8>> {
    let client = Doc::with_client_id(1);
    let text = client.get_or_insert_text("text");
    (0..n)
        .map(|_| {
            let mut txn = client.transact_mut();
            text.insert(&mut txn, 0, &"x".repeat(chunk));
            txn.encode_update_v1()
        })
        .collect()
}

#[test]
fn compaction_interrupted_at_every_step_loses_nothing() {
    let a = doc("a");
    let steps = {
        let env = Env::manual();
        let (store, _) = two_generations(&env);
        let before = env.fs.ops().len();
        store.compact(&a).unwrap();
        env.fs.ops().len() - before
    };
    assert_eq!(
        steps, 15,
        "create, write, fsync, full sync, read, rename, fsync, full sync, list, 2 reads, 2 removes, fsync, full sync"
    );

    for k in 0..=steps {
        for crash in [Crash::DropUnsynced, Crash::KeepTornPrefix(k as u64)] {
            let env = Env::manual();
            let (store, sent) = two_generations(&env);
            env.fs.die_after(k);
            let _ = store.compact(&a);
            drop(store);
            env.fs.crash(crash);

            let store = env.open();
            assert_doc(&store, &a, &sent);
            let result = match store.run_compactor_once() {
                Some((_, result)) => result,
                None => store.compact(&a),
            };
            result.unwrap_or_else(|e| {
                panic!("crash after {k} ops ({crash:?}): next compaction fails: {e}")
            });
            let files = env.doc_files(&a);
            assert_eq!(files.len(), 1, "crash after {k} ops ({crash:?}): {files:?}");
            assert!(files[0].starts_with("snapshot-"), "{files:?}");
            assert_eq!(env.quarantine(), [].into(), "the old generation is deleted");
            drop(store);
            assert_doc(&env.open(), &a, &sent);
        }
    }
}

fn two_generations_pending(env: &Env, sent: &[Vec<u8>]) -> Store {
    let store = env.open();
    let a = doc("a");
    let (first, second) = sent.split_at(sent.len() / 2);
    for (half, then_compact) in [(first, true), (second, false)] {
        for update in half {
            store.append(&a, update).unwrap();
        }
        assert_eq!(store.flush(soon()), []);
        if then_compact {
            store.compact(&a).unwrap();
        }
    }
    assert!(store.has_pending_structs(&a));
    assert_eq!(
        env.doc_files(&a),
        [
            "log-0000000001-0000000000.ylog",
            "snapshot-0000000001.ysnap"
        ]
    );
    store
}

fn assert_holds_what_was_sent(store: &Store, sent: &[Vec<u8>], case: &str) {
    let (held, _) = applied(&store.load(&doc("a")).unwrap(), case);
    assert!(
        held.contains(&ops_of_updates(sent)),
        "{case}: lost an update"
    );
    assert!(
        held.only_from(&ops_of_updates(sent)),
        "{case}: holds something never appended"
    );
}

#[test]
fn compaction_of_a_doc_with_pending_structs_interrupted_at_every_step_loses_nothing() {
    let a = doc("a");
    for (delivery, seed, n) in [
        (Delivery::Shuffled, 35, 80),
        (Delivery::OneWithheld, 17, 12),
    ] {
        let sent = out_of_order(seed, n, delivery);
        let steps = {
            let env = Env::manual();
            let store = two_generations_pending(&env, &sent);
            let before = env.fs.ops().len();
            store.compact(&a).unwrap();
            env.fs.ops().len() - before
        };
        assert_eq!(
            steps, 18,
            "list and 2 reads to merge, then as many as an in-order compaction"
        );

        for k in 0..=steps {
            for crash in [Crash::DropUnsynced, Crash::KeepTornPrefix(k as u64)] {
                let case = format!("{delivery:?}, crash after {k} ops ({crash:?})");
                let env = Env::manual();
                let store = two_generations_pending(&env, &sent);
                env.fs.die_after(k);
                let _ = store.compact(&a);
                drop(store);
                env.fs.crash(crash);

                let store = env.open();
                assert_holds_what_was_sent(&store, &sent, &case);
                let result = match store.run_compactor_once() {
                    Some((_, result)) => result,
                    None => store.compact(&a),
                };
                result.unwrap_or_else(|e| panic!("{case}: next compaction fails: {e}"));
                let files = env.doc_files(&a);
                assert_eq!(files.len(), 1, "{case}: {files:?}");
                assert!(files[0].starts_with("snapshot-"), "{case}: {files:?}");
                assert_eq!(
                    env.quarantine(),
                    [].into(),
                    "{case}: the old generation is deleted"
                );
                drop(store);
                let store = env.open();
                assert_holds_what_was_sent(&store, &sent, &case);
                if let Delivery::Shuffled = delivery {
                    let loaded = doc_from([store.load(&a).unwrap().as_slice()]);
                    assert_eq!(
                        fingerprint(&loaded),
                        fingerprint(&in_order(seed, n)),
                        "{case}"
                    );
                }
            }
        }
    }
}

#[test]
fn passing_10_000_records_triggers_compaction() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let sent = typing(10_001, 1);
    for update in &sent[..10_000] {
        store.append(&a, update).unwrap();
    }
    store.run_writer_once();
    assert_eq!(
        store.queued_compactions(),
        [],
        "10,000 records is within the limit"
    );
    store.append(&a, &sent[10_000]).unwrap();
    store.run_writer_once();
    assert_eq!(
        store.queued_compactions(),
        std::slice::from_ref(&a),
        "10,001 records is over it"
    );
    let log = env.path("docs/a/log-0000000000-0000000000.ylog");
    assert!(
        env.fs.files()[&log].len() < 1_000_000,
        "the record count triggered it, not bytes"
    );

    drop(store);
    let store = env.open();
    assert_doc(&store, &a, &sent);
    assert_eq!(
        store.queued_compactions(),
        std::slice::from_ref(&a),
        "loading a doc over the limit queues it"
    );
    let (compacted, result) = store.run_compactor_once().unwrap();
    assert_eq!(compacted, a);
    result.unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"]);
    assert_doc(&store, &a, &sent);
}

#[test]
fn passing_1_mb_of_logs_triggers_compaction() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let sent = typing(1_000, 1_000);
    let mut bytes = 8;
    let within = sent
        .iter()
        .take_while(|u| {
            bytes += 8 + u.len();
            bytes <= 1_000_000
        })
        .count();
    assert!(within < 10_000, "the byte limit comes first");
    for update in &sent[..within] {
        store.append(&a, update).unwrap();
    }
    store.run_writer_once();
    let log = env.path("docs/a/log-0000000000-0000000000.ylog");
    let size = env.fs.files()[&log].len();
    assert!(size <= 1_000_000, "{size} bytes");
    assert_eq!(
        store.queued_compactions(),
        [],
        "{size} bytes is within the limit"
    );

    store.append(&a, &sent[within]).unwrap();
    store.run_writer_once();
    assert!(env.fs.files()[&log].len() > 1_000_000);
    assert_eq!(
        store.queued_compactions(),
        std::slice::from_ref(&a),
        "passing 1 MB queues it"
    );
    store.run_compactor_once().unwrap().1.unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"]);
    assert_doc(&store, &a, &sent[..=within]);
}

#[test]
fn pending_structs_survive_compaction() {
    let env = Env::manual();
    let a = doc("a");
    let client = Doc::with_client_id(1);
    let text = client.get_or_insert_text("text");
    let edit = |f: &dyn Fn(&mut yrs::TransactionMut)| {
        let mut txn = client.transact_mut();
        f(&mut txn);
        txn.encode_update_v1()
    };
    let first = edit(&|txn| text.insert(txn, 0, "ab"));
    let second = edit(&|txn| text.insert(txn, 2, "c"));
    let third = edit(&|txn| text.remove_range(txn, 0, 1));

    let store = env.open();
    store.append(&a, &second).unwrap();
    store.append(&a, &third).unwrap();
    assert_eq!(store.flush(soon()), []);
    store
        .compact(&a)
        .expect("compaction verifies a doc with pending structs");
    assert_eq!(
        env.doc_files(&a),
        ["snapshot-0000000001.ysnap"],
        "only the snapshot holds them"
    );
    drop(store);

    let store = env.open();
    assert_doc(&store, &a, &[second.clone(), third.clone()]);
    store.append(&a, &first).unwrap();
    assert_doc(&store, &a, &[first, second, third]);
    let loaded = crate::storage::testkit::doc_from([store.load(&a).unwrap().as_slice()]);
    let text = loaded.get_or_insert_text("text");
    assert_eq!(text.get_string(&loaded.transact()), "bc");
}

#[test]
fn pending_structs_still_let_compaction_delete_the_old_generation() {
    for (delivery, seed, n, pending, reapplying_changes_it) in [
        (Delivery::Shuffled, 192, 20, [true, false], true),
        (Delivery::OneWithheld, 17, 12, [true, true], false),
    ] {
        let sent = out_of_order(seed, n, delivery);
        let env = Env::manual();
        let store = env.open();
        let a = doc("a");
        let (first, second) = sent.split_at(sent.len() / 2);
        let mut snapshots = Vec::new();
        for (generation, half) in [(1, first), (2, second)] {
            for update in half {
                store.append(&a, update).unwrap();
            }
            assert_eq!(store.flush(soon()), []);
            store.compact(&a).unwrap();
            let name = format!("snapshot-{generation:010}.ysnap");
            assert_eq!(
                env.doc_files(&a),
                std::slice::from_ref(&name),
                "{delivery:?}: generation {generation}"
            );
            snapshots.push(env.fs.files()[&env.path("docs/a").join(name)].clone());
        }
        let held: Vec<_> = snapshots
            .iter()
            .map(|bytes| doc_from([read_snapshot(bytes).unwrap()]))
            .collect();
        assert_eq!(
            held.iter()
                .map(|doc| doc.transact().has_missing_updates())
                .collect::<Vec<_>>(),
            pending,
            "{delivery:?}: which snapshots hold pending structs"
        );

        let before = fingerprint(&held[0]);
        for update in first {
            apply(&held[0], update);
        }
        assert_eq!(
            fingerprint(&held[0]) != before,
            reapplying_changes_it,
            "{delivery:?}: yrs re-applies pending structs differently, so containment compares IDs"
        );
        drop(store);
        assert_doc(&env.open(), &a, &sent);
    }
}

#[test]
fn a_doc_with_pending_structs_compacts_to_a_snapshot_of_exactly_its_operations() {
    for seed in [35, 2, 7, 0, 5, 8, 47] {
        let mut sent = out_of_order(seed, 80, Delivery::Shuffled);
        let env = Env::manual();
        let store = env.open();
        let (a, b) = (doc("a"), doc("b"));
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert!(store.has_pending_structs(&a), "seed {seed}");
        assert_eq!(store.flush(soon()), []);
        store
            .compact(&a)
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let snapshot = "snapshot-0000000001.ysnap";
        assert_eq!(env.doc_files(&a), [snapshot], "seed {seed}");
        let stored = &env.fs.files()[&env.path("docs/a").join(snapshot)];
        let held = Update::decode_v1(read_snapshot(stored).unwrap()).unwrap();
        assert_eq!(Ops::of_update(&held), ops_of_updates(&sent), "seed {seed}");

        let more = UpdateGen::new(seed + 1_000, 2).updates(2);
        store.append(&a, &more[0]).unwrap();
        store.append(&b, &more[1]).unwrap();
        assert_eq!(store.flush(soon()), []);
        sent.push(more[0].clone());
        drop(store);
        let store = env.open();
        let loaded = doc_from([store.load(&a).unwrap().as_slice()]);
        let expected = in_order(seed, 80);
        apply(&expected, &more[0]);
        assert_eq!(fingerprint(&loaded), fingerprint(&expected), "seed {seed}");
        assert_doc(&store, &b, &more[1..]);
    }
}

#[test]
fn a_snapshot_damaged_on_its_way_to_disk_is_refused_and_leaves_every_file() {
    for (seed, shuffled) in [(3, false), (35, true)] {
        let sent = match shuffled {
            true => out_of_order(seed, 80, Delivery::Shuffled),
            false => UpdateGen::new(seed, 3).updates(80),
        };
        let env = Env::manual();
        let store = env.open();
        let a = doc("a");
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert_eq!(store.has_pending_structs(&a), shuffled, "seed {seed}");
        assert_eq!(store.flush(soon()), []);

        env.fs.damage_append_once(".ysnap.tmp");
        let refused = store.compact(&a);
        assert!(
            matches!(refused, Err(CompactError::Mismatch)),
            "seed {seed}: {refused:?}"
        );
        let log = "log-0000000000-0000000000.ylog";
        assert_eq!(env.doc_files(&a), [log], "seed {seed}");

        store.compact(&a).unwrap();
        assert_eq!(
            env.doc_files(&a),
            ["snapshot-0000000002.ysnap"],
            "seed {seed}"
        );
        drop(store);
        let loaded = doc_from([env.open().load(&a).unwrap().as_slice()]);
        assert_eq!(
            fingerprint(&loaded),
            fingerprint(&in_order(seed, 80)),
            "seed {seed}"
        );
    }
}

#[test]
fn a_doc_with_pending_structs_whose_merge_passes_the_payload_limit_keeps_its_files() {
    let sent = typing(5, 40);
    let mut env = Env::manual();
    env.limits = Limits {
        records: 3,
        max_payload: 150,
        ..Limits::DATA_6
    };
    let store = env.open();
    let a = doc("a");
    for update in &sent[1..] {
        store.append(&a, update).unwrap();
    }
    assert!(
        store.has_pending_structs(&a),
        "each insert needs the one before"
    );
    assert_eq!(store.flush(soon()), []);
    assert!(store.load(&a).unwrap().len() > 150);

    let (compacted, result) = store.run_compactor_once().expect("4 records queue it");
    assert_eq!(compacted, a);
    result.expect("compaction skips the doc");
    let log = "log-0000000000-0000000000.ylog";
    assert_eq!(env.doc_files(&a), [log]);

    store.append(&a, &sent[0]).unwrap();
    assert_eq!(store.flush(soon()), []);
    assert_eq!(
        env.doc_files(&a),
        [log, "log-0000000001-0000000000.ylog"],
        "the merge comes after the rotation"
    );
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn a_doc_whose_snapshot_passes_the_payload_limit_skips_compaction_and_keeps_writing() {
    let sent = typing(5, 40);
    let mut env = Env::manual();
    env.limits = Limits {
        records: 3,
        max_payload: 150,
        ..Limits::DATA_6
    };
    assert!(sent.iter().all(|u| u.len() < 150));
    let store = env.open();
    let a = doc("a");
    for update in &sent[..4] {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    let snapshot = store.load(&a).unwrap();
    assert!(snapshot.len() > 150, "{} bytes", snapshot.len());

    let (compacted, result) = store.run_compactor_once().expect("4 records queue it");
    assert_eq!(compacted, a);
    result.expect("compaction skips the doc");
    let log = "log-0000000000-0000000000.ylog";
    assert_eq!(env.doc_files(&a), [log]);

    let seq = store.append(&a, &sent[4]).unwrap();
    store.run_writer_once();
    assert!(store.is_durable(&a, seq));
    assert_eq!(env.doc_files(&a), [log], "the segment was not rotated");
    assert_eq!(
        store.queued_compactions(),
        [],
        "a skipped doc waits for another 3 records"
    );
    let bytes = &env.fs.durable_files()[&env.path("docs/a").join(log)];
    assert_eq!(read_log(bytes).records, sent);
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn a_panic_compacting_one_doc_leaves_its_files_and_the_next_doc_still_compacts() {
    let mut env = Env::manual();
    env.limits = Limits {
        bytes: u64::MAX,
        records: 3,
        ..Limits::DATA_6
    };
    let store = env.open();
    let (a, b) = (doc("a"), doc("b"));
    let mut sent_a = UpdateGen::new(110, 2).updates(4);
    let sent_b = UpdateGen::new(111, 2).updates(4);
    for update in &sent_a {
        store.append(&a, update).unwrap();
    }
    for update in &sent_b {
        store.append(&b, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    assert_eq!(store.queued_compactions(), [a.clone(), b.clone()]);
    env.fs
        .panic_once(OpKind::Read, "/docs/a/snapshot-0000000001.ysnap.tmp");

    let (compacted, result) = store.run_compactor_once().unwrap();
    assert_eq!(compacted, a);
    assert!(matches!(result, Err(CompactError::Panicked)), "{result:?}");
    assert_eq!(
        env.doc_files(&a),
        [
            "log-0000000000-0000000000.ylog",
            "snapshot-0000000001.ysnap.tmp"
        ]
    );
    let (compacted, result) = store.run_compactor_once().unwrap();
    assert_eq!(compacted, b);
    result.unwrap();
    assert_eq!(env.doc_files(&b), ["snapshot-0000000001.ysnap"]);

    let more = UpdateGen::new(112, 1).updates(4);
    for update in &more {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    assert_eq!(
        store.queued_compactions(),
        std::slice::from_ref(&a),
        "the abandoned compaction left a idle"
    );
    store.run_compactor_once().unwrap().1.unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000002.ysnap"]);
    sent_a.extend(more);
    drop(store);
    let store = env.open();
    assert_doc(&store, &a, &sent_a);
    assert_doc(&store, &b, &sent_b);
}

#[test]
fn a_damaged_log_loads_every_record_around_the_damage_and_is_quarantined_not_deleted() {
    let a = doc("a");
    let sent = with_a_loner_sixth(UpdateGen::new(31, 2).updates(10));
    let sixth = record_at(&sent, 5);
    for (damage, flips, torn_tail, lost) in [
        (
            "a bit in the sixth payload",
            vec![sixth + 9],
            false,
            vec![5],
        ),
        (
            "a high bit in the sixth length",
            vec![sixth + 2],
            false,
            vec![5],
        ),
        ("a low bit in the sixth length", vec![sixth], false, vec![5]),
        (
            "a bit in each of the last two records",
            vec![record_at(&sent, 9) + 9, record_at(&sent, 10) + 9],
            false,
            vec![9, 10],
        ),
        (
            "a bit in the sixth payload, then a torn tail",
            vec![sixth + 9],
            true,
            vec![5, 10],
        ),
    ] {
        let env = Env::manual();
        let store = env.open();
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert_eq!(store.flush(soon()), []);
        drop(store);

        let name = "log-0000000000-0000000000.ylog";
        let path = env.path("docs/a").join(name);
        let mut damaged = env.fs.files()[&path].clone();
        for at in flips {
            damaged[at] ^= 0x10;
        }
        if torn_tail {
            damaged.truncate(record_at(&sent, 10) + 8 + 3);
        }
        env.fs.put_durable(&path, &damaged);

        let store = env.open();
        let expected = without(&sent, &lost);
        assert_doc(&store, &a, &expected);
        store.compact(&a).unwrap();
        assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"], "{damage}");
        let quarantined = env.path("quarantine/a").join(name);
        assert_eq!(
            env.fs.files().get(&quarantined),
            Some(&damaged),
            "{damage}: moved byte for byte"
        );
        drop(store);
        assert_doc(&env.open(), &a, &expected);
    }
}

#[test]
fn a_torn_log_is_quarantined_at_compaction_not_deleted() {
    let a = doc("a");
    let sent = UpdateGen::new(31, 2).updates(10);
    let last = record_at(&sent, 9);
    let logged = "log-0000000000-0000000000.ylog";
    let planted = "log-0000000000-0000000001.ylog";
    for (damage, name, flip, lost) in [
        ("a bit in the last payload", logged, Some(last + 9), vec![9]),
        (
            "a high bit in the last length",
            logged,
            Some(last + 2),
            vec![9],
        ),
        ("garbage right after the log header", planted, None, vec![]),
    ] {
        let env = Env::manual();
        let store = env.open();
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert_eq!(store.flush(soon()), []);
        drop(store);

        let path = env.path("docs/a").join(name);
        let damaged = match flip {
            Some(at) => {
                let mut bytes = env.fs.files()[&path].clone();
                bytes[at] ^= 0x10;
                bytes
            }
            None => [&LOG_MAGIC[..], b"not a record"].concat(),
        };
        assert_eq!(read_log(&damaged).end, LogEnd::Torn, "{damage}");
        env.fs.put_durable(&path, &damaged);

        let store = env.open();
        let expected = without(&sent, &lost);
        assert_doc(&store, &a, &expected);
        store.compact(&a).unwrap();
        assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"], "{damage}");
        assert_eq!(
            env.quarantine(),
            [(env.path("quarantine/a").join(name), damaged)].into(),
            "{damage}: only the torn file moved, byte for byte"
        );
        drop(store);
        assert_doc(&env.open(), &a, &expected);
    }
}

#[test]
fn compaction_deletes_a_log_holding_no_record_bytes_but_quarantines_a_short_corrupt_one() {
    let env = Env::manual();
    let a = doc("a");
    let sent = UpdateGen::new(23, 2).updates(3);
    let store = env.open();
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    drop(store);
    let header = encode_segment::<Vec<u8>>(&[]);
    let corrupt = ("log-0000000000-0000000005.ylog", b"AEX".to_vec());
    for (name, bytes) in [
        ("log-0000000000-0000000001.ylog", Vec::new()),
        ("log-0000000000-0000000002.ylog", LOG_MAGIC[..5].to_vec()),
        ("log-0000000000-0000000003.ylog", header[..12].to_vec()),
        ("log-0000000000-0000000004.ylog", header),
        corrupt.clone(),
    ] {
        env.fs.put_durable(&env.path("docs/a").join(name), &bytes);
    }

    let store = env.open();
    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"]);
    assert_eq!(
        env.quarantine(),
        [(env.path("quarantine/a").join(corrupt.0), corrupt.1)].into(),
        "only the file that isn't part of a log header"
    );
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn a_torn_tail_left_by_a_crash_is_quarantined_at_compaction() {
    let a = doc("a");
    let sent = UpdateGen::new(41, 2).updates(4);
    let name = "log-0000000000-0000000000.ylog";
    let (env, torn) = (0..64)
        .find_map(|seed| {
            let env = Env::manual();
            let store = env.open();
            for update in &sent[..3] {
                store.append(&a, update).unwrap();
            }
            assert_eq!(store.flush(soon()), []);
            store.append(&a, &sent[3]).unwrap();
            env.fs.die_after(1);
            store.run_writer_once();
            drop(store);
            env.fs.crash(Crash::KeepTornPrefix(seed));
            let bytes = env.fs.files()[&env.path("docs/a").join(name)].clone();
            (read_log(&bytes).end == LogEnd::Torn).then_some((env, bytes))
        })
        .expect("a seed that keeps part of the fourth record");
    assert_eq!(read_log(&torn).records.len(), 3);

    let store = env.open();
    assert_doc(&store, &a, &sent[..3]);
    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"]);
    assert_eq!(
        env.quarantine(),
        [(env.path("quarantine/a").join(name), torn)].into(),
        "moved byte for byte"
    );
    drop(store);
    assert_doc(&env.open(), &a, &sent[..3]);
}

#[test]
fn compaction_quarantines_damage_no_load_read_when_the_snapshot_holds_every_record() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let sent = with_a_loner_sixth(UpdateGen::new(31, 2).updates(10));
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    let name = "log-0000000000-0000000000.ylog";
    let path = env.path("docs/a").join(name);
    env.fs.flip_bit(&path, record_at(&sent, 5) + 9);
    let damaged = env.fs.files()[&path].clone();

    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"]);
    assert_eq!(
        env.fs.files().get(&env.path("quarantine/a").join(name)),
        Some(&damaged),
        "moved byte for byte"
    );
    assert_doc(&store, &a, &sent);
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn compaction_keeps_a_damaged_log_until_the_doc_holds_what_it_salvages() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let sent = with_a_loner_sixth(UpdateGen::new(31, 2).updates(10));
    for update in &sent[..5] {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    let mut planted = encode_segment(&sent[5..]);
    planted[record_at(&sent[5..], 0) + 9] ^= 0x10;
    let name = "log-0000000000-0000000005.ylog";
    env.fs.put_durable(&env.path("docs/a").join(name), &planted);

    store.compact(&a).unwrap();
    assert_eq!(
        env.doc_files(&a),
        [name, "snapshot-0000000001.ysnap"],
        "snapshot 1 lacks updates 7 to 11"
    );
    let salvaged = without(&sent, &[5]);
    assert_doc(&store, &a, &salvaged);
    let (_, compacted) = store
        .run_compactor_once()
        .expect("the kept log queues a compaction");
    compacted.unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000002.ysnap"]);
    assert_eq!(
        env.fs.files().get(&env.path("quarantine/a").join(name)),
        Some(&planted),
        "moved byte for byte"
    );
    drop(store);
    assert_doc(&env.open(), &a, &salvaged);
}

#[test]
fn compaction_keeps_a_log_a_merged_load_marked_corrupt_until_the_snapshot_holds_what_it_salvages() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let first = typed(1, 2);
    store.append(&a, &first).unwrap();
    assert_eq!(store.flush(soon()), []);
    let client = Doc::with_client_id(200);
    let text = client.get_or_insert_text("u");
    text.insert(&mut client.transact_mut(), 0, "dependency");
    let dependency = client
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let pending = {
        let mut txn = client.transact_mut();
        text.insert(&mut txn, 0, "pending");
        txn.encode_update_v1()
    };
    store.append(&a, &pending).unwrap();
    assert!(store.has_pending_structs(&a));
    let planted_updates = [typed(800, 2), typed(801, 3)];
    let mut planted = encode_segment(&planted_updates);
    planted[record_at(&planted_updates, 0) + 4] ^= 1;
    let name = "log-0000000000-0000000005.ylog";
    env.fs.put_durable(&env.path("docs/a").join(name), &planted);
    store.load(&a).unwrap();
    store.append(&a, &dependency).unwrap();
    assert!(!store.has_pending_structs(&a));

    store.compact(&a).unwrap();
    assert_eq!(
        env.doc_files(&a),
        [name, "snapshot-0000000001.ysnap"],
        "snapshot 1 lacks the planted log's second record"
    );
    assert_eq!(env.quarantine(), [].into());
    drop(store);
    let store = env.open();
    let (held, _) = applied(&store.load(&a).unwrap(), "reopened");
    let salvaged = [first, dependency, pending, planted_updates[1].clone()];
    assert_eq!(held, ops_of_updates(&salvaged));
    let (_, compacted) = store
        .run_compactor_once()
        .expect("the kept log queues a compaction");
    compacted.unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000002.ysnap"]);
    assert_eq!(
        env.quarantine(),
        [(env.path("quarantine/a").join(name), planted)].into(),
        "moved byte for byte"
    );
}

#[test]
fn a_corrupt_snapshot_is_never_used_and_is_quarantined() {
    let env = Env::manual();
    let (store, sent) = two_generations(&env);
    drop(store);
    let path = env.path("docs/a/snapshot-0000000001.ysnap");
    env.fs.flip_bit(&path, 20);
    let damaged = env.fs.files()[&path].clone();

    let store = env.open();
    assert_doc(&store, &doc("a"), &sent[20..]);
    store.compact(&doc("a")).unwrap();
    let quarantined = env.path("quarantine/a/snapshot-0000000001.ysnap");
    assert_eq!(env.fs.files().get(&quarantined), Some(&damaged));
    assert_eq!(env.doc_files(&doc("a")), ["snapshot-0000000002.ysnap"]);
}

#[test]
fn recovery_loads_the_union_of_every_valid_snapshot() {
    let env = Env::manual();
    drop(env.open());
    let a = doc("a");
    let older = UpdateGen::new(61, 1).updates(5);
    let newer = UpdateGen::new(62, 1).updates(5);
    for (generation, updates) in [(1, &older), (2, &newer)] {
        let state = doc_from(updates.iter().map(Vec::as_slice))
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        let path = env.path(&format!("docs/a/snapshot-{generation:010}.ysnap"));
        env.fs.put_durable(&path, &encode_snapshot(&state));
    }
    let both = [older, newer].concat();

    let store = env.open();
    assert_doc(&store, &a, &both);
    let (_, compacted) = store
        .run_compactor_once()
        .expect("the older snapshot queues a compaction");
    compacted.unwrap();
    assert_eq!(
        env.doc_files(&a),
        ["snapshot-0000000003.ysnap"],
        "the new snapshot holds both"
    );
    drop(store);
    assert_doc(&env.open(), &a, &both);
}

#[test]
fn compaction_runs_beside_the_writer_without_losing_updates() {
    let mut env = Env::threaded();
    env.limits = Limits {
        bytes: 4_000,
        records: 40,
        ..Limits::DATA_6
    };
    let docs = [doc("a"), doc("b")];
    let mut gens = [UpdateGen::new(41, 3), UpdateGen::new(42, 3)];
    let mut sent = vec![Vec::new(), Vec::new()];
    let store = env.open();
    for i in 0..600 {
        let update = gens[i % 2].next_update();
        store.append(&docs[i % 2], &update).unwrap();
        sent[i % 2].push(update);
    }
    assert_eq!(store.flush(soon()), []);
    store.close(soon()).unwrap();

    let store = env.open();
    for (d, sent) in docs.iter().zip(&sent) {
        assert_doc(&store, d, sent);
        let snapshots = env
            .doc_files(d)
            .iter()
            .filter(|n| n.starts_with("snapshot-"))
            .count();
        assert!(snapshots >= 1, "{d} was compacted");
    }
}

#[test]
fn a_quarantined_file_survives_a_failed_sync_of_any_folder_it_moved_into() {
    let a = doc("a");
    let sent = UpdateGen::new(33, 2).updates(10);
    let name = "log-0000000000-0000000000.ylog";
    for failing in ["quarantine/a", "quarantine", ""] {
        for then in ["compact again", "append"] {
            let env = Env::manual();
            let store = env.open();
            for update in &sent {
                store.append(&a, update).unwrap();
            }
            assert_eq!(store.flush(soon()), []);
            drop(store);
            let path = env.path("docs/a").join(name);
            env.fs.flip_bit(&path, 8 + 8 + 2);
            let damaged = env.fs.files()[&path].clone();

            let store = env.open();
            env.fs.fail_at(OpKind::Fsync, &env.path(failing), libc::EIO);
            assert!(store.compact(&a).is_err(), "syncing /data/{failing} fails");
            env.fs.clear_faults();
            match then {
                "compact again" => store.compact(&a).unwrap(),
                _ => {
                    store
                        .append(&a, &UpdateGen::new(34, 2).next_update())
                        .unwrap();
                    assert_eq!(store.flush(soon()), []);
                }
            }
            drop(store);
            env.fs.crash(Crash::DropUnsynced);

            let holding: Vec<_> = env
                .fs
                .files()
                .into_iter()
                .filter(|(_, bytes)| *bytes == damaged)
                .map(|(path, _)| path)
                .collect();
            assert_eq!(
                holding,
                [env.path("quarantine/a").join(name)],
                "syncing /data/{failing} failed, then {then}"
            );
        }
    }
}
