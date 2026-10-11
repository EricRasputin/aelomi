use yrs::{Doc, GetString, Text, Transact};

use super::{Env, assert_doc, doc, soon};
use crate::storage::sim_fs::Crash;
use crate::storage::state::Limits;
use crate::storage::store::Store;
use crate::storage::testkit::UpdateGen;

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
        steps, 13,
        "create, write, fsync, full sync, read, rename, fsync, full sync, list, 2 removes, fsync, full sync"
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
            drop(store);
            assert_doc(&env.open(), &a, &sent);
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
fn a_corrupt_log_loads_up_to_the_damage_and_is_quarantined_not_deleted() {
    let env = Env::manual();
    let a = doc("a");
    let sent = UpdateGen::new(31, 2).updates(10);
    let store = env.open();
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    drop(store);

    let name = "log-0000000000-0000000000.ylog";
    let path = env.path("docs/a").join(name);
    let sixth = 8 + sent[..5].iter().map(|u| 8 + u.len()).sum::<usize>();
    env.fs.flip_bit(&path, sixth + 9);
    let damaged = env.fs.files()[&path].clone();

    let store = env.open();
    assert_doc(&store, &a, &sent[..5]);
    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"]);
    let quarantined = env.path("quarantine/a").join(name);
    assert_eq!(
        env.fs.files().get(&quarantined),
        Some(&damaged),
        "moved byte for byte"
    );
    drop(store);
    assert_doc(&env.open(), &a, &sent[..5]);
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
fn compaction_runs_beside_the_writer_without_losing_updates() {
    let mut env = Env::threaded();
    env.limits = Limits {
        bytes: 4_000,
        records: 40,
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
    assert_eq!(store.close(), []);

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
