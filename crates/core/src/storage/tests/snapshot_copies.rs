use std::ops::Range;

use yrs::{ReadTxn, StateVector, Transact};

use super::{Delivery, Env, applied, assert_doc, doc, ops_of_updates, out_of_order, soon};
use crate::storage::recover::merge;
use crate::storage::sim_fs::{Crash, OpKind};
use crate::storage::snapshot::{encode_snapshot, encode_snapshot_v2};
use crate::storage::testkit::{UpdateGen, doc_from, snapshot_with_forged_tail};

fn state_of(updates: &[Vec<u8>]) -> Vec<u8> {
    doc_from(updates.iter().map(Vec::as_slice))
        .transact()
        .encode_state_as_update_v1(&StateVector::default())
}

fn with_first_copy_zeroed(state: &[u8]) -> Vec<u8> {
    let mut bytes = encode_snapshot_v2(state);
    bytes[32..32 + state.len()].fill(0);
    bytes
}

fn intact_and_damaged(state: &[u8]) -> [(&'static str, Vec<u8>); 2] {
    [
        ("intact", encode_snapshot_v2(state)),
        ("first copy zeroed", with_first_copy_zeroed(state)),
    ]
}

#[test]
fn a_planted_format_2_snapshot_loads_through_recovery() {
    let sent = UpdateGen::new(71, 2).updates(10);
    for (case, planted) in intact_and_damaged(&state_of(&sent)) {
        let env = Env::manual();
        drop(env.open());
        let path = env.path("docs/a/snapshot-0000000003.ysnap2");
        env.fs.put_durable(&path, &planted);

        let store = env.open();
        assert!(!store.has_pending_structs(&doc("a")), "{case}");
        assert_doc(&store, &doc("a"), &sent);
    }
}

#[test]
fn a_merged_load_reads_a_format_2_snapshot() {
    let sent = out_of_order(82, 40, Delivery::OneWithheld);
    let state = merge(sent.iter().map(Vec::as_slice)).payload;
    for (case, planted) in intact_and_damaged(&state) {
        let env = Env::manual();
        drop(env.open());
        let path = env.path("docs/a/snapshot-0000000003.ysnap2");
        env.fs.put_durable(&path, &planted);

        let store = env.open();
        assert!(store.has_pending_structs(&doc("a")), "{case}");
        let (held, _) = applied(&store.load(&doc("a")).unwrap(), case);
        assert_eq!(held, ops_of_updates(&sent), "{case}");
    }
}

#[test]
fn compaction_quarantines_a_format_2_snapshot_a_load_found_damaged() {
    let sent = UpdateGen::new(72, 2).updates(10);
    for (case, planted) in intact_and_damaged(&state_of(&sent)) {
        let env = Env::manual();
        drop(env.open());
        let name = "snapshot-0000000003.ysnap2";
        env.fs.put_durable(&env.path("docs/a").join(name), &planted);

        let store = env.open();
        assert_doc(&store, &doc("a"), &sent);
        store.compact(&doc("a")).unwrap();
        assert_eq!(
            env.doc_files(&doc("a")),
            ["snapshot-0000000004.ysnap"],
            "{case}"
        );
        let quarantined = (case == "first copy zeroed")
            .then(|| (env.path("quarantine/a").join(name), planted.clone()));
        assert_eq!(
            env.quarantine(),
            quarantined.into_iter().collect(),
            "{case}"
        );
        drop(store);
        assert_doc(&env.open(), &doc("a"), &sent);
    }
}

#[test]
fn a_folder_mixing_snapshot_formats_loads_both_and_compacts_above_both() {
    let env = Env::manual();
    drop(env.open());
    let a = doc("a");
    let older = UpdateGen::new(61, 1).updates(5);
    let newer = UpdateGen::new(62, 1).updates(5);
    let later = UpdateGen::new(63, 1).updates(1);
    env.fs.put_durable(
        &env.path("docs/a/snapshot-0000000002.ysnap"),
        &encode_snapshot(&state_of(&older)),
    );
    env.fs.put_durable(
        &env.path("docs/a/snapshot-0000000003.ysnap2"),
        &encode_snapshot_v2(&state_of(&newer)),
    );
    let all = [older, newer, later.clone()].concat();

    let store = env.open();
    store.append(&a, &later[0]).unwrap();
    assert_eq!(store.flush(soon()).docs, []);
    assert_eq!(
        env.doc_files(&a),
        [
            "log-0000000003-0000000000.ylog2",
            "snapshot-0000000002.ysnap",
            "snapshot-0000000003.ysnap2",
        ]
    );
    assert_doc(&store, &a, &all);
    let (_, compacted) = store
        .run_compactor_once()
        .expect("the older snapshot queues a compaction");
    compacted.unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000004.ysnap2"]);
    assert_eq!(env.quarantine(), [].into());
    drop(store);
    assert_doc(&env.open(), &a, &all);
}

#[test]
fn compaction_deletes_a_whole_format_2_snapshot_it_holds_and_quarantines_a_damaged_one() {
    let sent = UpdateGen::new(73, 2).updates(10);
    for (case, planted) in intact_and_damaged(&state_of(&sent[..5])) {
        let env = Env::manual();
        let store = env.open();
        let a = doc("a");
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert_eq!(store.flush(soon()).docs, []);
        let name = "snapshot-0000000000.ysnap2";
        env.fs.put_durable(&env.path("docs/a").join(name), &planted);

        store.compact(&a).unwrap();
        assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap2"], "{case}");
        let quarantined = (case == "first copy zeroed")
            .then(|| (env.path("quarantine/a").join(name), planted.clone()));
        assert_eq!(
            env.quarantine(),
            quarantined.into_iter().collect(),
            "{case}"
        );
        drop(store);
        assert_doc(&env.open(), &a, &sent);
    }
}

#[test]
fn compaction_keeps_a_format_2_snapshot_holding_changes_it_lacks() {
    let sent = UpdateGen::new(74, 2).updates(10);
    for (case, planted) in intact_and_damaged(&state_of(&sent)) {
        let env = Env::manual();
        let store = env.open();
        let a = doc("a");
        for update in &sent[..5] {
            store.append(&a, update).unwrap();
        }
        assert_eq!(store.flush(soon()).docs, []);
        let path = env.path("docs/a/snapshot-0000000000.ysnap2");
        env.fs.put_durable(&path, &planted);

        store.compact(&a).unwrap();
        assert_eq!(
            env.doc_files(&a),
            ["snapshot-0000000000.ysnap2", "snapshot-0000000001.ysnap2"],
            "{case}: snapshot 1 lacks updates 6 to 10"
        );
        assert_eq!(env.fs.files().get(&path), Some(&planted), "{case}");
        drop(store);
        assert_doc(&env.open(), &a, &sent);
    }
}

#[test]
fn recovery_rejects_a_forged_tail_after_a_block_aligned_cut() {
    let (payload, cut) = snapshot_with_forged_tail();
    let env = Env::manual();
    drop(env.open());
    let path = env.path("docs/a/snapshot-0000000003.ysnap2");
    env.fs.put_durable(&path, &cut);
    env.fs.put_durable(
        &env.path("docs/b/snapshot-0000000003.ysnap2"),
        &encode_snapshot_v2(&payload),
    );

    let store = env.open();
    assert_doc(&store, &doc("b"), &[payload]);
    let loaded = store.load(&doc("a")).unwrap();
    assert_eq!(loaded.len(), 2, "recovery must load an empty doc");
    assert_eq!(loaded, [0, 0]);
    assert_eq!(env.fs.files().get(&path), Some(&cut));
}

#[test]
fn a_damaged_newest_snapshot_queues_compaction_and_keeps_its_bytes() {
    let sent = UpdateGen::new(75, 2).updates(10);
    let state = state_of(&sent);
    let intact = encode_snapshot_v2(&state);
    let mut damaged_head = intact.clone();
    damaged_head[..32].fill(0);
    let mut damaged_tail = intact.clone();
    let tail_at = damaged_tail.len() - 32;
    damaged_tail[tail_at..].fill(0);
    for (case, planted, damaged) in [
        ("intact", intact, false),
        ("head zeroed", damaged_head, true),
        ("first copy zeroed", with_first_copy_zeroed(&state), true),
        ("tail zeroed", damaged_tail, true),
    ] {
        let env = Env::manual();
        drop(env.open());
        let a = doc("a");
        let name = "snapshot-0000000003.ysnap2";
        let path = env.path("docs/a").join(name);
        env.fs.put_durable(&path, &planted);

        let store = env.open();
        assert_doc(&store, &a, &sent);
        if !damaged {
            assert!(store.run_compactor_once().is_none(), "{case}");
            assert_eq!(env.doc_files(&a), [name]);
            assert_eq!(env.fs.files().get(&path), Some(&planted));
            assert_eq!(env.quarantine(), [].into());
            continue;
        }
        let (compacted_doc, result) = store
            .run_compactor_once()
            .unwrap_or_else(|| panic!("{case}: recovery must queue compaction without any logs"));
        assert_eq!(compacted_doc, a, "{case}");
        result.unwrap();
        assert_eq!(env.doc_files(&a), ["snapshot-0000000004.ysnap"], "{case}");
        let quarantine = env.path("quarantine/a").join(name);
        assert_eq!(
            env.quarantine(),
            [(quarantine.clone(), planted.clone())].into(),
            "{case}"
        );
        assert_eq!(
            env.fs
                .ops()
                .iter()
                .filter(|op| {
                    op.kind == OpKind::Remove && (op.path == path || op.path == quarantine)
                })
                .count(),
            0,
            "{case}: damaged bytes must never be deleted"
        );
        drop(store);
        env.fs.crash(Crash::DropUnsynced);
        assert_doc(&env.open(), &a, &sent);
        assert_eq!(env.doc_files(&a), ["snapshot-0000000004.ysnap"], "{case}");
        assert_eq!(env.quarantine(), [(quarantine, planted)].into(), "{case}");
    }
}

#[test]
fn one_fault_in_a_compactor_written_format_2_snapshot_loses_no_update() {
    let sent = UpdateGen::new(76, 2).updates(10);
    let a = doc("a");
    let name = "snapshot-0000000001.ysnap2";
    let written_by_the_compactor = |env: &Env| {
        let store = env.open();
        for update in &sent {
            store.append(&a, update).unwrap();
        }
        assert_eq!(store.flush(soon()).docs, []);
        store.compact(&a).unwrap();
        assert_eq!(env.doc_files(&a), [name]);
        env.fs.files()[&env.path("docs/a").join(name)].clone()
    };
    let written = written_by_the_compactor(&Env::manual());
    let state = state_of(&sent);
    let (len, eof) = (state.len(), written.len());
    assert!(len < 4_064, "copy A shares block 0 with the head");
    assert_eq!(
        (&written[32..32 + len], &written[eof - 32 - len..eof - 32]),
        (&state[..], &state[..]),
        "the compactor writes the doc's state as both copies"
    );
    let flipped = |at: usize| {
        let mut bytes = written.clone();
        bytes[at] ^= 0x10;
        bytes
    };
    let zeroed = |range: Range<usize>| {
        let mut bytes = written.clone();
        bytes[range].fill(0);
        bytes
    };
    let copy_b = eof - 32 - len;
    for (case, damaged) in [
        ("a bit in the head", flipped(12)),
        ("a bit in copy A", flipped(32 + len / 2)),
        ("a bit in copy B", flipped(copy_b + len / 2)),
        ("a bit in the tail", flipped(eof - 12)),
        ("the block holding copy A zeroed", zeroed(0..4_096)),
        ("the block holding copy B zeroed", zeroed(copy_b..eof)),
    ] {
        let env = Env::manual();
        assert_eq!(written_by_the_compactor(&env), written, "{case}");
        let path = env.path("docs/a").join(name);
        env.fs.put_durable(&path, &damaged);

        let store = env.open();
        assert_doc(&store, &a, &sent);
        let (compacted, result) = store
            .run_compactor_once()
            .unwrap_or_else(|| panic!("{case}: a damaged newest snapshot queues compaction"));
        assert_eq!(compacted, a, "{case}");
        result.unwrap();
        assert_eq!(
            env.quarantine(),
            [(env.path("quarantine/a").join(name), damaged)].into(),
            "{case}"
        );
        drop(store);
        env.fs.crash(Crash::DropUnsynced);
        assert_doc(&env.open(), &a, &sent);
    }
}
