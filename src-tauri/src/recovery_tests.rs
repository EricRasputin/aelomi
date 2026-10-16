use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use super::*;

struct Home {
    _dir: tempfile::TempDir,
    root: PathBuf,
    location: DataLocation,
    record: RecoveryFile,
    cloud: PathBuf,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let cloud = root.join("cloud");
        fs::create_dir_all(&cloud).unwrap();
        Self {
            _dir: dir,
            location: DataLocation {
                path: root.join("local/Aelomi"),
                cloud_roots: vec![cloud.clone()],
            },
            record: RecoveryFile(root.join("preferences/recovery.json")),
            root,
            cloud,
        }
    }

    fn open(&self) -> Storage {
        Storage::open_with_recovery(&self.location, self.record.0.clone(), |_| {})
    }

    fn leaf(&self) -> PathBuf {
        let target = self.cloud.join("Aelomi Notes");
        let workspace = Workspace::open(&target, |_| {}).unwrap();
        workspace
            .append(
                &core::DocId::new("original").unwrap(),
                &tests::update("Original pages stay here"),
            )
            .unwrap();
        assert!(
            workspace
                .flush(Instant::now() + QUIT_FLUSH_TIMEOUT)
                .is_empty()
        );
        drop(workspace);
        fs::create_dir_all(self.location.path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&target, &self.location.path).unwrap();
        target
    }

    fn pending(&self) -> PendingCloudRefusal {
        self.record.load().unwrap()
    }
}

fn listing(path: &Path) -> Vec<OsString> {
    let mut names: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    names
}

#[test]
fn every_cloud_variant_records_paths_variant_and_timestamp_outside_the_workspace() {
    for variant in ["leaf", "shared", "override"] {
        let mut home = Home::new();
        let target = home.leaf();
        if variant != "leaf" {
            fs::remove_file(&home.location.path).unwrap();
            if variant == "shared" {
                let shared = home.root.join("shared");
                std::os::unix::fs::symlink(&home.cloud, &shared).unwrap();
                home.location.path = shared.join("Aelomi Notes");
            } else {
                home.location.path = target.clone();
            }
        }
        let before = listing(&target);
        let storage = home.open();
        let StorageStatus::CloudPath {
            configured,
            resolved,
            recovery,
        } = storage.status()
        else {
            panic!("cloud refusal")
        };
        let record = home.pending();
        assert_eq!(record.configured, PathBuf::from(configured));
        assert_eq!(record.resolved, PathBuf::from(resolved));
        assert_eq!(record.recovery, recovery);
        assert!(record.refused_at_unix_ms >= 1_760_000_000_000);
        assert_eq!(listing(&target), before);
        let bytes = fs::read(&home.record.0).unwrap();
        let decoded: PendingCloudRefusal = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, record);
        assert_eq!(
            listing(home.record.0.parent().unwrap()),
            [OsString::from("recovery.json")]
        );
    }
}

#[test]
fn misplaced_drop_blocks_creation_for_missing_and_markerless_destinations() {
    let home = Home::new();
    let target = home.leaf();
    assert!(matches!(
        home.open().status(),
        StorageStatus::CloudPath { .. }
    ));
    let pending = home.pending();
    fs::remove_file(&home.location.path).unwrap();
    let misplaced = home.root.join("local/subfolder");
    fs::create_dir(&misplaced).unwrap();
    fs::rename(target, misplaced.join("Aelomi Notes")).unwrap();
    let before = listing(home.location.path.parent().unwrap());
    for exists in [false, true] {
        if exists {
            fs::create_dir(&home.location.path).unwrap();
        }
        let storage = home.open();
        assert_eq!(
            storage.status(),
            StorageStatus::FinishMoving {
                configured: home.location.path.to_string_lossy().into_owned(),
                resolved: pending.resolved.to_string_lossy().into_owned(),
                recovery: pending.recovery.clone(),
            }
        );
        assert_eq!(home.pending(), pending);
        assert_eq!(home.location.path.exists(), exists);
        if exists {
            assert_eq!(listing(&home.location.path), Vec::<OsString>::new());
        } else {
            assert_eq!(listing(home.location.path.parent().unwrap()), before);
        }
    }
}

#[test]
fn a_correct_move_opens_the_original_document_and_clears_the_record() {
    let home = Home::new();
    let target = home.leaf();
    home.open();
    fs::remove_file(&home.location.path).unwrap();
    fs::rename(target, &home.location.path).unwrap();
    let storage = home.open();
    assert_eq!(storage.status(), StorageStatus::Ready);
    assert_eq!(
        tests::text_of(
            &storage
                .workspace()
                .unwrap()
                .load(&core::DocId::new("original").unwrap())
                .unwrap()
        ),
        "Original pages stay here"
    );
    assert_eq!(home.record.0.try_exists().unwrap(), false);
}

#[test]
fn still_cloud_keeps_the_refusal_and_record() {
    let home = Home::new();
    home.leaf();
    assert!(matches!(
        home.open().status(),
        StorageStatus::CloudPath { .. }
    ));
    let first = home.pending();
    assert!(matches!(
        home.open().status(),
        StorageStatus::CloudPath { .. }
    ));
    assert_eq!(home.pending().configured, first.configured);
    assert_eq!(home.pending().resolved, first.resolved);
    assert_eq!(home.pending().recovery, first.recovery);
}

#[test]
fn a_different_configured_path_ignores_and_retains_the_record() {
    let mut home = Home::new();
    home.leaf();
    home.open();
    let record = home.pending();
    home.location.path = home.root.join("different");
    let storage = home.open();
    assert_eq!(storage.status(), StorageStatus::Ready);
    assert_eq!(
        fs::read_to_string(home.location.path.join("format.json")).unwrap(),
        "{\n  \"format\": 1\n}\n"
    );
    assert_eq!(home.pending(), record);
}

#[test]
fn unreadable_records_are_ignored_without_overwriting_them() {
    for bytes in [b"broken JSON".as_slice(), br#"{"configured":"relative","resolved":"/cloud","recovery":"Override","refused_at_unix_ms":1}"#] {
        let home = Home::new();
        fs::create_dir_all(home.record.0.parent().unwrap()).unwrap();
        fs::write(&home.record.0, bytes).unwrap();
        assert_eq!(home.open().status(), StorageStatus::Ready);
        assert_eq!(fs::read(&home.record.0).unwrap(), bytes);
    }
}

#[test]
fn explicit_empty_creation_leaves_original_bytes_and_clears_only_after_open() {
    let home = Home::new();
    let target = home.leaf();
    home.open();
    let original = fs::read(target.join("format.json")).unwrap();
    fs::remove_file(&home.location.path).unwrap();
    let storage = home.open();
    assert_eq!(
        storage.start_empty_workspace(|_| {}).unwrap(),
        StorageStatus::Ready
    );
    assert_eq!(storage.status(), StorageStatus::Ready);
    assert_eq!(storage.workspace().unwrap().tree().unwrap().pages.len(), 0);
    assert_eq!(
        storage
            .workspace()
            .unwrap()
            .load(&core::DocId::new("original").unwrap())
            .unwrap(),
        vec![0, 0]
    );
    assert_eq!(fs::read(target.join("format.json")).unwrap(), original);
    let original = Workspace::open(&target, |_| {}).unwrap();
    assert_eq!(
        tests::text_of(
            &original
                .load(&core::DocId::new("original").unwrap())
                .unwrap()
        ),
        "Original pages stay here"
    );
    assert_eq!(home.record.0.try_exists().unwrap(), false);
}

#[test]
fn explicit_creation_rechecks_cloud_and_never_overwrites_unrecognized_files() {
    for cloud_again in [false, true] {
        let home = Home::new();
        let target = home.leaf();
        home.open();
        fs::remove_file(&home.location.path).unwrap();
        let storage = home.open();
        if cloud_again {
            std::os::unix::fs::symlink(&target, &home.location.path).unwrap();
        } else {
            fs::create_dir(&home.location.path).unwrap();
            fs::create_dir(home.location.path.join("docs")).unwrap();
            fs::write(home.location.path.join("docs/sentinel"), b"never overwrite").unwrap();
        }
        let status = storage.start_empty_workspace(|_| {}).unwrap();
        if cloud_again {
            assert!(matches!(status, StorageStatus::CloudPath { .. }));
            assert_eq!(fs::read_link(&home.location.path).unwrap(), target);
        } else {
            assert_eq!(status, StorageStatus::UnknownFormat);
            assert_eq!(
                fs::read(home.location.path.join("docs/sentinel")).unwrap(),
                b"never overwrite"
            );
            assert_eq!(listing(&home.location.path), [OsString::from("docs")]);
        }
        assert_eq!(home.pending().configured, home.location.path);
    }
}

#[test]
fn record_failure_refuses_recovery_instructions_and_does_not_touch_pages() {
    let home = Home::new();
    let target = home.leaf();
    fs::write(home.root.join("preferences"), b"blocked").unwrap();
    let before = listing(&target);
    assert!(matches!(home.open().status(), StorageStatus::Io(_)));
    assert_eq!(listing(&target), before);
    assert_eq!(fs::read(home.root.join("preferences")).unwrap(), b"blocked");
}

#[test]
fn records_cannot_be_written_inside_cloud_or_the_data_folder() {
    for relative in ["cloud/recovery.json", "local/Aelomi/recovery.json"] {
        let mut home = Home::new();
        let target = home.leaf();
        home.record = RecoveryFile(home.root.join(relative));
        let before = listing(&target);
        assert!(matches!(home.open().status(), StorageStatus::Io(_)));
        assert_eq!(listing(&target), before);
        assert_eq!(home.record.0.try_exists().unwrap(), false);
    }
}

#[test]
fn failed_workspace_opens_retain_the_matching_record() {
    for marker in [r#"{"format":3}"#, r#"{"format":"broken"}"#] {
        let home = Home::new();
        let target = home.leaf();
        home.open();
        let pending = home.pending();
        fs::remove_file(&home.location.path).unwrap();
        fs::rename(target, &home.location.path).unwrap();
        fs::write(home.location.path.join("format.json"), marker).unwrap();
        let status = home.open().status();
        assert_eq!(
            status,
            if marker.contains('3') {
                StorageStatus::NewerFormat
            } else {
                StorageStatus::UnknownFormat
            }
        );
        assert_eq!(home.pending(), pending);
        assert_eq!(
            fs::read_to_string(home.location.path.join("format.json")).unwrap(),
            marker
        );
    }
}

#[test]
fn atomic_record_replacement_keeps_one_complete_latest_record() {
    let home = Home::new();
    let mut record = PendingCloudRefusal::new(
        &home.location.path,
        &home.cloud.join("old"),
        CloudRecovery::Override,
    );
    home.record.save(&record).unwrap();
    assert_eq!(home.pending(), record);
    record.resolved = home.cloud.join("new");
    record.refused_at_unix_ms += 1;
    home.record.save(&record).unwrap();
    assert_eq!(home.pending(), record);
    assert_eq!(
        listing(home.record.0.parent().unwrap()),
        [OsString::from("recovery.json")]
    );
}
