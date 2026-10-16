use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use super::*;

struct Home {
    _dir: tempfile::TempDir,
    root: PathBuf,
    location: DataLocation,
    record: RecoveryDirectory,
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
            record: RecoveryDirectory(root.join("preferences/recovery.json")),
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

    fn file(&self) -> crate::recovery::RecoveryFile {
        self.record
            .file_for(&normalize_configured(&self.location.path).unwrap())
    }

    fn pending(&self) -> PendingCloudRefusal {
        self.file().load().unwrap().unwrap()
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
        assert_eq!(record.recovery().unwrap(), recovery);
        assert!(record.refused_at_unix_ms >= 1_760_000_000_000);
        assert_eq!(listing(&target), before);
        let bytes = fs::read(&home.file().path).unwrap();
        let decoded: PendingCloudRefusal = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, record);
        assert_eq!(
            listing(home.record.0.as_path()),
            [home.file().path.file_name().unwrap().to_owned()]
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
                recovery: pending.recovery().unwrap(),
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
    assert_eq!(home.file().path.try_exists().unwrap(), false);
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
    assert_eq!(
        home.pending().recovery().unwrap(),
        first.recovery().unwrap()
    );
}

#[test]
fn a_different_configured_path_ignores_and_retains_the_record() {
    let mut home = Home::new();
    home.leaf();
    home.open();
    let record = home.pending();
    let original_file = home.file();
    home.location.path = home.root.join("different");
    let storage = home.open();
    assert_eq!(storage.status(), StorageStatus::Ready);
    assert_eq!(
        fs::read_to_string(home.location.path.join("format.json")).unwrap(),
        "{\n  \"format\": 1\n}\n"
    );
    assert_eq!(original_file.load().unwrap(), Some(record));
}

#[test]
fn unreadable_records_keep_the_guard_without_overwriting_them() {
    for bytes in [b"broken JSON".as_slice(), br#"{"configured":"relative","resolved":"/cloud","recovery":"Override","refused_at_unix_ms":1}"#] {
        let home = Home::new();
        fs::create_dir_all(home.record.0.as_path()).unwrap();
        fs::write(&home.file().path, bytes).unwrap();
        assert_eq!(serde_json::to_value(home.open().status()).unwrap(), serde_json::json!({"RecoveryRecordUnreadable": {"configured": home.location.path}}));
        assert_eq!(home.location.path.exists(), false);
        assert_eq!(fs::read(&home.file().path).unwrap(), bytes);
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
    assert_eq!(home.file().path.try_exists().unwrap(), false);
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
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(&home.record.0).unwrap();
    fs::set_permissions(&home.record.0, fs::Permissions::from_mode(0o500)).unwrap();
    assert!(home.record.validated(&home.location).is_ok());
    let before = listing(&target);
    assert_eq!(
        home.open().status(),
        StorageStatus::Io("Permission denied (os error 13)".into())
    );
    assert_eq!(listing(&target), before);
    assert_eq!(listing(&home.record.0), Vec::<OsString>::new());
    fs::set_permissions(&home.record.0, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn records_cannot_be_written_inside_cloud_or_the_data_folder() {
    for cloud in [false, true] {
        let mut home = Home::new();
        let target = if cloud {
            home.leaf()
        } else {
            fs::create_dir_all(&home.location.path).unwrap();
            fs::write(home.location.path.join("sentinel"), b"local pages").unwrap();
            home.location.path.clone()
        };
        home.record = RecoveryDirectory(target.join("recovery"));
        let before = listing(&target);
        let error = home
            .record
            .validated(&home.location)
            .err()
            .unwrap()
            .to_string();
        if !cloud {
            assert_eq!(
                error,
                "the recovery directory must be outside the data folder"
            );
        }
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
    home.file().save(&record).unwrap();
    assert_eq!(home.pending(), record);
    record.resolved = home.cloud.join("new");
    record.refused_at_unix_ms += 1;
    home.file().save(&record).unwrap();
    assert_eq!(home.pending(), record);
    assert_eq!(
        listing(home.record.0.as_path()),
        [home.file().path.file_name().unwrap().to_owned()]
    );
}

#[test]
fn marker_only_destination_keeps_guard_and_every_byte_unchanged() {
    let home = Home::new();
    let target = home.leaf();
    home.open();
    let pending = home.pending();
    fs::remove_file(&home.location.path).unwrap();
    fs::create_dir(&home.location.path).unwrap();
    fs::copy(
        target.join("format.json"),
        home.location.path.join("format.json"),
    )
    .unwrap();
    let before = listing(&home.location.path);
    let record = fs::read(&home.file().path).unwrap();
    assert_eq!(
        home.open().status(),
        StorageStatus::FinishMoving {
            configured: home.location.path.to_string_lossy().into_owned(),
            resolved: pending.resolved.to_string_lossy().into_owned(),
            recovery: pending.recovery().unwrap(),
        }
    );
    assert_eq!(listing(&home.location.path), before);
    assert_eq!(
        fs::read(home.location.path.join("format.json")).unwrap(),
        fs::read(target.join("format.json")).unwrap()
    );
    assert_eq!(fs::read(&home.file().path).unwrap(), record);
}

#[test]
fn docs_without_marker_reports_unknown_format_without_changing_destination_or_record() {
    let home = Home::new();
    home.leaf();
    home.open();
    fs::remove_file(&home.location.path).unwrap();
    fs::create_dir_all(home.location.path.join("docs")).unwrap();
    fs::write(home.location.path.join("docs/page"), b"page bytes").unwrap();
    let record = fs::read(&home.file().path).unwrap();
    assert_eq!(home.open().status(), StorageStatus::UnknownFormat);
    assert_eq!(listing(&home.location.path), [OsString::from("docs")]);
    assert_eq!(
        listing(&home.location.path.join("docs")),
        [OsString::from("page")]
    );
    assert_eq!(
        fs::read(home.location.path.join("docs/page")).unwrap(),
        b"page bytes"
    );
    assert_eq!(fs::read(&home.file().path).unwrap(), record);
}

#[test]
fn explicit_creation_refuses_unrecognized_items_and_keeps_the_record() {
    let home = Home::new();
    home.leaf();
    home.open();
    fs::remove_file(&home.location.path).unwrap();
    let storage = home.open();
    fs::create_dir(&home.location.path).unwrap();
    fs::write(
        home.location.path.join("misplaced-page.ylog"),
        b"original bytes",
    )
    .unwrap();
    let record = fs::read(&home.file().path).unwrap();
    assert_eq!(
        storage.start_empty_workspace(|_| {}).unwrap(),
        StorageStatus::UnknownFormat
    );
    assert_eq!(
        listing(&home.location.path),
        [OsString::from("misplaced-page.ylog")]
    );
    assert_eq!(
        fs::read(home.location.path.join("misplaced-page.ylog")).unwrap(),
        b"original bytes"
    );
    assert_eq!(fs::read(&home.file().path).unwrap(), record);
}

#[test]
fn clearing_x_after_read_does_not_remove_a_new_refusal_for_y() {
    let home = Home::new();
    let x = PendingCloudRefusal::new(
        &home.location.path,
        &home.cloud.join("X"),
        CloudRecovery::Override,
    );
    let y = PendingCloudRefusal::new(
        &home.root.join("Y"),
        &home.cloud.join("Y"),
        CloudRecovery::Override,
    );
    let x_file = home.record.file_for(&x.configured);
    let y_file = home.record.file_for(&y.configured);
    x_file.save(&x).unwrap();
    assert_eq!(x_file.load().unwrap(), Some(x.clone()));
    y_file.save(&y).unwrap();
    assert_eq!(x_file.load().unwrap(), Some(x));
    x_file.clear();
    assert_eq!(x_file.load().unwrap(), None);
    assert_eq!(y_file.load().unwrap(), Some(y));
    y_file.clear();
    assert_eq!(listing(&home.record.0), Vec::<OsString>::new());
}

#[test]
fn unknown_record_versions_and_unreadable_files_require_explicit_creation() {
    for directory in [false, true] {
        let home = Home::new();
        fs::create_dir_all(&home.record.0).unwrap();
        if directory {
            fs::create_dir(&home.file().path).unwrap();
        } else {
            fs::write(
                &home.file().path,
                serde_json::to_vec(&serde_json::json!({
                    "version": 99, "configured": home.location.path, "resolved": "/cloud",
                    "kind": "Override", "refused_at_unix_ms": 1
                }))
                .unwrap(),
            )
            .unwrap();
        }
        let storage = home.open();
        assert_eq!(
            storage.status(),
            StorageStatus::RecoveryRecordUnreadable {
                configured: home.location.path.to_string_lossy().into_owned()
            }
        );
        assert_eq!(home.location.path.exists(), false);
        if directory {
            fs::remove_dir(&home.file().path).unwrap();
        }
        assert_eq!(
            storage.start_empty_workspace(|_| {}).unwrap(),
            StorageStatus::Ready
        );
        assert_eq!(home.file().path.exists(), false);
        assert_eq!(storage.workspace().unwrap().tree().unwrap().pages.len(), 0);
    }
}

#[test]
fn normalized_configured_paths_have_one_stable_record_filename() {
    let directory = RecoveryDirectory("/preferences/recovery".into());
    let path = normalize_configured(Path::new("/local/missing/../Aelomi/")).unwrap();
    let file = directory.file_for(&path);
    assert_eq!(
        file.path,
        directory.file_for(Path::new("/local/Aelomi")).path
    );
    assert_eq!(
        file.path.file_name().unwrap().to_string_lossy(),
        "04e4667d4c7f20d958be55c7ec05a9af1188ae4bf03ddf4381f2d8ef0dc8c651.json"
    );
}
