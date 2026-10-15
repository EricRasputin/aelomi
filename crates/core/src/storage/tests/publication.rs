use std::io;

use super::{Env, assert_doc, doc, soon};
use crate::storage::health::{FailReason, Health};
use crate::storage::log::encode_segment;
use crate::storage::sim_fs::{Crash, OpKind};
use crate::storage::snapshot::{self, Condition, SnapshotFormat};
use crate::storage::testkit::{UpdateGen, doc_from, fingerprint};

const MARKER_1: &str = "{\n  \"format\": 1\n}\n";
const MARKER_2: &str = "{\n  \"format\": 2\n}\n";

fn markers(env: &Env) -> (String, String) {
    let marker = env.path("format.json");
    let live = env.fs.files().remove(&marker).unwrap();
    let durable = env.fs.durable_files().remove(&marker).unwrap();
    (
        String::from_utf8(live).unwrap(),
        String::from_utf8(durable).unwrap(),
    )
}

fn pair(live: &str, durable: &str) -> (String, String) {
    (live.into(), durable.into())
}

fn has_tmp(env: &Env) -> bool {
    env.fs.files().contains_key(&env.path("format.json.tmp"))
}

fn ops_since(env: &Env, before: usize) -> Vec<String> {
    let ops = env.fs.ops()[before..].to_vec();
    ops.iter()
        .map(|op| {
            let failed = if op.ok { "" } else { " failed" };
            format!("{:?} {}{failed}", op.kind, op.path.display())
        })
        .collect()
}

const PUBLISH: [&str; 7] = [
    "Create /data/format.json.tmp",
    "Append /data/format.json.tmp",
    "Fsync /data/format.json.tmp",
    "FullSync /data/format.json.tmp",
    "ReplaceFormat /data/format.json.tmp",
    "Fsync /data",
    "FullSync /data",
];

const CONFIRM: [&str; 3] = ["Fsync /data/format.json", "Fsync /data", "FullSync /data"];

const REPLACE_TMP: [&str; 2] = [
    "Create /data/format.json.tmp failed",
    "Remove /data/format.json.tmp",
];

fn first_segment(log: &str) -> Vec<String> {
    let log = format!("/data/docs/a/{log}");
    vec![
        "CreateDir /data/docs/a".into(),
        format!("Create {log}"),
        format!("Append {log}"),
        format!("Fsync {log}"),
        "Fsync /data/docs/a".into(),
        "Fsync /data/docs".into(),
        format!("FullSync {log}"),
    ]
}

fn strings(ops: &[&str]) -> Vec<String> {
    ops.iter().map(|op| (*op).to_owned()).collect()
}

#[test]
fn opening_leaves_format_1_and_the_first_segment_publishes_format_2() {
    let env = Env::manual();
    let a = doc("a");
    let update = UpdateGen::new(1, 2).next_update();
    let store = env.open();
    assert_eq!(
        markers(&env),
        pair(MARKER_1, MARKER_1),
        "opening never bumps a folder"
    );

    let seq = store.append(&a, &update).unwrap();
    let before = env.fs.ops().len();
    store.run_writer_once();
    assert!(store.is_durable(&a, seq));
    assert_eq!(
        ops_since(&env, before),
        [
            strings(&PUBLISH),
            first_segment("log-0000000000-0000000000.ylog2")
        ]
        .concat()
    );
    assert_eq!(
        (markers(&env), env.doc_files(&a)),
        (
            pair(MARKER_2, MARKER_2),
            vec!["log-0000000000-0000000000.ylog2".to_owned()]
        )
    );
    drop(store);
    assert_doc(&env.open(), &a, &[update]);
}

#[test]
fn a_marker_whose_folder_sync_failed_after_the_rename_is_confirmed_again_on_retry() {
    let env = Env::manual();
    let a = doc("a");
    let update = UpdateGen::new(2, 2).next_update();
    let store = env.open();
    env.fs.fail_at(OpKind::FullSync, env.root(), libc::EIO);
    let seq = store.append(&a, &update).unwrap();
    let before = env.fs.ops().len();
    store.run_writer_once();
    assert!(!store.is_durable(&a, seq), "the batch fails with the sync");
    let mut failed = strings(&PUBLISH);
    failed[6] = "FullSync /data failed".into();
    assert_eq!(ops_since(&env, before), failed);
    let Health::Failing {
        reason,
        pending_docs,
        ..
    } = store.health()
    else {
        panic!("writes are failing")
    };
    let eio = io::Error::from_raw_os_error(libc::EIO).to_string();
    assert_eq!(
        (reason, pending_docs, markers(&env), env.doc_files(&a)),
        (
            FailReason::Io(eio),
            vec![a.clone()],
            pair(MARKER_2, MARKER_1),
            vec![]
        )
    );

    env.fs.clear_faults();
    let before = env.fs.ops().len();
    assert_eq!(store.flush(soon()).docs, []);
    assert_eq!(
        ops_since(&env, before),
        [
            strings(&CONFIRM),
            first_segment("log-0000000000-0000000001.ylog2")
        ]
        .concat(),
        "the retry confirms the renamed marker before creating a .ylog2"
    );
    assert_eq!(
        (store.health(), markers(&env), env.doc_files(&a)),
        (
            Health::Healthy,
            pair(MARKER_2, MARKER_2),
            vec!["log-0000000000-0000000001.ylog2".to_owned()]
        )
    );
    drop(store);
    assert_doc(&env.open(), &a, &[update]);
}

#[test]
fn a_format_2_marker_left_in_the_drive_cache_by_an_exit_is_confirmed_by_the_next_session() {
    let env = Env::manual();
    let a = doc("a");
    let mut updates = UpdateGen::new(3, 2);
    let store = env.open();
    env.fs.fail_at(OpKind::FullSync, env.root(), libc::EIO);
    let lost = store.append(&a, &updates.next_update()).unwrap();
    store.run_writer_once();
    assert!(!store.is_durable(&a, lost), "the batch fails with the sync");
    drop(store);
    env.fs.clear_faults();

    let store = env.open();
    assert_eq!(
        markers(&env),
        pair(MARKER_2, MARKER_1),
        "the process exited, so the drive cache still holds the rename"
    );
    let update = updates.next_update();
    let seq = store.append(&a, &update).unwrap();
    let before = env.fs.ops().len();
    assert_eq!(store.flush(soon()).docs, []);
    assert!(store.is_durable(&a, seq));
    assert_eq!(
        ops_since(&env, before),
        [
            strings(&CONFIRM),
            first_segment("log-0000000000-0000000000.ylog2")
        ]
        .concat(),
        "each session confirms its marker before its first .ylog2"
    );
    assert_eq!(
        (markers(&env), env.doc_files(&a)),
        (
            pair(MARKER_2, MARKER_2),
            vec!["log-0000000000-0000000000.ylog2".to_owned()]
        )
    );
    drop(store);
    assert_doc(&env.open(), &a, &[update]);
}

#[test]
fn a_stale_format_tmp_beside_a_format_1_marker_is_replaced_by_the_first_segment() {
    for stale in [&b"{\"for"[..], b"{\"format\":2}", b"\xff\0 not json"] {
        let case = String::from_utf8_lossy(stale);
        let env = Env::manual();
        env.fs
            .put_durable(&env.path("format.json"), MARKER_1.as_bytes());
        env.fs.put_durable(&env.path("format.json.tmp"), stale);
        let a = doc("a");
        let update = UpdateGen::new(4, 2).next_update();
        let store = env.open();
        assert_eq!(
            env.fs.files()[&env.path("format.json.tmp")],
            stale,
            "opening leaves a tmp beside a marker"
        );

        let seq = store.append(&a, &update).unwrap();
        let before = env.fs.ops().len();
        store.run_writer_once();
        assert!(store.is_durable(&a, seq), "{case}");
        assert_eq!(
            ops_since(&env, before),
            [
                strings(&REPLACE_TMP),
                strings(&PUBLISH),
                first_segment("log-0000000000-0000000000.ylog2"),
            ]
            .concat(),
            "{case}"
        );
        assert_eq!(
            (markers(&env), has_tmp(&env)),
            (pair(MARKER_2, MARKER_2), false),
            "{case}"
        );
        drop(store);
        assert_doc(&env.open(), &a, &[update]);
    }
}

#[test]
fn a_format_tmp_left_by_a_failed_append_is_replaced_when_the_write_retries() {
    let env = Env::manual();
    let a = doc("a");
    let update = UpdateGen::new(5, 2).next_update();
    let store = env.open();
    env.fs.fail(OpKind::Append, "format.json.tmp", libc::EIO);
    let seq = store.append(&a, &update).unwrap();
    let before = env.fs.ops().len();
    store.run_writer_once();
    assert!(
        !store.is_durable(&a, seq),
        "the batch fails with the append"
    );
    assert_eq!(
        ops_since(&env, before),
        [
            "Create /data/format.json.tmp",
            "Append /data/format.json.tmp failed"
        ]
    );
    assert_eq!(
        (
            env.fs.files().remove(&env.path("format.json.tmp")),
            markers(&env)
        ),
        (Some(b"{\n  \"form".to_vec()), pair(MARKER_1, MARKER_1)),
        "the failed append left half a marker"
    );

    env.fs.clear_faults();
    let before = env.fs.ops().len();
    assert_eq!(store.flush(soon()).docs, []);
    assert_eq!(
        ops_since(&env, before),
        [
            strings(&REPLACE_TMP),
            strings(&PUBLISH),
            first_segment("log-0000000000-0000000001.ylog2"),
        ]
        .concat()
    );
    assert_eq!(
        (markers(&env), has_tmp(&env)),
        (pair(MARKER_2, MARKER_2), false)
    );
    drop(store);
    assert_doc(&env.open(), &a, &[update]);
}

fn compaction(snapshot: &str, older: &str) -> Vec<String> {
    let dir = "/data/docs/a";
    let tmp = format!("{dir}/{snapshot}.tmp");
    vec![
        format!("Create {tmp}"),
        format!("Append {tmp}"),
        format!("Fsync {tmp}"),
        format!("FullSync {tmp}"),
        format!("Read {tmp}"),
        format!("Rename {tmp}"),
        format!("Fsync {dir}"),
        format!("FullSync {dir}"),
        format!("List {dir}"),
        format!("Read {dir}/{older}"),
        format!("Remove {dir}/{older}"),
        format!("Fsync {dir}"),
        format!("FullSync {dir}"),
    ]
}

fn whole_format_2_snapshot(env: &Env, name: &str) -> Vec<u8> {
    let bytes = &env.fs.files()[&env.path("docs/a").join(name)];
    match snapshot::read(SnapshotFormat::V2, bytes) {
        Some((payload, Condition::Whole)) => payload.to_vec(),
        read => panic!("{name} is not a whole .ysnap2: {read:?}"),
    }
}

#[test]
fn compaction_after_a_lifetimes_first_segment_writes_a_whole_format_2_snapshot() {
    let env = Env::manual();
    let a = doc("a");
    let sent = UpdateGen::new(6, 2).updates(10);
    let store = env.open();
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()).docs, []);
    let before = env.fs.ops().len();
    store.compact(&a).unwrap();
    assert_eq!(
        ops_since(&env, before),
        compaction(
            "snapshot-0000000001.ysnap2",
            "log-0000000000-0000000000.ylog2"
        )
    );
    assert_eq!(
        (env.doc_files(&a), env.quarantine()),
        (vec!["snapshot-0000000001.ysnap2".to_owned()], [].into())
    );
    let payload = whole_format_2_snapshot(&env, "snapshot-0000000001.ysnap2");
    assert_eq!(
        fingerprint(&doc_from([payload.as_slice()])),
        fingerprint(&doc_from(sent.iter().map(Vec::as_slice)))
    );
    drop(store);
    env.fs.crash(Crash::DropUnsynced);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn compaction_before_a_lifetimes_first_segment_writes_format_1_in_a_format_2_folder() {
    let mut env = Env::manual();
    env.limits.records = 5;
    let a = doc("a");
    let mut updates = UpdateGen::new(7, 2);
    let sent = updates.updates(10);
    let store = env.open();
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()).docs, []);
    drop(store);
    assert_eq!(markers(&env), pair(MARKER_2, MARKER_2));

    let store = env.open();
    assert_doc(&store, &a, &sent);
    let before = env.fs.ops().len();
    let (compacted, result) = store
        .run_compactor_once()
        .expect("logs over the limit queue a compaction at load");
    result.unwrap();
    assert_eq!(
        (compacted, ops_since(&env, before)),
        (
            a.clone(),
            compaction(
                "snapshot-0000000001.ysnap",
                "log-0000000000-0000000000.ylog2"
            )
        ),
        "the gate isn't confirmed this lifetime, and compaction never confirms it"
    );
    assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"]);

    let later = updates.next_update();
    store.append(&a, &later).unwrap();
    assert_eq!(store.flush(soon()).docs, []);
    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), ["snapshot-0000000002.ysnap2"]);
    whole_format_2_snapshot(&env, "snapshot-0000000002.ysnap2");
    drop(store);
    assert_doc(&env.open(), &a, &[sent, vec![later]].concat());
}

#[test]
fn compaction_in_a_format_1_folder_writes_format_1_and_leaves_the_marker() {
    let env = Env::manual();
    drop(env.open());
    let a = doc("a");
    let sent = UpdateGen::new(8, 2).updates(10);
    env.fs.put_durable(
        &env.path("docs/a/log-0000000000-0000000000.ylog"),
        &encode_segment(&sent),
    );

    let store = env.open();
    assert_doc(&store, &a, &sent);
    let before = env.fs.ops().len();
    store.compact(&a).unwrap();
    assert_eq!(
        ops_since(&env, before),
        compaction(
            "snapshot-0000000001.ysnap",
            "log-0000000000-0000000000.ylog"
        )
    );
    assert_eq!(
        (markers(&env), env.doc_files(&a)),
        (
            pair(MARKER_1, MARKER_1),
            vec!["snapshot-0000000001.ysnap".to_owned()]
        )
    );
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}
