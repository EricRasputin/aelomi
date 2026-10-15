use std::sync::Arc;
use std::time::Duration;

use super::*;
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
fn data_4_missing_corrupt_and_unreadable_settings_use_defaults_and_can_be_replaced() {
    let fs = disk();
    assert_eq!(load(&*fs, Path::new("/data")), Settings::default());
    for bytes in [
        &b"{broken"[..],
        b"[]",
        br#"{"theme":42}"#,
        br#"{"recentPages":["workspace"]}"#,
    ] {
        fs.put_durable(Path::new("/data/settings.json"), bytes);
        assert_eq!(load(&*fs, Path::new("/data")), Settings::default());
        write(&*fs, Path::new("/data"), &session()).unwrap();
        assert_eq!(load(&*fs, Path::new("/data")), session());
    }
    fs.fail(OpKind::Read, "settings.json", libc::EACCES);
    assert_eq!(load(&*fs, Path::new("/data")), Settings::default());
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
    fs.set_latency(OpKind::FullSync, Duration::from_millis(6));
    fs.set_latency(OpKind::Fsync, Duration::from_millis(1));
    let before = clock.now();
    write(&*fs, Path::new("/data"), &session()).unwrap();
    assert_eq!(clock.now() - before, Duration::from_millis(14));
    fs.crash(Crash::DropUnsynced);
    assert_eq!(load(&*fs, Path::new("/data")), session());
}

#[test]
fn data_4_settings_replace_refuses_an_unsynced_source() {
    let fs = disk();
    let root = Path::new("/data");
    write(&*fs, root, &Settings::default()).unwrap();
    fs.create_new(&root.join("settings.json.tmp")).unwrap();
    fs.append(
        &root.join("settings.json.tmp"),
        &serde_json::to_vec(&session().to_json()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        fs.replace_settings(root).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    fs.crash(Crash::DropUnsynced);
    assert_eq!(load(&*fs, root), Settings::default());
}
