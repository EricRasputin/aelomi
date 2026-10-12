use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use yrs::{Doc, Map, ReadTxn, StateVector, Text, TextPrelim, TextRef, Transact};

use super::{
    Delivery, Env, applied, assert_doc, doc, frames, in_order, log_name, ops_of_updates,
    out_of_order, snapshot_name, soon, typed, with_a_loner_sixth, without,
};
use crate::storage::compact::CompactError;
use crate::storage::log::encode_segment;
use crate::storage::sim_fs::{Crash, OpKind};
use crate::storage::snapshot::encode_snapshot;
use crate::storage::state::Limits;
use crate::storage::store::{AppendError, Store};
use crate::storage::testkit::{SplitMix64, UpdateGen, apply, doc_from, fingerprint};
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
    store.close(soon()).unwrap();

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
    store.close(soon()).unwrap();

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

#[test]
fn an_update_larger_than_a_record_holds_is_rejected_and_stores_nothing() {
    let insert = |s: &str| {
        let client = Doc::with_client_id(1);
        let text = client.get_or_insert_text("text");
        let mut txn = client.transact_mut();
        text.insert(&mut txn, 0, s);
        txn.encode_update_v1()
    };
    let (fits, over) = (insert("ab"), insert("abc"));
    assert_eq!(over.len(), fits.len() + 1);
    let mut env = Env::manual();
    env.limits.max_payload = fits.len() as u32;
    let store = env.open();
    let a = doc("a");

    assert!(matches!(
        store.append(&a, &over),
        Err(AppendError::InvalidUpdate)
    ));
    assert_doc(&store, &a, &[]);
    assert_eq!(store.flush(soon()), []);
    assert_eq!(env.doc_files(&a), Vec::<String>::new());
    store.append(&a, &fits).unwrap();
    assert_eq!(store.flush(soon()), []);
    drop(store);
    assert_doc(&env.open(), &a, &[fits]);
}

struct PartlyApplying {
    good: Vec<u8>,
    bad: Vec<u8>,
}

fn partly_applying_update() -> PartlyApplying {
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
    PartlyApplying { good, bad }
}

#[test]
fn an_update_that_fails_to_apply_is_rejected_and_leaves_no_trace_in_memory() {
    let env = Env::manual();
    let store = env.open();
    let PartlyApplying { good, bad } = partly_applying_update();
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
    assert_eq!(env.doc_files(&doc("a")), [log_name(0, 0), log_name(0, 1)]);
    drop(store);
    assert_doc(&env.open(), &doc("a"), &[first, second]);
}

#[test]
fn a_doc_with_pending_structs_loads_as_bytes_that_apply_and_hold_exactly_its_operations() {
    for seed in [35, 2, 7, 0, 5, 8, 47] {
        let sent = out_of_order(seed, 80, Delivery::Shuffled);
        let env = Env::manual();
        let store = env.open();
        let a = doc("a");
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert!(store.has_pending_structs(&a), "seed {seed}");
        let queued = store.load(&a).unwrap();
        assert_eq!(store.flush(soon()), []);
        drop(store);
        let stored = env.open().load(&a).unwrap();

        for (from, bytes) in [("queue", queued), ("files", stored)] {
            let case = format!("seed {seed}, loaded from its {from}");
            let (held, loaded) = applied(&bytes, &case);
            assert_eq!(held, ops_of_updates(&sent), "{case}");
            assert_eq!(
                fingerprint(&loaded),
                fingerprint(&in_order(seed, 80)),
                "{case}"
            );
        }
    }
}

#[test]
fn a_doc_missing_an_update_loads_every_operation_and_settles_once_it_arrives() {
    for seed in [83, 87, 100] {
        let mut sent = out_of_order(seed, 40, Delivery::Shuffled);
        let withheld = sent.remove(SplitMix64::new(seed).below(40) as usize);
        let env = Env::manual();
        let store = env.open();
        let a = doc("a");
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert!(store.has_pending_structs(&a), "seed {seed}");
        let (held, _) = applied(&store.load(&a).unwrap(), &format!("seed {seed}"));
        assert_eq!(held, ops_of_updates(&sent), "seed {seed}");

        store.append(&a, &withheld).unwrap();
        let (_, loaded) = applied(&store.load(&a).unwrap(), &format!("seed {seed}"));
        assert_eq!(
            fingerprint(&loaded),
            fingerprint(&in_order(seed, 40)),
            "seed {seed}"
        );
    }
}

#[test]
fn a_doc_without_pending_structs_loads_from_memory_without_reading_its_files() {
    let sent = UpdateGen::new(4, 3).updates(80);
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    drop(store);
    let store = env.open();
    store.load(&a).unwrap();
    assert!(!store.has_pending_structs(&a));

    let before = env.fs.ops().len();
    let (held, loaded) = applied(&store.load(&a).unwrap(), "in order");
    assert_eq!(env.fs.ops().len(), before, "a second load reads nothing");
    assert_eq!(fingerprint(&loaded), fingerprint(&in_order(4, 80)));
    assert!(held.contains(&ops_of_updates(&sent)));
}

#[test]
fn a_doc_that_fails_to_rebuild_stays_stale_until_a_rebuild_succeeds() {
    let mut env = Env::manual();
    env.limits = Limits {
        bytes: u64::MAX,
        records: 0,
        ..Limits::DATA_6
    };
    let store = env.open();
    let a = doc("a");
    let PartlyApplying { good, bad } = partly_applying_update();
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
        [log_name(0, 0)],
        "compaction skips a stale doc"
    );
    assert_eq!(store.flush(soon()), [], "its queue is still written");

    env.fs.clear_faults();
    assert_doc(&store, &a, &[good.clone(), later.clone()]);
    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), [snapshot_name(1)]);
    drop(store);
    assert_doc(&env.open(), &a, &[good, later]);
}

#[test]
fn a_rebuild_keeps_damage_it_finds_for_quarantine() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let PartlyApplying { good, bad } = partly_applying_update();
    let sent = with_a_loner_sixth([vec![good], UpdateGen::new(31, 2).updates(9)].concat());
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);

    let name = log_name(0, 0);
    let path = env.path("docs/a").join(&name);
    env.fs.flip_bit(&path, frames(&sent)[5].body.start + 1);
    let damaged = env.fs.files()[&path].clone();

    assert!(matches!(
        store.append(&a, &bad),
        Err(AppendError::InvalidUpdate)
    ));
    assert_doc(&store, &a, &without(&sent, &[5]));
    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), [snapshot_name(1)]);
    assert_eq!(
        env.fs.files().get(&env.path("quarantine/a").join(&name)),
        Some(&damaged),
        "moved byte for byte"
    );
    drop(store);
    assert_doc(&env.open(), &a, &without(&sent, &[5]));
}

#[test]
fn an_update_after_a_rebuild_survives_damage_the_rebuild_read_in_the_open_segment() {
    let later = UpdateGen::new(41, 1).next_update();
    for poisoned in [false, true] {
        let env = Env::manual();
        let store = env.open();
        let a = doc("a");
        let PartlyApplying { good, bad } = partly_applying_update();
        let sent = with_a_loner_sixth([vec![good], UpdateGen::new(31, 2).updates(9)].concat());
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert_eq!(store.flush(soon()), []);
        let open = env.path("docs/a").join(log_name(0, 0));
        env.fs.flip_bit(&open, frames(&sent)[5].body.start + 1);

        if poisoned {
            store.panic_while_applying(&a, &UpdateGen::new(42, 1).next_update());
        } else {
            assert!(store.append(&a, &bad).is_err());
        }
        assert_doc(&store, &a, &without(&sent, &[5]));
        store.append(&a, &later).unwrap();
        assert_eq!(store.flush(soon()), []);
        drop(store);
        env.fs.crash(Crash::DropUnsynced);

        let expected = [without(&sent, &[5]), vec![later.clone()]].concat();
        assert_doc(&env.open(), &a, &expected);
    }
}

#[test]
fn a_queue_written_while_a_rebuild_fails_survives_damage_in_the_open_segment() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let PartlyApplying { good, bad } = partly_applying_update();
    let sent = with_a_loner_sixth([vec![good], UpdateGen::new(31, 2).updates(9)].concat());
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    let open = env.path("docs/a").join(log_name(0, 0));
    env.fs.flip_bit(&open, frames(&sent)[5].body.start + 1);

    let later = UpdateGen::new(41, 1).next_update();
    store.append(&a, &later).unwrap();
    assert!(store.append(&a, &bad).is_err());
    env.fs.fail(OpKind::Read, "/docs/a/", libc::EIO);
    assert!(store.load(&a).is_err(), "the rebuild fails");
    assert_eq!(store.flush(soon()), [], "its queue is still written");
    drop(store);
    env.fs.clear_faults();
    env.fs.crash(Crash::DropUnsynced);

    let expected = [without(&sent, &[5]), vec![later.clone()]].concat();
    assert_doc(&env.open(), &a, &expected);
}

#[test]
fn an_update_written_behind_damage_in_the_open_segment_survives_a_crash() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let sent = with_a_loner_sixth(UpdateGen::new(31, 2).updates(10));
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    let open = env.path("docs/a").join(log_name(0, 0));
    env.fs.flip_bit(&open, frames(&sent)[5].body.start + 1);

    let later = UpdateGen::new(41, 1).next_update();
    store.append(&a, &later).unwrap();
    assert_eq!(store.flush(soon()), []);
    assert_eq!(
        env.doc_files(&a),
        [log_name(0, 0)],
        "written behind the damage"
    );
    drop(store);
    env.fs.crash(Crash::DropUnsynced);

    let expected = [without(&sent, &[5]), vec![later]].concat();
    assert_doc(&env.open(), &a, &expected);
}

#[test]
fn an_update_after_a_pending_doc_loads_survives_damage_the_load_read_in_the_open_segment() {
    let sent = out_of_order(35, 80, Delivery::Shuffled);
    let later = UpdateGen::new(41, 1).next_update();
    for (damage, intact) in [("mid-file", 9), ("in the last record", 79)] {
        let env = Env::manual();
        let store = env.open();
        let a = doc("a");
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert!(store.has_pending_structs(&a));
        assert_eq!(store.flush(soon()), []);
        let open = env.path("docs/a").join(log_name(0, 0));
        env.fs.flip_bit(&open, frames(&sent)[intact].body.start + 1);

        store.load(&a).unwrap();
        store.append(&a, &later).unwrap();
        assert_eq!(store.flush(soon()), []);
        drop(store);
        env.fs.crash(Crash::DropUnsynced);

        let (held, _) = applied(&env.open().load(&a).unwrap(), damage);
        let expected = [&sent[..intact], std::slice::from_ref(&later)].concat();
        assert!(held.contains(&ops_of_updates(&expected)), "damage {damage}");
    }
}

#[test]
fn the_next_compaction_quarantines_damage_a_pending_doc_load_reads() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let sent = out_of_order(35, 80, Delivery::Shuffled);
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert!(store.has_pending_structs(&a));
    assert_eq!(store.flush(soon()), []);
    let name = log_name(0, 0);
    let path = env.path("docs/a").join(&name);
    env.fs.flip_bit(&path, frames(&sent)[9].body.start + 1);
    let damaged = env.fs.files()[&path].clone();

    store.load(&a).unwrap();
    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), [snapshot_name(1)]);
    assert_eq!(
        env.fs.files().get(&env.path("quarantine/a").join(&name)),
        Some(&damaged),
        "moved byte for byte"
    );
    drop(store);
    let (held, _) = applied(&env.open().load(&a).unwrap(), "reopened");
    assert!(held.contains(&ops_of_updates(&without(&sent, &[9]))));
}

#[test]
fn a_rebuild_queues_the_compaction_a_stale_doc_skipped() {
    let later = UpdateGen::new(41, 1).next_update();
    for rebuild_by_appending in [false, true] {
        let mut env = Env::manual();
        env.limits = Limits {
            bytes: u64::MAX,
            records: 0,
            ..Limits::DATA_6
        };
        let store = env.open();
        let a = doc("a");
        let PartlyApplying { good, bad } = partly_applying_update();
        store.append(&a, &good).unwrap();
        assert_eq!(store.flush(soon()), []);
        assert!(store.append(&a, &bad).is_err());
        let (_, skipped) = store.run_compactor_once().expect("a was queued");
        assert!(skipped.is_ok());
        assert_eq!(store.queued_compactions(), []);

        let mut sent = vec![good];
        if rebuild_by_appending {
            store.append(&a, &later).unwrap();
            sent.push(later.clone());
        } else {
            assert_doc(&store, &a, &sent);
        }
        assert_eq!(
            store.queued_compactions(),
            std::slice::from_ref(&a),
            "rebuilt by appending: {rebuild_by_appending}"
        );
        let (_, compacted) = store.run_compactor_once().expect("a was queued");
        assert!(compacted.is_ok());
        assert_eq!(env.doc_files(&a), [snapshot_name(1)]);
        assert_eq!(store.flush(soon()), []);
        drop(store);
        assert_doc(&env.open(), &a, &sent);
    }
}

#[test]
fn compaction_quarantines_damage_a_rebuild_finds_while_it_runs() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let PartlyApplying { good, bad } = partly_applying_update();
    let sent = with_a_loner_sixth([vec![good], UpdateGen::new(31, 2).updates(9)].concat());
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    let rotated = store.begin_compaction(&a).unwrap();

    let name = log_name(0, 0);
    let path = env.path("docs/a").join(&name);
    env.fs.flip_bit(&path, frames(&sent)[5].body.start + 1);
    let damaged = env.fs.files()[&path].clone();
    assert!(matches!(
        store.append(&a, &bad),
        Err(AppendError::InvalidUpdate)
    ));
    assert_doc(&store, &a, &without(&sent, &[5]));

    store.finish_compaction(&a, rotated).unwrap();
    assert_doc(&store, &a, &sent);
    assert_eq!(env.doc_files(&a), [snapshot_name(1)]);
    assert_eq!(
        env.fs.files().get(&env.path("quarantine/a").join(&name)),
        Some(&damaged),
        "moved byte for byte"
    );
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

fn plant_an_older_snapshot_the_loaded_doc_lacks(env: &Env) -> (Store, Vec<Vec<u8>>) {
    let store = env.open();
    let a = doc("a");
    let sent = UpdateGen::new(31, 2).updates(10);
    for update in &sent[..5] {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    let all_ten = doc_from(sent.iter().map(Vec::as_slice))
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let older = env.path("docs/a/snapshot-0000000000.ysnap");
    env.fs.put_durable(&older, &encode_snapshot(&all_ten));
    (store, sent)
}

fn keep_a_snapshot_the_loaded_doc_lacks(env: &Env) -> (Store, Vec<Vec<u8>>) {
    let a = doc("a");
    let (store, sent) = plant_an_older_snapshot_the_loaded_doc_lacks(env);
    let older = env.path("docs/a/snapshot-0000000000.ysnap");
    let holding = env.fs.files()[&older].clone();

    store.compact(&a).unwrap();
    assert_eq!(
        env.doc_files(&a),
        ["snapshot-0000000000.ysnap", snapshot_name(1).as_str()],
        "snapshot 1 lacks updates 6 to 10"
    );
    assert_eq!(env.fs.files().get(&older), Some(&holding));
    (store, sent)
}

#[test]
fn a_later_compaction_keeps_a_snapshot_the_loaded_doc_lacks() {
    let env = Env::manual();
    let a = doc("a");
    let (store, sent) = keep_a_snapshot_the_loaded_doc_lacks(&env);
    drop(store);

    let store = env.open();
    assert_doc(&store, &a, &sent);
    let (_, compacted) = store
        .run_compactor_once()
        .expect("the older snapshot queues a compaction");
    compacted.unwrap();
    assert_eq!(env.doc_files(&a), [snapshot_name(2)]);
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn a_kept_snapshot_merges_back_without_reopening() {
    let env = Env::manual();
    let a = doc("a");
    let (store, sent) = keep_a_snapshot_the_loaded_doc_lacks(&env);
    assert_doc(&store, &a, &sent);
    let (_, compacted) = store
        .run_compactor_once()
        .expect("the older snapshot queues a compaction");
    compacted.unwrap();
    assert_eq!(env.doc_files(&a), [snapshot_name(2)]);
    assert_eq!(store.queued_compactions(), []);
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn a_kept_snapshot_merges_back_even_if_a_sync_after_the_keep_fails() {
    let env = Env::manual();
    let a = doc("a");
    let (store, sent) = plant_an_older_snapshot_the_loaded_doc_lacks(&env);
    let publish_then_retire = 1;
    env.fs.fail_at_after(
        OpKind::FullSync,
        &env.path("docs/a"),
        publish_then_retire,
        libc::EIO,
    );
    assert!(matches!(store.compact(&a), Err(CompactError::Io(_))));
    env.fs.clear_faults();
    assert_eq!(
        env.doc_files(&a),
        ["snapshot-0000000000.ysnap", snapshot_name(1).as_str()],
        "snapshot 1 lacks updates 6 to 10"
    );
    assert_doc(&store, &a, &sent);
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn a_compaction_asked_for_while_one_runs_is_queued_once_when_it_ends() {
    let PartlyApplying { good, bad } = partly_applying_update();
    let more = UpdateGen::new(51, 2).updates(3);
    for ask_while_running in [false, true] {
        let mut env = Env::manual();
        env.limits = Limits {
            bytes: u64::MAX,
            records: 1,
            ..Limits::DATA_6
        };
        let store = env.open();
        let a = doc("a");
        let mut sent = vec![good.clone(), more[0].clone()];
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert_eq!(store.flush(soon()), []);
        store.append(&a, &more[1]).unwrap();
        sent.push(more[1].clone());
        assert_eq!(store.flush(soon()), []);
        assert_eq!(
            store.queued_compactions(),
            std::slice::from_ref(&a),
            "asked twice before it runs"
        );

        let rotated = store.begin_compaction(&a).unwrap();
        if ask_while_running {
            assert!(store.append(&a, &bad).is_err());
            assert_doc(&store, &a, &sent);
            store.append(&a, &more[2]).unwrap();
            sent.push(more[2].clone());
            assert_eq!(store.flush(soon()), []);
        }
        store.finish_compaction(&a, rotated).unwrap();
        if ask_while_running {
            assert_eq!(
                store.queued_compactions(),
                std::slice::from_ref(&a),
                "asked by a rebuild and by the writer while it ran"
            );
            assert_doc(&store, &a, &sent);
            let (_, compacted) = store.run_compactor_once().expect("a was queued");
            assert!(compacted.is_ok());
            assert_eq!(env.doc_files(&a), [snapshot_name(2)]);
        } else {
            assert_eq!(env.doc_files(&a), [snapshot_name(1)]);
        }
        assert_eq!(
            store.queued_compactions(),
            [],
            "asked while running: {ask_while_running}"
        );
        drop(store);
        assert_doc(&env.open(), &a, &sent);
    }
}

#[test]
fn a_kept_record_that_never_applies_costs_no_compaction_per_edit() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let PartlyApplying { good, bad } = partly_applying_update();
    store.append(&a, &good).unwrap();
    assert_eq!(store.flush(soon()), []);
    let name = "log-0000000000-0000000005.ylog";
    env.fs
        .put_durable(&env.path("docs/a").join(name), &encode_segment(&[&bad]));
    drop(store);
    let store = env.open();
    store.load(&a).unwrap();
    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), [name, snapshot_name(1).as_str()]);
    let mut sent = vec![good];
    for client in 50..54 {
        sent.push(typed(client, 1));
        store.append(&a, sent.last().unwrap()).unwrap();
        assert_eq!(store.flush(soon()), []);
        assert!(store.run_compactor_once().is_none(), "client {client}");
    }
    let (held, _) = applied(&store.load(&a).unwrap(), "loaded");
    assert!(held.contains(&ops_of_updates(&sent)));
    assert_eq!(
        env.doc_files(&a),
        [name, log_name(1, 0).as_str(), snapshot_name(1).as_str()]
    );
}
