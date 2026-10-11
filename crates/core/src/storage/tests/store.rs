use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use yrs::{Doc, Map, ReadTxn, StateVector, Text, TextPrelim, TextRef, Transact};

use super::{Env, assert_doc, doc, soon};
use crate::storage::sim_fs::OpKind;
use crate::storage::state::Limits;
use crate::storage::store::AppendError;
use crate::storage::testkit::{UpdateGen, apply};
use crate::storage::writer::Step;

#[test]
fn docs_round_trip_through_close_and_reopen() {
    let env = Env::manual();
    let docs = [doc("workspace"), doc("page-a"), doc("page-b")];
    let mut gens: Vec<UpdateGen> = (0..3).map(|seed| UpdateGen::new(seed, 3)).collect();
    let mut sent = vec![Vec::new(); docs.len()];
    let store = env.open();
    for _ in 0..10 {
        for (i, id) in docs.iter().enumerate() {
            for update in gens[i].updates(5) {
                store.append(id, &update).unwrap();
                sent[i].push(update);
            }
        }
        store.run_writer_once();
    }
    assert_eq!(store.flush(soon()), [], "flush leaves nothing pending");
    assert_eq!(store.close(), [], "close finds nothing pending");

    let store = env.open();
    for (i, id) in docs.iter().enumerate() {
        assert_doc(&store, id, &sent[i]);
    }
    assert_doc(&store, &doc("never-written"), &[]);
}

#[test]
fn a_batch_makes_one_full_sync_after_fsyncing_every_file_and_new_entry() {
    let env = Env::manual();
    let store = env.open();
    let mut updates = UpdateGen::new(7, 2);
    let docs: Vec<_> = (0..50).map(|i| doc(&format!("page-{i}"))).collect();

    for round in ["new docs", "existing segments"] {
        let seqs: Vec<_> = docs
            .iter()
            .map(|d| store.append(d, &updates.next_update()).unwrap())
            .collect();
        let before = env.fs.ops().len();
        assert!(matches!(store.run_writer_once(), Step::Wrote), "{round}");
        let ops = env.fs.ops()[before..].to_vec();

        let paths = |kinds: &[OpKind]| -> BTreeSet<PathBuf> {
            ops.iter()
                .filter(|op| kinds.contains(&op.kind))
                .map(|op| op.path.clone())
                .collect()
        };
        let written = paths(&[OpKind::Append]);
        let new_entries: BTreeSet<PathBuf> = paths(&[OpKind::Create, OpKind::CreateDir])
            .iter()
            .map(|p| p.parent().unwrap().to_path_buf())
            .collect();
        let full_syncs: Vec<_> = ops
            .iter()
            .filter(|op| op.kind == OpKind::FullSync)
            .collect();
        assert_eq!(full_syncs.len(), 1, "{round}: one full sync");
        assert_eq!(
            ops.last().unwrap().kind,
            OpKind::FullSync,
            "{round}: full sync comes last"
        );
        assert_eq!(written.len(), 50, "{round}: one write per doc");
        let expected_dirs = if round == "new docs" { 51 } else { 0 };
        assert_eq!(
            new_entries.len(),
            expected_dirs,
            "{round}: docs/ and each doc dir"
        );
        let fsynced = paths(&[OpKind::Fsync]);
        assert_eq!(
            fsynced,
            &written | &new_entries,
            "{round}: exactly these were fsynced"
        );
        assert!(
            ops.iter().all(|op| op.ok),
            "{round}: every operation succeeded"
        );
        for (d, seq) in docs.iter().zip(seqs) {
            assert!(store.is_durable(d, seq), "{round}: {d} durable");
        }
    }
}

#[test]
fn flush_then_close_keeps_every_update() {
    let env = Env::threaded();
    env.fs
        .set_latency(OpKind::FullSync, Duration::from_millis(6));
    let docs = [doc("a"), doc("b")];
    let mut gens = [UpdateGen::new(11, 3), UpdateGen::new(12, 3)];
    let mut sent = vec![Vec::new(), Vec::new()];
    let store = env.open();
    for i in 0..200 {
        let update = gens[i % 2].next_update();
        store.append(&docs[i % 2], &update).unwrap();
        sent[i % 2].push(update);
    }
    assert_eq!(
        store.flush(soon()),
        [],
        "flush finishes before its deadline"
    );
    assert_eq!(store.close(), []);

    let store = env.open();
    for (id, updates) in docs.iter().zip(&sent) {
        assert_doc(&store, id, updates);
    }
}

#[test]
fn bytes_that_do_not_decode_are_rejected_and_write_nothing() {
    let env = Env::manual();
    let store = env.open();
    let before = env.fs.ops().len();
    for garbage in [&[][..], &[1, 2, 3, 4, 5], b"not a yjs update"] {
        assert!(matches!(
            store.append(&doc("a"), garbage),
            Err(AppendError::InvalidUpdate)
        ));
    }
    assert_eq!(store.flush(soon()), []);
    assert_eq!(env.fs.ops().len(), before, "no filesystem operation at all");
    assert_doc(&store, &doc("a"), &[]);
}

/// Returns `good` and `bad`. After `good`, `bad` decodes but fails to apply, part way through.
fn partly_applying_update() -> (Vec<u8>, Vec<u8>) {
    // Client 2 edits a text nested at item (1, 0). After `good`, (1, 0) is plain text instead,
    // so client 2's update can't apply. Its first block, a root text insert, applies before the
    // failure.
    let parent = Doc::with_client_id(1);
    let map = parent.get_or_insert_map("m");
    map.insert(&mut parent.transact_mut(), "k", TextPrelim::new(""));
    let editor = Doc::with_client_id(2);
    apply(
        &editor,
        &parent
            .transact()
            .encode_state_as_update_v1(&StateVector::default()),
    );
    let nested: TextRef = editor
        .get_or_insert_map("m")
        .get(&editor.transact(), "k")
        .unwrap()
        .cast()
        .unwrap();
    let root = editor.get_or_insert_text("t");
    let bad = {
        let mut txn = editor.transact_mut();
        root.insert(&mut txn, 0, "partial");
        nested.insert(&mut txn, 0, "x");
        txn.encode_update_v1()
    };
    let other = Doc::with_client_id(1);
    let text = other.get_or_insert_text("t");
    let good = {
        let mut txn = other.transact_mut();
        text.insert(&mut txn, 0, "kept");
        txn.encode_update_v1()
    };
    (good, bad)
}

#[test]
fn an_update_that_fails_to_apply_is_rejected_and_leaves_no_trace_in_memory() {
    let env = Env::manual();
    let store = env.open();
    let (good, bad) = partly_applying_update();
    store.append(&doc("a"), &good).unwrap();
    assert!(matches!(
        store.append(&doc("a"), &bad),
        Err(AppendError::InvalidUpdate)
    ));
    assert_doc(&store, &doc("a"), std::slice::from_ref(&good));
    assert_eq!(store.flush(soon()), []);
    let appended: Vec<_> = env
        .fs
        .ops()
        .into_iter()
        .filter(|op| op.kind == OpKind::Append && op.path.starts_with("/data/docs"))
        .collect();
    assert_eq!(appended.len(), 1, "only the good update is written");
}

#[test]
fn loading_writes_nothing_and_the_first_append_starts_a_new_segment() {
    let env = Env::manual();
    let store = env.open();
    let mut updates = UpdateGen::new(3, 1);
    let first = updates.next_update();
    store.append(&doc("a"), &first).unwrap();
    assert_eq!(store.flush(soon()), []);
    drop(store);

    let store = env.open();
    let before = env.fs.ops().len();
    assert_doc(&store, &doc("a"), std::slice::from_ref(&first));
    let kinds: Vec<_> = env.fs.ops()[before..].iter().map(|op| op.kind).collect();
    assert_eq!(
        kinds,
        [OpKind::List, OpKind::Read],
        "a load only lists and reads"
    );

    let second = updates.next_update();
    store.append(&doc("a"), &second).unwrap();
    assert_eq!(store.flush(soon()), []);
    assert_eq!(
        env.doc_files(&doc("a")),
        [
            "log-0000000000-0000000000.ylog",
            "log-0000000000-0000000001.ylog"
        ]
    );
    drop(store);
    assert_doc(&env.open(), &doc("a"), &[first, second]);
}

#[test]
fn a_doc_that_fails_to_rebuild_stays_stale_until_a_rebuild_succeeds() {
    let mut env = Env::manual();
    env.limits = Limits {
        bytes: u64::MAX,
        records: 0,
    };
    let store = env.open();
    let a = doc("a");
    let (good, bad) = partly_applying_update();
    let later = {
        let client = Doc::with_client_id(3);
        let text = client.get_or_insert_text("u");
        let mut txn = client.transact_mut();
        text.insert(&mut txn, 0, "later");
        txn.encode_update_v1()
    };
    store.append(&a, &good).unwrap();
    assert_eq!(store.flush(soon()), []);
    assert_eq!(store.queued_compactions(), std::slice::from_ref(&a));
    store.append(&a, &later).unwrap();

    env.fs.fail(OpKind::Read, "/docs/a/", libc::EIO);
    assert!(store.append(&a, &bad).is_err());
    assert!(
        matches!(store.load(&a), Err(e) if e.raw_os_error() == Some(libc::EIO)),
        "a load retries the rebuild first"
    );
    assert!(matches!(
        store.append(&a, &later),
        Err(AppendError::Io(e)) if e.raw_os_error() == Some(libc::EIO)
    ));
    let (_, compacted) = store.run_compactor_once().expect("a was queued");
    assert!(compacted.is_ok());
    assert_eq!(
        env.doc_files(&a),
        ["log-0000000000-0000000000.ylog"],
        "compaction skips a stale doc"
    );
    assert_eq!(store.flush(soon()), [], "its queue is still written");

    env.fs.clear_faults();
    assert_doc(&store, &a, &[good.clone(), later.clone()]);
    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"]);
    drop(store);
    assert_doc(&env.open(), &a, &[good, later]);
}
