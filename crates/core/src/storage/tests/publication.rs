use std::io;

use super::{Env, assert_doc, doc, soon};
use crate::storage::health::{FailReason, Health};
use crate::storage::sim_fs::OpKind;
use crate::storage::testkit::UpdateGen;

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
    assert_eq!(store.flush(soon()), []);
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
    assert_eq!(store.flush(soon()), []);
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
    assert_eq!(store.flush(soon()), []);
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
