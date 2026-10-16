use super::{Env, soon};
use crate::storage::sim_fs::{Crash, OpKind};
use crate::storage::{DocId, Health};
use crate::workspace::{PreparedWorkspace, Workspace};

fn prepare(env: &Env, index: &std::path::Path) -> PreparedWorkspace {
    Workspace::prepare_with_store(env.open(), index.join("index.sqlite"))
}

#[test]
fn a_failed_first_body_flush_keeps_the_workspace_open_unseeded_and_retries_on_relaunch() {
    let env = Env::manual();
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = prepare(&env, dir.path());
    env.fs.fail(OpKind::Append, "getting-started", libc::ENOSPC);
    prepared.seed_getting_started(soon());
    let w = prepared.start(|_| {});
    assert_eq!(w.tree().unwrap().pages, []);
    assert_eq!(
        w.health(),
        Health::Failing {
            pending_settings: false,
            reason: crate::storage::FailReason::DiskFull,
            since: std::time::Duration::ZERO,
            pending_docs: vec![DocId::new("getting-started").unwrap()],
        }
    );
    drop(w);
    env.fs.crash(Crash::DropUnsynced);
    env.fs.clear_faults();
    let mut prepared = prepare(&env, dir.path());
    prepared.seed_getting_started(soon());
    let w = prepared.start(|_| {});
    assert_eq!(w.tree().unwrap().pages[0].title, "Getting started");
    assert_eq!(w.flush(soon()), crate::storage::Pending::default());
}

#[test]
fn an_unreadable_workspace_log_skips_seed_without_refusing_the_workspace() {
    let env = Env::manual();
    let store = env.open();
    store.append(&DocId::workspace(), &[0, 0]).unwrap();
    assert_eq!(store.flush(soon()), crate::storage::Pending::default());
    drop(store);
    env.fs.fail(OpKind::Read, "docs/workspace/", libc::EACCES);
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = prepare(&env, dir.path());
    prepared.seed_getting_started(soon());
    let w = prepared.start(|_| {});
    assert!(w.tree().is_err());
    w.append(&DocId::new("healthy-page").unwrap(), &[0, 0])
        .unwrap();
    assert_eq!(w.flush(soon()), crate::storage::Pending::default());
    env.fs.clear_faults();
    assert_eq!(w.tree().unwrap().pages, []);
}

#[test]
fn corrupt_workspace_history_and_quarantine_never_seed_but_a_torn_first_write_retries() {
    use crate::storage::log::{self, LogEnd, LogFormat};
    for kind in ["corrupt", "quarantine", "torn"] {
        let dir = tempfile::tempdir().unwrap();
        drop(crate::storage::Store::open(dir.path()).unwrap());
        let mut frame = Vec::new();
        log::encode_frame(&[0, 0], &mut frame);
        if kind == "quarantine" {
            let path = dir.path().join("quarantine/workspace");
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("damaged.ylog2"), &frame).unwrap();
        } else {
            if kind == "corrupt" {
                frame[20] ^= 1;
            } else {
                frame.truncate(21);
            }
            let contents = log::read(LogFormat::V2, &frame);
            assert_eq!(contents.records, Vec::<Vec<u8>>::new());
            assert_eq!(
                contents.end,
                if kind == "corrupt" {
                    LogEnd::Corrupt
                } else {
                    LogEnd::Torn
                }
            );
            let path = dir.path().join("docs/workspace");
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("log-0000000000-0000000000.ylog2"), frame).unwrap();
        }
        let mut prepared = Workspace::prepare(dir.path()).unwrap();
        prepared.seed_getting_started(soon());
        let w = prepared.start(|_| {});
        if kind == "torn" {
            assert_eq!(w.tree().unwrap().pages[0].id.as_str(), "getting-started");
        } else {
            assert_eq!(w.tree().unwrap().pages, [], "{kind}");
        }
    }
}

#[test]
fn a_slow_first_flush_opens_unseeded_and_a_later_launch_reuses_the_body() {
    use std::time::{Duration, Instant};
    let env = Env::threaded();
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = prepare(&env, dir.path());
    env.fs
        .set_latency(OpKind::FullSync, Duration::from_millis(50));
    prepared.seed_getting_started(Instant::now() + Duration::from_millis(5));
    let w = prepared.start(|_| {});
    assert_eq!(w.tree().unwrap().pages, []);
    assert_eq!(w.flush(soon()), crate::storage::Pending::default());
    let body = w.load(&DocId::new("getting-started").unwrap()).unwrap();
    drop(w);
    env.fs.set_latency(OpKind::FullSync, Duration::ZERO);
    let mut prepared = prepare(&env, dir.path());
    prepared.seed_getting_started(soon());
    let w = prepared.start(|_| {});
    assert_eq!(w.tree().unwrap().pages[0].title, "Getting started");
    assert_eq!(
        w.load(&DocId::new("getting-started").unwrap()).unwrap(),
        body
    );
}

#[test]
fn a_large_first_log_record_or_corrupt_prefix_is_history_but_a_large_torn_first_write_retries() {
    use crate::storage::log::{self, LogEnd, LogFormat};
    use yrs::{Doc, ReadTxn, Text, Transact};
    let doc = Doc::with_client_id(200);
    doc.get_or_insert_text("future")
        .insert(&mut doc.transact_mut(), 0, &"a".repeat(6000));
    let update = doc
        .transact()
        .encode_state_as_update_v1(&Default::default());
    for kind in ["whole", "corrupt", "torn"] {
        let dir = tempfile::tempdir().unwrap();
        drop(crate::storage::Store::open(dir.path()).unwrap());
        let mut frame = Vec::new();
        log::encode_frame(&update, &mut frame);
        if kind == "corrupt" {
            frame[30] ^= 1;
        }
        if kind == "torn" {
            frame.truncate(5000);
        }
        let contents = log::read(LogFormat::V2, &frame);
        if kind == "torn" {
            assert_eq!(contents.end, LogEnd::Torn);
            assert_eq!(contents.records, Vec::<Vec<u8>>::new());
        }
        let path = dir.path().join("docs/workspace");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("log-0000000000-0000000000.ylog2"), frame).unwrap();
        let mut prepared = Workspace::prepare(dir.path()).unwrap();
        prepared.seed_getting_started(soon());
        let w = prepared.start(|_| {});
        if kind == "torn" {
            assert_eq!(w.tree().unwrap().pages[0].title, "Getting started");
        } else {
            assert_eq!(w.tree().unwrap().pages, [], "{kind}");
        }
    }
}

#[test]
fn an_unreadable_torn_first_log_defers_the_seed_until_the_next_launch() {
    let env = Env::manual();
    drop(env.open());
    let mut frame = Vec::new();
    super::super::log::encode_frame(&[0, 0], &mut frame);
    env.fs.put_durable(
        &env.path("docs/workspace/log-0000000000-0000000000.ylog2"),
        &frame[..21],
    );
    env.fs.fail(OpKind::Read, "docs/workspace/", libc::EIO);
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = prepare(&env, dir.path());
    prepared.seed_getting_started(soon());
    let w = prepared.start(|_| {});
    assert!(w.tree().is_err());
    assert_eq!(
        env.doc_files(&DocId::new("getting-started").unwrap()),
        Vec::<String>::new()
    );
    drop(w);
    env.fs.clear_faults();
    let mut prepared = prepare(&env, dir.path());
    prepared.seed_getting_started(soon());
    let w = prepared.start(|_| {});
    assert_eq!(w.tree().unwrap().pages[0].title, "Getting started");
}

#[test]
fn the_cold_start_empty_fixture_has_one_stored_empty_update_and_never_seeds() {
    use crate::storage::log::{self, LogContents, LogEnd, LogFormat};
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../perf/fixtures/empty-workspace");
    let file = "docs/workspace/log-0000000000-0000000000.ylog2";
    let bytes = std::fs::read(fixture.join(file)).unwrap();
    assert_eq!(
        log::read(LogFormat::V2, &bytes),
        LogContents {
            records: vec![vec![0, 0]],
            end: LogEnd::Clean
        }
    );
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs/workspace")).unwrap();
    std::fs::copy(fixture.join("format.json"), dir.path().join("format.json")).unwrap();
    std::fs::write(dir.path().join(file), bytes).unwrap();
    let mut prepared = Workspace::prepare(dir.path()).unwrap();
    prepared.seed_getting_started(soon());
    let w = prepared.start(|_| {});
    assert_eq!(w.load(&DocId::workspace()).unwrap(), [0, 0]);
    assert_eq!(w.tree().unwrap().pages, []);
    assert!(!dir.path().join("docs/getting-started").exists());
}
