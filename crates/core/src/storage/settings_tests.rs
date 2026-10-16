use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::settings::*;
use crate::workspace::records::PageId;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

fn write(fs: &dyn Fs, root: &Path, value: &Settings) -> io::Result<()> {
    super::write(fs, &Layout::new(root), value)
}
fn load(fs: &dyn Fs, root: &Path) -> Settings {
    super::load(fs, &Layout::new(root)).unwrap()
}
use crate::storage::clock::{Clock, ManualClock};
use crate::storage::sim_fs::{Crash, OpKind, SimFs};

fn session() -> Settings {
    Settings::from_json(
        br#"{
        "theme":"Dark",
        "openPage":{"pageId":"page-a","topVisibleBlockId":"block-7"},
        "recentPages":["page-b","page-a"],
        "expandedRows":["page-a","page-b"]
    }"#,
    )
    .unwrap()
}

fn disk() -> Arc<SimFs> {
    let fs = SimFs::new(Arc::new(ManualClock::default()));
    fs.put_durable(Path::new("/data/format.json"), br#"{"format":1}"#);
    fs
}

#[test]
fn data_4_session_state_round_trips_on_simulated_disk() {
    let fs = disk();
    write(&*fs, Path::new("/data"), &session()).unwrap();
    fs.crash(Crash::DropUnsynced);
    assert_eq!(load(&*fs, Path::new("/data")), session());
    assert_eq!(fs.violations(), Vec::<String>::new());
}

#[test]
fn data_4_every_write_crash_keeps_the_complete_old_or_new_settings() {
    let old = Settings::default();
    let new = session();
    let baseline = disk();
    write(&*baseline, Path::new("/data"), &old).unwrap();
    let before = baseline.ops().len();
    write(&*baseline, Path::new("/data"), &new).unwrap();
    let steps = baseline.ops().len() - before;
    assert_eq!(steps, 8);
    let mut seen = BTreeSet::new();
    for stale_temp in [false, true] {
        for step in 0..=steps {
            for seed in 0..40 {
                let fs = disk();
                write(&*fs, Path::new("/data"), &old).unwrap();
                if stale_temp {
                    fs.put_durable(Path::new("/data/settings.json.tmp"), b"torn prior attempt");
                }
                fs.die_after(step);
                let result = write(&*fs, Path::new("/data"), &new);
                assert_eq!(result.is_ok(), step == steps);
                fs.crash(if seed == 0 {
                    Crash::DropUnsynced
                } else {
                    Crash::KeepTornPrefix(seed)
                });
                let bytes = fs.read(Path::new("/data/settings.json")).unwrap();
                let actual = Settings::from_json(&bytes).expect("published settings always parse");
                assert!(actual == old || actual == new, "step={step} seed={seed}");
                if step == steps {
                    assert_eq!(actual, new);
                }
                seen.insert(actual.to_json().to_string());
                assert_eq!(fs.violations(), Vec::<String>::new());
            }
        }
    }
    assert_eq!(
        seen,
        BTreeSet::from([old.to_json().to_string(), new.to_json().to_string()])
    );
}

#[test]
fn data_4_unknown_keys_and_theme_survive_read_modify_write() {
    let fs = disk();
    fs.put_durable(
        Path::new("/data/settings.json"),
        br#"{
        "theme":"Sepia",
        "export":{"schedule":"nightly","future":null},
        "openPage":{"pageId":"page-a","topVisibleBlockId":"b1","futureAnchor":42},
        "recentPages":["page-b"]
    }"#,
    );
    let mut settings = load(&*fs, Path::new("/data"));
    settings.apply(SettingsPatch {
        open_page: Some(Some(OpenPage {
            page_id: PageId::new("page-a").unwrap(),
            top_visible_block_id: Some("b2".into()),
            extra: Default::default(),
        })),
        expanded_rows: Some(BTreeSet::from([PageId::new("page-c").unwrap()])),
        ..Default::default()
    });
    write(&*fs, Path::new("/data"), &settings).unwrap();
    fs.crash(Crash::DropUnsynced);
    let actual: Value =
        serde_json::from_slice(&fs.read(Path::new("/data/settings.json")).unwrap()).unwrap();
    assert_eq!(
        actual,
        json!({"theme":"Sepia", "export":{"schedule":"nightly","future":null}, "openPage":{"pageId":"page-a","topVisibleBlockId":"b2","futureAnchor":42}, "recentPages":["page-b"], "expandedRows":["page-c"]})
    );
    assert_eq!(load(&*fs, Path::new("/data")), settings);
}

#[test]
fn data_4_missing_and_corrupt_settings_use_defaults_and_can_be_replaced() {
    let fs = disk();
    assert_eq!(load(&*fs, Path::new("/data")), Settings::default());
    for bytes in [&b"{broken"[..], b"[]"] {
        fs.put_durable(Path::new("/data/settings.json"), bytes);
        assert_eq!(load(&*fs, Path::new("/data")), Settings::default());
        write(&*fs, Path::new("/data"), &session()).unwrap();
        assert_eq!(load(&*fs, Path::new("/data")), session());
    }
    fs.fail(OpKind::Read, "settings.json", libc::EACCES);
    assert_eq!(
        super::load(&*fs, &Layout::new(Path::new("/data")))
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    fs.clear_faults();
    write(&*fs, Path::new("/data"), &session()).unwrap();
    assert_eq!(load(&*fs, Path::new("/data")), session());
}

#[test]
fn data_4_recent_pages_keep_the_first_occurrence_and_cap_at_fifty() {
    let mut settings = Settings::default();
    let pages = (0..60)
        .flat_map(|i| vec![PageId::new(format!("p-{i}")).unwrap(); 2])
        .collect();
    settings.apply(SettingsPatch {
        recent_pages: Some(pages),
        ..Default::default()
    });
    assert_eq!(
        settings
            .recent_pages
            .iter()
            .map(PageId::as_str)
            .collect::<Vec<_>>(),
        (0..50).map(|i| format!("p-{i}")).collect::<Vec<_>>()
    );
}

#[test]
fn shell_9_settings_returns_durable_within_one_hundred_ms_on_the_controllable_clock() {
    let clock = Arc::new(ManualClock::default());
    let fs = SimFs::new(clock.clone());
    fs.put_durable(Path::new("/data/format.json"), br#"{"format":1}"#);
    let store = crate::storage::Store::open_with(
        Path::new("/data"),
        fs.clone(),
        clock.clone(),
        crate::storage::Driver::Manual,
        crate::storage::Limits::DATA_6,
    )
    .unwrap();
    fs.set_latency(OpKind::FullSync, Duration::from_millis(6));
    fs.set_latency(OpKind::Fsync, Duration::from_millis(1));
    let before = clock.now();
    let seq = store.queue_settings(session());
    store.run_writer_once();
    store.wait_settings(seq).unwrap();
    assert_eq!(clock.now() - before, Duration::from_millis(14));
    fs.crash(Crash::DropUnsynced);
    assert_eq!(load(&*fs, Path::new("/data")), session());
}

#[test]
fn data_4_settings_replace_refuses_an_unsynced_source() {
    let fs = disk();
    let root = Path::new("/data");
    write(&*fs, root, &Settings::default()).unwrap();
    let source = create_durable(&*fs, &root.join("settings.json.tmp"), b"").unwrap();
    fs.append(
        &root.join("settings.json.tmp"),
        &serde_json::to_vec(&session().to_json()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        fs.replace(root, RootFile::Settings, source)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    fs.crash(Crash::DropUnsynced);
    assert_eq!(load(&*fs, root), Settings::default());
}

#[test]
fn data_4_invalid_known_fields_preserve_other_fields_and_raw_values() {
    let mut settings = Settings::from_json(br#"{"theme":{"name":"Dark"},"openPage":{"futurePage":42},"recentPages":["page-a","workspace",42,"page-b","page-a"],"expandedRows":[false,"page-c","page-c"],"export":{"schedule":"nightly"}}"#).unwrap();
    settings.apply(SettingsPatch {
        expanded_rows: Some(BTreeSet::from([PageId::new("page-d").unwrap()])),
        ..Default::default()
    });
    assert_eq!(
        settings.to_json(),
        json!({"theme":{"name":"Dark"},"openPage":{"futurePage":42},"recentPages":["page-a","workspace",42,"page-b","page-a"],"expandedRows":["page-d"],"export":{"schedule":"nightly"}})
    );
}

#[test]
fn data_4_open_page_extensions_do_not_move_to_another_page() {
    let mut settings =
        Settings::from_json(br#"{"openPage":{"pageId":"page-a","futureAnchor":42}}"#).unwrap();
    settings.apply(SettingsPatch {
        open_page: Some(Some(OpenPage {
            page_id: PageId::new("page-b").unwrap(),
            top_visible_block_id: Some("b2".into()),
            extra: Default::default(),
        })),
        ..Default::default()
    });
    assert_eq!(
        settings.to_json()["openPage"],
        json!({"pageId":"page-b","topVisibleBlockId":"b2"})
    );
}

#[test]
fn data_4_each_unparsable_field_survives_an_unrelated_patch() {
    for (key, invalid) in [
        ("theme", json!({"name":"Dark"})),
        ("openPage", json!({"pageId":"workspace","futureAnchor":42})),
        ("recentPages", json!({"future":true})),
        ("expandedRows", json!(42)),
        ("recentPages", json!(["page-a", {"id":"future-page"}])),
        ("expandedRows", json!(["page-a", "workspace"])),
    ] {
        let mut input = session().to_json();
        input[key] = invalid.clone();
        input["export"] = json!({"schedule":"nightly"});
        let mut settings = Settings::from_json(&serde_json::to_vec(&input).unwrap()).unwrap();
        settings.apply(SettingsPatch::default());
        assert_eq!(settings.to_json(), input);
        assert_eq!(
            Settings::from_json(&serde_json::to_vec(&settings.to_json()).unwrap()).unwrap(),
            settings
        );
    }
}

#[test]
fn data_4_explicit_patches_replace_retained_raw_fields() {
    let mut settings = Settings::from_json(br#"{"theme":{"name":"Dark"},"openPage":{"future":42},"recentPages":42,"expandedRows":42,"export":{"schedule":"nightly"}}"#).unwrap();
    settings.apply(SettingsPatch {
        theme: Some(ThemeChoice::Light),
        open_page: Some(None),
        recent_pages: Some(vec![PageId::new("page-a").unwrap()]),
        expanded_rows: Some(BTreeSet::new()),
    });
    assert_eq!(
        settings.to_json(),
        json!({"theme":"Light","openPage":null,"recentPages":["page-a"],"expandedRows":[],"export":{"schedule":"nightly"}})
    );
}

#[test]
fn data_4_os_replace_publishes_a_complete_settings_source() {
    use crate::storage::fs::OsFs;
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::new(dir.path());
    write(&OsFs, dir.path(), &Settings::default()).unwrap();
    let source = create_durable(
        &OsFs,
        &layout.temp_file(RootFile::Settings),
        &serde_json::to_vec(&session().to_json()).unwrap(),
    )
    .unwrap();
    OsFs.replace(dir.path(), RootFile::Settings, source)
        .unwrap();
    OsFs.fsync(dir.path()).unwrap();
    OsFs.full_sync(dir.path()).unwrap();
    assert_eq!(load(&OsFs, dir.path()), session());
}

#[test]
fn shell_10_an_older_retry_cannot_fail_or_defer_a_newer_settings_patch() {
    use crate::storage::tests::Env;
    use std::sync::mpsc;
    let env = Env::manual();
    let store = env.open();
    env.fs
        .fail(OpKind::Append, "settings.json.tmp", libc::EACCES);
    let a = store.queue_settings(Settings::default());
    store.run_writer_once();
    assert!(store.wait_settings(a).is_err());
    env.clock.advance(Duration::from_millis(100));
    env.fs.clear_faults();
    let hold = env
        .fs
        .hold_once(OpKind::Append, "settings.json.tmp", Duration::ZERO);
    env.fs
        .fail(OpKind::Fsync, "settings.json.tmp", libc::EACCES);
    std::thread::scope(|scope| {
        let retry = scope.spawn(|| store.run_writer_once());
        hold.wait_until_reached();
        let b = store.queue_settings(session());
        let (done, result) = mpsc::channel();
        let store_ref = &store;
        scope.spawn(move || done.send(store_ref.wait_settings(b)).unwrap());
        hold.release();
        retry.join().unwrap();
        let early = result.recv_timeout(Duration::from_millis(50));
        env.fs.clear_faults();
        store.run_writer_once();
        let saved = result.recv_timeout(Duration::from_secs(1));
        assert!(
            matches!(early, Err(mpsc::RecvTimeoutError::Timeout)),
            "new patch received {early:?} from the older retry"
        );
        saved.unwrap().unwrap();
    });
    env.fs.crash(Crash::DropUnsynced);
    assert_eq!(store.load_settings().unwrap(), session());
}

#[test]
fn shell_9_a_document_is_acknowledged_before_a_held_settings_write_in_the_same_pass() {
    use crate::storage::tests::{Env, doc, typed};
    let env = Env::manual();
    let store = env.open();
    let page = doc("page-a");
    let seq = store.append(&page, &typed(7, 3)).unwrap();
    store.queue_settings(session());
    let hold = env
        .fs
        .hold_once(OpKind::Append, "settings.json.tmp", Duration::ZERO);
    let acknowledged = std::thread::scope(|scope| {
        let writer = scope.spawn(|| store.run_writer_once());
        hold.wait_until_reached();
        let acknowledged = store.is_durable(&page, seq);
        hold.release();
        writer.join().unwrap();
        acknowledged
    });
    assert!(
        acknowledged,
        "settings must not hold the document acknowledgement"
    );
    drop(store);
    env.fs.crash(Crash::DropUnsynced);
    let reopened = env.open();
    crate::storage::tests::assert_doc(&reopened, &page, &[typed(7, 3)]);
    assert_eq!(reopened.load_settings().unwrap(), session());
}

#[test]
fn shell_9_a_document_appended_during_settings_io_lands_next_pass_within_the_clock_budget() {
    use crate::storage::tests::{Env, assert_doc, doc, typed};
    let env = Env::manual();
    let store = env.open();
    env.fs
        .set_latency(OpKind::FullSync, Duration::from_millis(6));
    env.fs.set_latency(OpKind::Fsync, Duration::from_millis(1));
    let hold = env.fs.hold_once(
        OpKind::Append,
        "settings.json.tmp",
        Duration::from_millis(20),
    );
    let settings = store.queue_settings(session());
    let page = doc("during-settings");
    let update = typed(108, 4);
    let (seq, before, held) = std::thread::scope(|scope| {
        let writer = scope.spawn(|| store.run_writer_once());
        hold.wait_until_reached();
        let before = env.clock.now();
        let seq = store.append(&page, &update).unwrap();
        let held = !store.is_durable(&page, seq);
        hold.release();
        writer.join().unwrap();
        (seq, before, held)
    });
    store.wait_settings(settings).unwrap();
    assert!(held);
    assert!(!store.is_durable(&page, seq));
    assert_eq!(env.clock.now() - before, Duration::from_millis(34));
    store.run_writer_once();
    assert!(store.is_durable(&page, seq));
    assert!(env.clock.now() - before <= Duration::from_millis(100));
    drop(store);
    env.fs.crash(Crash::DropUnsynced);
    let reopened = env.open();
    assert_doc(&reopened, &page, &[update]);
    assert_eq!(reopened.load_settings().unwrap(), session());
}

#[test]
fn shell_10_close_returns_the_store_while_settings_are_pending() {
    use crate::storage::tests::Env;
    let env = Env::manual();
    let store = env.open();
    env.fs
        .fail(OpKind::Append, "settings.json.tmp", libc::EACCES);
    store.queue_settings(session());
    let result = store.close(Instant::now() + Duration::from_millis(20));
    assert!(
        result.is_err(),
        "close discarded a pending settings snapshot"
    );
    let (store, pending) = result.unwrap_err();
    assert_eq!(
        pending,
        crate::storage::Pending {
            docs: vec![],
            settings: true
        }
    );
    env.fs.clear_faults();
    store
        .close(Instant::now() + Duration::from_secs(1))
        .unwrap();
    env.fs.crash(Crash::DropUnsynced);
    assert_eq!(env.open().load_settings().unwrap(), session());
}

#[test]
fn shell_10_a_settings_panic_does_not_fail_documents_and_a_document_panic_does_not_fail_settings() {
    use crate::storage::tests::{Env, doc, typed};
    use crate::storage::{Health, Pending};
    for panic_in_settings in [false, true] {
        let env = Env::manual();
        let store = env.open();
        let page = doc("page-a");
        store.append(&page, &typed(7, 3)).unwrap();
        let settings = store.queue_settings(session());
        env.fs.panic_once(
            OpKind::Append,
            if panic_in_settings {
                "settings.json.tmp"
            } else {
                "/docs/page-a/"
            },
        );
        store.run_writer_once();
        let pending = store.flush(Instant::now());
        assert_eq!(
            pending,
            Pending {
                docs: if panic_in_settings {
                    vec![]
                } else {
                    vec![page.clone()]
                },
                settings: panic_in_settings,
            }
        );
        assert!(
            matches!(store.health(), Health::Failing { reason: FailReason::Internal, pending_settings, .. } if pending_settings == panic_in_settings)
        );
        if !panic_in_settings {
            store.wait_settings(settings).unwrap();
        }
        env.fs.clear_faults();
        assert_eq!(
            store.flush(Instant::now() + Duration::from_secs(1)),
            Pending::default()
        );
        drop(store);
        env.fs.crash(Crash::DropUnsynced);
        let reopened = env.open();
        crate::storage::tests::assert_doc(&reopened, &page, &[typed(7, 3)]);
        assert_eq!(reopened.load_settings().unwrap(), session());
    }
}

#[test]
fn shell_10_a_panic_in_settings_failure_bookkeeping_does_not_stop_the_writer() {
    use crate::storage::state::Limits;
    use crate::storage::store::{Driver, Store};
    use crate::storage::tests::{Env, assert_doc, doc, typed};
    use crate::storage::{Health, Pending};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, mpsc};

    #[derive(Default)]
    struct TailClock {
        clock: ManualClock,
        panic_next: AtomicBool,
    }
    impl Clock for TailClock {
        fn unix_ms(&self) -> u64 {
            self.clock.unix_ms()
        }
        fn now(&self) -> Duration {
            assert!(
                !self.panic_next.swap(false, Ordering::SeqCst),
                "panic in settings failure bookkeeping"
            );
            self.clock.now()
        }
        fn sleep(&self, duration: Duration) {
            self.clock.sleep(duration);
        }
    }

    let env = Env::manual();
    let clock = Arc::new(TailClock::default());
    let store = Store::open_with(
        env.root(),
        env.fs.clone(),
        clock.clone(),
        Driver::Threads,
        Limits::DATA_6,
    )
    .unwrap();
    let (send, health) = mpsc::channel();
    let send = Mutex::new(send);
    store.subscribe_health(move |value| drop(send.lock().unwrap().send(value.clone())));
    let hold = env
        .fs
        .hold_once(OpKind::Append, "settings.json.tmp", Duration::ZERO);
    env.fs.fail(OpKind::Fsync, "/data", libc::EACCES);
    let settings = store.queue_settings(session());
    hold.wait_until_reached();
    clock.panic_next.store(true, Ordering::SeqCst);
    hold.release();
    assert_eq!(
        health.recv_timeout(Duration::from_secs(1)),
        Ok(Health::Failing {
            reason: FailReason::Internal,
            since: Duration::ZERO,
            pending_docs: vec![],
            pending_settings: true,
        })
    );
    assert_eq!(store.wait_settings(settings), Err(FailReason::Internal));
    env.fs.clear_faults();
    let page = doc("after-tail-panic");
    let update = typed(107, 4);
    store.append(&page, &update).unwrap();
    assert_eq!(
        store.flush(Instant::now() + Duration::from_secs(1)),
        Pending::default()
    );
    drop(store);
    env.fs.crash(Crash::DropUnsynced);
    let reopened = env.open();
    assert_doc(&reopened, &page, &[update]);
    assert_eq!(reopened.load_settings().unwrap(), session());
}
