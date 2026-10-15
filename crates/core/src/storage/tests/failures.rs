use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use super::{
    Delivery, Env, applied, assert_doc, doc, frames, log_name, ops_of_updates, out_of_order,
    read_log_file, soon, typed, v1_snapshot_name, v2_snapshot_name,
};
use crate::storage::doc_id::DocId;
use crate::storage::health::{FailReason, Health};
use crate::storage::log::LogEnd;
use crate::storage::sim_fs::{Crash, OpKind};
use crate::storage::store::Store;
use crate::storage::testkit::UpdateGen;
use crate::storage::writer::Step;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn write_batch(store: &Store) {
    let step = store.run_writer_once();
    assert!(
        matches!(step, Step::Wrote),
        "expected a batch, got {step:?}"
    );
}

fn record_health(store: &Store) -> Arc<Mutex<Vec<Health>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    store.subscribe_health(move |health| sink.lock().unwrap().push(health.clone()));
    seen
}

#[test]
fn a_failing_doc_stays_pending_while_other_docs_are_acknowledged() {
    let env = Env::manual();
    let store = env.open();
    let seen = record_health(&store);
    let (a, b) = (doc("a"), doc("b"));
    let (mut gen_a, mut gen_b) = (UpdateGen::new(1, 2), UpdateGen::new(2, 2));
    let mut sent_a = gen_a.updates(3);
    let mut sent_b = Vec::new();
    env.fs.fail(OpKind::Append, "/docs/a/", libc::ENOSPC);
    env.clock.advance(ms(1000));

    let a_seqs: Vec<_> = sent_a
        .iter()
        .map(|u| store.append(&a, u).unwrap())
        .collect();
    let a_last = a_seqs[a_seqs.len() - 1];
    sent_b.push(gen_b.next_update());
    let b_first = store.append(&b, &sent_b[0]).unwrap();
    write_batch(&store);
    assert!(store.is_durable(&b, b_first), "b is acknowledged");
    assert!(!store.is_durable(&a, a_last), "a stays pending");
    let failing = Health::Failing {
        reason: FailReason::DiskFull,
        since: ms(1000),
        pending_docs: vec![a.clone()],
        pending_settings: false,
    };
    assert_eq!(store.health(), failing);

    sent_b.push(gen_b.next_update());
    let b_second = store.append(&b, &sent_b[1]).unwrap();
    write_batch(&store);
    assert!(
        store.is_durable(&b, b_second),
        "b keeps going while a waits"
    );
    assert!(!store.is_durable(&a, a_last));

    let mut retries = Vec::new();
    for _ in 0..4 {
        let Step::Idle { retry_at: Some(at) } = store.run_writer_once() else {
            panic!("a waits for its retry")
        };
        retries.push(at);
        env.clock.advance_to(at);
        write_batch(&store);
        assert!(!store.is_durable(&a, a_last), "retry at {at:?} fails too");
    }
    assert_eq!(
        retries,
        [ms(1100), ms(1300), ms(1700), ms(2500)],
        "100 ms, doubling"
    );
    assert_eq!(
        store.health(),
        failing,
        "still failing, since the first failure"
    );

    sent_a.push(gen_a.next_update());
    let a_more = store.append(&a, sent_a.last().unwrap()).unwrap();
    env.fs.clear_faults();
    let Step::Idle { retry_at: Some(at) } = store.run_writer_once() else {
        panic!("a waits for its retry")
    };
    assert_eq!(at, ms(4100));
    env.clock.advance_to(at);
    write_batch(&store);
    assert!(
        store.is_durable(&a, a_more),
        "a recovers with everything queued"
    );
    assert_eq!(store.health(), Health::Healthy);
    assert_eq!(*seen.lock().unwrap(), [failing, Health::Healthy]);

    assert_eq!(
        env.doc_files(&a),
        (0..=5).map(|n| log_name(0, n)).collect::<Vec<_>>(),
        "each attempt wrote a new segment"
    );
    drop(store);
    let store = env.open();
    assert_doc(&store, &a, &sent_a);
    assert_doc(&store, &b, &sent_b);
}

#[test]
fn queued_updates_reach_disk_in_order_once_writes_recover() {
    let env = Env::manual();
    let store = env.open();
    let seen = record_health(&store);
    let a = doc("a");
    let sent = UpdateGen::new(5, 3).updates(100);
    env.fs.fail(OpKind::Append, "/docs/a/", libc::EIO);

    let seqs: Vec<_> = sent.iter().map(|u| store.append(&a, u).unwrap()).collect();
    let last = seqs[seqs.len() - 1];
    write_batch(&store);
    assert!(!store.is_durable(&a, last));
    env.fs.clear_faults();
    env.clock.advance(ms(100));
    write_batch(&store);
    assert!(store.is_durable(&a, last));
    assert_eq!(store.health(), Health::Healthy);
    let health = seen.lock().unwrap().clone();
    assert!(matches!(
        &health[..],
        [
            Health::Failing {
                reason: FailReason::Io(_),
                ..
            },
            Health::Healthy
        ]
    ));

    let segments: Vec<Vec<Vec<u8>>> = env
        .doc_files(&a)
        .iter()
        .map(|name| {
            let path = env.path("docs/a").join(name);
            read_log_file(&path, &env.fs.files()[&path]).records
        })
        .collect();
    assert_eq!(segments.len(), 2, "the failed segment, then the retry's");
    assert!(
        sent.starts_with(&segments[0]),
        "the failed write left a prefix at most"
    );
    assert!(segments[0].len() < sent.len());
    assert_eq!(segments[1], sent, "the retry wrote every update, in order");
}

#[test]
fn a_failed_full_sync_leaves_the_whole_batch_pending() {
    let env = Env::manual();
    let store = env.open();
    let docs = [doc("a"), doc("b")];
    let mut updates = UpdateGen::new(9, 2);
    let mut sent = vec![Vec::new(), Vec::new()];
    let mut seqs = Vec::new();
    for (i, d) in docs.iter().enumerate() {
        sent[i].push(updates.next_update());
        seqs.push(store.append(d, &sent[i][0]).unwrap());
    }
    env.fs.fail(OpKind::FullSync, "/docs/", libc::EIO);
    write_batch(&store);
    for (d, seq) in docs.iter().zip(&seqs) {
        assert!(!store.is_durable(d, *seq), "{d} stays pending");
    }
    let Health::Failing { pending_docs, .. } = store.health() else {
        panic!("failing")
    };
    assert_eq!(pending_docs, docs);

    env.fs.clear_faults();
    env.clock.advance(ms(100));
    write_batch(&store);
    for (d, seq) in docs.iter().zip(&seqs) {
        assert!(store.is_durable(d, *seq), "{d} lands on retry");
    }
    assert_eq!(store.health(), Health::Healthy);
    drop(store);
    let store = env.open();
    for (d, sent) in docs.iter().zip(&sent) {
        assert_doc(&store, d, sent);
    }
}

#[test]
fn the_writer_thread_retries_on_its_own() {
    let env = Env::threaded();
    let store = env.open();
    let a = doc("a");
    let sent = UpdateGen::new(4, 2).updates(5);
    let (changed, health) = mpsc::channel();
    let changed = Mutex::new(changed);
    store.subscribe_health(move |h| drop(changed.lock().unwrap().send(h.clone())));
    env.fs.fail(OpKind::Append, "/docs/a/", libc::EROFS);
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    let Ok(Health::Failing { reason, .. }) = health.recv_timeout(Duration::from_secs(10)) else {
        panic!("health turns Failing")
    };
    assert_eq!(reason, FailReason::ReadOnlyVolume);
    env.fs.clear_faults();
    assert_eq!(
        health.recv_timeout(Duration::from_secs(10)),
        Ok(Health::Healthy),
        "the retry lands without new work"
    );
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn flush_retries_a_failing_doc_at_once_whatever_its_backoff() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let sent = UpdateGen::new(81, 2).updates(3);
    env.fs.fail(OpKind::Append, "/docs/a/", libc::EIO);
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    write_batch(&store);
    for _ in 0..6 {
        env.clock.advance(ms(5000));
        write_batch(&store);
    }
    assert!(matches!(
        store.run_writer_once(),
        Step::Idle { retry_at: Some(at) } if at == ms(35_000)
    ));

    let appends = || {
        let ops = env.fs.ops();
        ops.iter().filter(|op| op.kind == OpKind::Append).count()
    };
    let before = appends();
    let quit_deadline = || Instant::now() + Duration::from_secs(2);
    assert_eq!(store.flush(quit_deadline()), std::slice::from_ref(&a));
    assert_eq!(appends(), before + 1, "one retry per flush");
    env.fs.clear_faults();
    assert_eq!(store.flush(quit_deadline()), [], "the disk recovered");
    assert_eq!(store.health(), Health::Healthy);
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn a_flush_retries_a_failing_doc_whose_files_were_busy_as_soon_as_they_free() {
    let env = Env::manual();
    let b = doc("b");
    let sent = out_of_order(82, 40, Delivery::OneWithheld);
    let (last, stored) = sent.split_last().unwrap();
    let store = env.open();
    for update in stored {
        store.append(&b, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    assert!(store.has_pending_structs(&b), "b's load merges its files");
    env.fs.fail(OpKind::Append, "/docs/b/", libc::EIO);
    store.append(&b, last).unwrap();
    write_batch(&store);
    env.fs.clear_faults();

    let hold = env.fs.hold_once(OpKind::Read, "/docs/b/", Duration::ZERO);
    let pending = std::thread::scope(|s| {
        let loading = s.spawn(|| store.load(&b).unwrap());
        hold.wait_until_reached();
        let pending = store.flush(soon());
        hold.release();
        loading.join().unwrap();
        pending
    });
    assert_eq!(pending, std::slice::from_ref(&b), "b's files were busy");
    write_batch(&store);
    assert_eq!(store.health(), Health::Healthy, "within b's 100 ms backoff");
    drop(store);
    let (held, _) = applied(&env.open().load(&b).unwrap(), "b");
    assert_eq!(held, ops_of_updates(&sent));
}

#[test]
fn close_never_discards_a_pending_update() {
    let env = Env::manual();
    let a = doc("a");
    let sent = UpdateGen::new(71, 2).updates(3);
    let store = env.open();
    env.fs.fail(OpKind::Append, "/docs/a/", libc::EIO);
    for update in &sent {
        store.append(&a, update).unwrap();
    }
    let (store, pending) = store.close(soon()).unwrap_err();
    assert_eq!(
        pending,
        std::slice::from_ref(&a),
        "close hands the store back"
    );
    assert_doc(&store, &a, &sent);
    env.fs.clear_faults();
    env.clock.advance(ms(100));
    store.close(soon()).unwrap();
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn a_panic_holding_a_docs_state_stops_neither_its_writes_nor_any_others() {
    let env = Env::manual();
    let store = env.open();
    let (a, b) = (doc("a"), doc("b"));
    let mut gen_a = UpdateGen::new(93, 2);
    let mut sent_a = gen_a.updates(3);
    let sent_b = UpdateGen::new(94, 2).updates(3);
    let mut seqs = Vec::new();
    for (id, sent) in [(&a, &sent_a), (&b, &sent_b)] {
        for update in sent {
            seqs.push((id, store.append(id, update).unwrap()));
        }
    }

    store.panic_while_applying(&a, &UpdateGen::new(95, 1).next_update());
    write_batch(&store);
    assert!(seqs.iter().all(|(id, seq)| store.is_durable(id, *seq)));
    assert_eq!(store.health(), Health::Healthy);
    assert_doc(&store, &a, &sent_a);
    assert_doc(&store, &b, &sent_b);

    let more = gen_a.next_update();
    store.append(&a, &more).unwrap();
    assert_eq!(store.flush(soon()), []);
    sent_a.push(more);
    drop(store);
    let store = env.open();
    assert_doc(&store, &a, &sent_a);
    assert_doc(&store, &b, &sent_b);
}

#[test]
fn a_panicking_health_subscriber_stops_neither_writes_nor_other_subscribers() {
    let env = Env::manual();
    let store = env.open();
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    store.subscribe_health(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        panic!("a subscriber bug");
    });
    let seen = record_health(&store);
    let docs = [doc("a"), doc("b"), doc("c")];
    let sent: Vec<Vec<u8>> = (0..3)
        .map(|i| UpdateGen::new(97 + i, 1).next_update())
        .collect();
    env.fs.fail(OpKind::Append, "/docs/a/", libc::ENOSPC);
    env.clock.advance(ms(1000));
    let seqs: Vec<_> = docs
        .iter()
        .zip(&sent)
        .map(|(id, update)| store.append(id, update).unwrap())
        .collect();

    write_batch(&store);
    let durable = |store: &Store| -> Vec<bool> {
        docs.iter()
            .zip(&seqs)
            .map(|(id, seq)| store.is_durable(id, *seq))
            .collect()
    };
    assert_eq!(durable(&store), [false, true, true]);
    env.fs.clear_faults();
    env.clock.advance(ms(100));
    write_batch(&store);
    assert_eq!(durable(&store), [true, true, true]);

    let failing = Health::Failing {
        reason: FailReason::DiskFull,
        since: ms(1000),
        pending_docs: vec![doc("a")],
        pending_settings: false,
    };
    assert_eq!(*seen.lock().unwrap(), [failing, Health::Healthy]);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    drop(store);
    let store = env.open();
    for (id, update) in docs.iter().zip(&sent) {
        assert_doc(&store, id, std::slice::from_ref(update));
    }
}

#[test]
fn a_panic_in_a_writer_step_leaves_every_update_queued_for_the_retry() {
    let env = Env::manual();
    let store = env.open();
    let seen = record_health(&store);
    let (a, b) = (doc("a"), doc("b"));
    let sent: Vec<Vec<u8>> = (0..2)
        .map(|i| UpdateGen::new(98 + i, 1).next_update())
        .collect();
    env.clock.advance(ms(1000));
    env.fs.panic_once(OpKind::Append, "/docs/a/");
    let seq_a = store.append(&a, &sent[0]).unwrap();
    let seq_b = store.append(&b, &sent[1]).unwrap();

    let step = store.run_writer_once();
    assert!(
        matches!(step, Step::Idle { retry_at: Some(at) } if at == ms(1100)),
        "{step:?}"
    );
    assert!(!store.is_durable(&a, seq_a) && !store.is_durable(&b, seq_b));
    let failing = Health::Failing {
        reason: FailReason::Internal,
        since: ms(1000),
        pending_docs: vec![a.clone(), b.clone()],
        pending_settings: false,
    };
    assert_eq!(store.health(), failing);

    env.clock.advance(ms(100));
    write_batch(&store);
    assert!(store.is_durable(&a, seq_a) && store.is_durable(&b, seq_b));
    assert_eq!(*seen.lock().unwrap(), [failing, Health::Healthy]);
    assert_eq!(
        env.doc_files(&a),
        [log_name(0, 0), log_name(0, 1)],
        "the retry writes to a new segment"
    );
    drop(store);
    let store = env.open();
    assert_doc(&store, &a, &sent[..1]);
    assert_doc(&store, &b, &sent[1..]);
}

#[test]
fn the_writer_thread_survives_a_panic() {
    let env = Env::threaded();
    let store = env.open();
    let a = doc("a");
    let sent = UpdateGen::new(100, 1).updates(2);
    env.fs.panic_once(OpKind::Append, "/docs/a/");
    store.append(&a, &sent[0]).unwrap();
    assert_eq!(store.flush(Instant::now() + Duration::from_secs(5)), []);
    store.append(&a, &sent[1]).unwrap();
    assert_eq!(store.flush(soon()), []);
    assert_eq!(store.health(), Health::Healthy);
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

fn fail_then_recover(env: &Env, store: &Store, a: &DocId, sent: &[Vec<u8>], attempts: usize) {
    for update in &sent[..3] {
        store.append(a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    env.fs.fail(OpKind::Append, "/docs/a/", libc::EIO);
    for update in &sent[3..] {
        store.append(a, update).unwrap();
    }
    write_batch(store);
    for _ in 1..attempts {
        let Step::Idle { retry_at: Some(at) } = store.run_writer_once() else {
            panic!("a waits for its retry")
        };
        env.clock.advance_to(at);
        write_batch(store);
    }
    env.fs.clear_faults();
    assert_eq!(store.flush(soon()), []);
}

fn torn_logs(env: &Env, a: &DocId) -> Vec<(String, usize)> {
    let files = env.fs.files();
    env.doc_files(a)
        .into_iter()
        .filter_map(|name| {
            let path = env.path("docs/a").join(&name);
            let contents = read_log_file(&path, &files[&path]);
            (contents.end == LogEnd::Torn).then_some((name, contents.records.len()))
        })
        .collect()
}

#[test]
fn compaction_deletes_the_segments_this_sessions_failed_writes_left_instead_of_quarantining_them() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let sent = UpdateGen::new(17, 2).updates(13);
    fail_then_recover(&env, &store, &a, &sent, 21);
    let torn = torn_logs(&env, &a);
    let segment = |n: usize| log_name(0, n as u64);
    assert_eq!(
        torn.iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>(),
        (0..=20).map(segment).collect::<Vec<_>>(),
        "the first attempt and 20 retries each tore a segment"
    );
    assert_eq!(torn[0].1, 3 + 5, "the first holds 3 acknowledged records");
    assert!(
        torn[1..].iter().all(|(_, records)| *records == 5),
        "each retry wrote records before it failed: {torn:?}"
    );
    assert_eq!(env.doc_files(&a).len(), 22);

    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), [v2_snapshot_name(1)]);
    let quarantined: Vec<PathBuf> = env.quarantine().into_keys().collect();
    assert_eq!(quarantined, Vec::<PathBuf>::new());
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn a_segment_a_failed_write_tore_in_an_earlier_session_is_quarantined_not_deleted() {
    let a = doc("a");
    let sent = UpdateGen::new(17, 2).updates(13);
    let name = log_name(0, 0);
    let (env, torn) = (0..64)
        .find_map(|seed| {
            let env = Env::manual();
            let store = env.open();
            fail_then_recover(&env, &store, &a, &sent, 1);
            drop(store);
            env.fs.crash(Crash::KeepTornPrefix(seed));
            let path = env.path("docs/a").join(&name);
            let bytes = env.fs.files()[&path].clone();
            let contents = read_log_file(&path, &bytes);
            let kept_a_failed_record = contents.records.len() > 3;
            (contents.end == LogEnd::Torn && kept_a_failed_record).then_some((env, bytes))
        })
        .expect("a seed that keeps part of the failed write");

    let store = env.open();
    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), [v1_snapshot_name(1)]);
    assert_eq!(
        env.quarantine(),
        [(env.path("quarantine/a").join(&name), torn)].into(),
        "moved byte for byte"
    );
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}

#[test]
fn a_damaged_head_in_a_segment_a_failed_write_tore_moves_it_to_quarantine_not_deletion() {
    let env = Env::manual();
    let store = env.open();
    let a = doc("a");
    let sent: Vec<Vec<u8>> = (1..=5).map(|client| typed(client, 4)).collect();
    fail_then_recover(&env, &store, &a, &sent, 1);
    let name = log_name(0, 0);
    let path = env.path("docs/a").join(&name);
    assert_eq!(
        read_log_file(&path, &env.fs.files()[&path]).records.len(),
        4,
        "the failed write kept half its bytes, the whole fourth record"
    );
    env.fs.flip_bit(&path, frames(&sent)[3].length_byte(2));
    let damaged = env.fs.files()[&path].clone();

    store.compact(&a).unwrap();
    assert_eq!(env.doc_files(&a), [v2_snapshot_name(1)]);
    assert_eq!(
        env.quarantine(),
        [(env.path("quarantine/a").join(&name), damaged)].into(),
        "moved byte for byte"
    );
    drop(store);
    assert_doc(&env.open(), &a, &sent);
}
