use yrs::{ReadTxn, StateVector, Transact};

use super::{Delivery, Env, applied, assert_doc, doc, ops_of_updates, out_of_order, soon};
use crate::storage::recover::merge;
use crate::storage::snapshot::{encode_snapshot, encode_snapshot_v2};
use crate::storage::testkit::{UpdateGen, doc_from};

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
    assert_eq!(store.flush(soon()), []);
    assert_eq!(
        env.doc_files(&a),
        [
            "log-0000000003-0000000000.ylog",
            "snapshot-0000000002.ysnap",
            "snapshot-0000000003.ysnap2",
        ]
    );
    assert_doc(&store, &a, &all);
    let (_, compacted) = store
        .run_compactor_once()
        .expect("the older snapshot queues a compaction");
    compacted.unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000004.ysnap"]);
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
        assert_eq!(store.flush(soon()), []);
        let name = "snapshot-0000000000.ysnap2";
        env.fs.put_durable(&env.path("docs/a").join(name), &planted);

        store.compact(&a).unwrap();
        assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"], "{case}");
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
        assert_eq!(store.flush(soon()), []);
        let path = env.path("docs/a/snapshot-0000000000.ysnap2");
        env.fs.put_durable(&path, &planted);

        store.compact(&a).unwrap();
        assert_eq!(
            env.doc_files(&a),
            ["snapshot-0000000000.ysnap2", "snapshot-0000000001.ysnap"],
            "{case}: snapshot 1 lacks updates 6 to 10"
        );
        assert_eq!(env.fs.files().get(&path), Some(&planted), "{case}");
        drop(store);
        assert_doc(&env.open(), &a, &sent);
    }
}
