use std::path::Path;
use std::sync::{Arc, Barrier};
use std::thread;

use aelomi_core::storage::test_support::SimDisk;

use super::*;
use crate::storage::{DataLocation, Storage};

fn session_patch() -> SettingsPatch {
    SettingsPatch {
        theme: Some(ThemeChoice::Dark),
        open_page: Some(OpenPagePatch::Set(SettingsOpenPage {
            page_id: "page-a".into(),
            top_visible_block_id: Some("block-7".into()),
        })),
        recent_pages: Some(vec!["page-b".into(), "page-a".into()]),
        expanded_rows: Some(vec!["page-a".into(), "page-b".into()]),
    }
}

struct Owner(Arc<aelomi_core::workspace::Workspace>);
impl Owner {
    fn get(&self) -> Settings {
        Settings::from(&self.0.settings().unwrap())
    }
    fn update(&self, patch: SettingsPatch) -> Result<Settings, SettingsError> {
        Ok(Settings::from(&self.0.update_settings(patch.try_into()?)?))
    }
}
fn owner(storage: &Storage) -> Arc<Owner> {
    Arc::new(Owner(storage.workspace().unwrap()))
}

#[test]
fn data_4_session_state_round_trips_through_the_owner_on_a_real_folder() {
    let dir = tempfile::tempdir().unwrap();
    let location = DataLocation {
        path: dir.path().into(),
        cloud_roots: vec![],
    };
    let expected;
    {
        let storage = Storage::open(&location, |_| {});
        let owner = owner(&storage);
        expected = owner.update(session_patch()).unwrap();
        assert_eq!(expected.theme, Theme::Dark);
        assert_eq!(
            expected.open_page,
            Some(SettingsOpenPage {
                page_id: "page-a".into(),
                top_visible_block_id: Some("block-7".into())
            })
        );
        assert_eq!(expected.recent_pages, ["page-b", "page-a"]);
        assert_eq!(expected.expanded_rows, ["page-a", "page-b"]);
    }
    let reopened = Storage::open(&location, |_| {});
    assert_eq!(owner(&reopened).get(), expected);
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("settings.json")).unwrap()).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"theme":"Dark","openPage":{"pageId":"page-a","topVisibleBlockId":"block-7"},"recentPages":["page-b","page-a"],"expandedRows":["page-a","page-b"]})
    );
}

#[test]
fn shell_9_owner_acknowledges_only_durable_settings_on_simulated_disk() {
    let disk = SimDisk::default();
    let expected;
    {
        let storage = crate::storage::tests::open_on(&disk);
        expected = owner(&storage).update(session_patch()).unwrap();
    }
    disk.crash();
    let storage = crate::storage::tests::open_on(&disk);
    assert_eq!(owner(&storage).get(), expected);
    assert_eq!(expected.recent_pages, ["page-b", "page-a"]);
}

#[test]
fn data_4_concurrent_patches_to_different_fields_both_land() {
    let disk = SimDisk::default();
    {
        let storage = crate::storage::tests::open_on(&disk);
        let owner = owner(&storage);
        let barrier = Arc::new(Barrier::new(3));
        thread::scope(|scope| {
            for patch in [
                SettingsPatch {
                    theme: Some(ThemeChoice::Light),
                    ..Default::default()
                },
                SettingsPatch {
                    expanded_rows: Some(vec!["page-b".into()]),
                    ..Default::default()
                },
            ] {
                let (owner, barrier) = (owner.clone(), barrier.clone());
                scope.spawn(move || {
                    barrier.wait();
                    owner.update(patch).unwrap();
                });
            }
            barrier.wait();
        });
        assert_eq!(
            owner.get(),
            Settings {
                theme: Theme::Light,
                open_page: None,
                recent_pages: vec![],
                expanded_rows: vec!["page-b".into()]
            }
        );
    }
    disk.crash();
    let store = disk.open(Path::new("/data")).unwrap();
    assert_eq!(
        Settings::from(&store.load_settings().unwrap()),
        Settings {
            theme: Theme::Light,
            open_page: None,
            recent_pages: vec![],
            expanded_rows: vec!["page-b".into()]
        }
    );
}

#[test]
fn data_4_failed_write_keeps_the_patch_for_the_next_durable_update() {
    let disk = SimDisk::default();
    let storage = crate::storage::tests::open_on(&disk);
    let owner = owner(&storage);
    disk.fail_appends("settings.json.tmp");
    assert!(matches!(
        owner.update(session_patch()),
        Err(SettingsError::Io(_))
    ));
    assert_eq!(owner.get().theme, Theme::Dark);
    disk.clear_faults();
    let saved = owner
        .update(SettingsPatch {
            theme: Some(ThemeChoice::Light),
            ..Default::default()
        })
        .unwrap();
    disk.crash();
    let reopened = disk.open(Path::new("/data")).unwrap();
    assert_eq!(Settings::from(&reopened.load_settings().unwrap()), saved);
    assert_eq!(
        saved.open_page,
        Some(SettingsOpenPage {
            page_id: "page-a".into(),
            top_visible_block_id: Some("block-7".into())
        })
    );
}

#[test]
fn data_4_invalid_patch_is_rejected_before_any_field_changes() {
    let disk = SimDisk::default();
    let storage = crate::storage::tests::open_on(&disk);
    let owner = owner(&storage);
    let before = owner.get();
    let result = owner.update(SettingsPatch {
        theme: Some(ThemeChoice::Dark),
        recent_pages: Some(vec!["workspace".into()]),
        ..Default::default()
    });
    assert_eq!(result, Err(SettingsError::InvalidPageId));
    assert_eq!(owner.get(), before);
}

#[test]
fn data_4_commands_return_unavailable_when_storage_did_not_open() {
    use tauri::Manager;
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage(Storage::not_open());
    tauri::async_runtime::block_on(async {
        assert_eq!(
            crate::commands::settings_get(app.state()).await,
            Err(SettingsError::Unavailable)
        );
        assert_eq!(
            crate::commands::settings_update(app.state(), session_patch()).await,
            Err(SettingsError::Unavailable)
        );
    });
}

#[test]
fn data_4_commands_update_and_get_the_full_durable_snapshot() {
    use tauri::Manager;
    let disk = SimDisk::default();
    let storage = crate::storage::tests::open_on(&disk);
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage(storage);
    tauri::async_runtime::block_on(async {
        let updated = crate::commands::settings_update(app.state(), session_patch())
            .await
            .unwrap();
        assert_eq!(updated.theme, Theme::Dark);
        assert_eq!(
            crate::commands::settings_get(app.state()).await.unwrap(),
            updated
        );
        disk.crash();
        let reopened = disk.open(Path::new("/data")).unwrap();
        assert_eq!(Settings::from(&reopened.load_settings().unwrap()), updated);
    });
}

#[test]
fn shell_9_quit_defers_exit_until_a_held_settings_write_is_durable() {
    use std::sync::mpsc;
    use std::time::Duration;
    let disk = SimDisk::default();
    let storage = crate::storage::tests::open_on(&disk);
    let owner = owner(&storage);
    let hold = disk.hold_settings_write();
    thread::scope(|scope| {
        let writing = scope.spawn(|| owner.update(session_patch()).unwrap());
        hold.wait_until_reached();
        let (done, result) = mpsc::channel();
        let storage_ref = &storage;
        scope.spawn(move || {
            done.send(storage_ref.flush_before_quit(Duration::from_secs(2)))
                .unwrap()
        });
        let early = result.recv_timeout(Duration::from_millis(100));
        hold.release();
        assert_eq!(
            early,
            Err(mpsc::RecvTimeoutError::Timeout),
            "quit must not confirm while settings are held"
        );
        assert_eq!(result.recv_timeout(Duration::from_secs(2)).unwrap(), Ok(()));
        let expected = writing.join().unwrap();
        disk.crash();
        let reopened = disk.open(Path::new("/data")).unwrap();
        assert_eq!(Settings::from(&reopened.load_settings().unwrap()), expected);
    });
}

#[test]
fn data_4_unreadable_settings_refuse_updates_until_a_read_succeeds() {
    use tauri::Manager;
    let disk = SimDisk::default();
    let storage = crate::storage::tests::open_on(&disk);
    let expected = owner(&storage).update(session_patch()).unwrap();
    drop(storage);
    disk.fail_settings_reads();
    let storage = crate::storage::tests::open_on(&disk);
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage(storage);
    tauri::async_runtime::block_on(async {
        assert!(matches!(
            crate::commands::settings_update(
                app.state(),
                SettingsPatch {
                    theme: Some(ThemeChoice::Light),
                    ..Default::default()
                }
            )
            .await,
            Err(SettingsError::Io(_))
        ));
    });
    disk.clear_faults();
    tauri::async_runtime::block_on(async {
        assert_eq!(
            crate::commands::settings_get(app.state()).await.unwrap(),
            expected
        );
    });
    disk.crash();
    let reopened = disk.open(Path::new("/data")).unwrap();
    assert_eq!(Settings::from(&reopened.load_settings().unwrap()), expected);
}

#[test]
fn data_4_unknown_theme_is_output_only() {
    for theme in [r#"{"Unknown":"Dark"}"#, r#"{"Unknown":"Sepia"}"#] {
        let input = format!("{{\"theme\":{theme}}}");
        assert!(serde_json::from_str::<SettingsPatch>(&input).is_err());
    }
    assert_eq!(
        serde_json::to_value(Settings::from(
            &core::Settings::from_json(br#"{"theme":"Sepia"}"#).unwrap()
        ))
        .unwrap()["theme"],
        serde_json::json!({"Unknown":"Sepia"})
    );
}

#[test]
fn shell_10_failed_settings_retry_without_another_patch_and_clear_health() {
    use std::sync::mpsc;
    use std::time::Duration;
    let disk = SimDisk::default();
    let storage = crate::storage::tests::open_on(&disk);
    let workspace = storage.workspace().unwrap();
    let (send, health) = mpsc::channel();
    workspace.subscribe_health(move |value| {
        send.send(value.clone()).unwrap();
    });
    disk.fail_appends("settings.json.tmp");
    assert!(matches!(
        owner(&storage).update(session_patch()),
        Err(SettingsError::Io(_))
    ));
    let failed = health.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        failed,
        aelomi_core::storage::Health::Failing {
            pending_settings: true,
            ..
        }
    ));
    assert_eq!(storage.pending_docs(), ["settings.json"]);
    disk.clear_faults();
    assert_eq!(
        health.recv_timeout(Duration::from_secs(2)).unwrap(),
        aelomi_core::storage::Health::Healthy
    );
    disk.crash();
    let reopened = disk.open(Path::new("/data")).unwrap();
    assert_eq!(
        Settings::from(&reopened.load_settings().unwrap()),
        Settings {
            theme: Theme::Dark,
            open_page: Some(SettingsOpenPage {
                page_id: "page-a".into(),
                top_visible_block_id: Some("block-7".into())
            }),
            recent_pages: vec!["page-b".into(), "page-a".into()],
            expanded_rows: vec!["page-a".into(), "page-b".into()]
        }
    );
}

#[test]
fn shell_9_quit_waits_for_a_settings_command_queued_before_its_worker_runs() {
    use std::sync::mpsc;
    use std::time::Duration;
    let disk = SimDisk::default();
    let storage = crate::storage::tests::open_on(&disk);
    let workspace = storage.workspace().unwrap();
    let update = workspace.prepare_settings_update(session_patch().try_into().unwrap());
    thread::scope(|scope| {
        let (done, result) = mpsc::channel();
        let storage_ref = &storage;
        scope.spawn(move || {
            done.send(storage_ref.flush_before_quit(Duration::from_secs(2)))
                .unwrap()
        });
        assert_eq!(
            result.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        let snapshot = update.run().unwrap();
        assert_eq!(result.recv_timeout(Duration::from_secs(2)).unwrap(), Ok(()));
        disk.crash();
        let reopened = disk.open(Path::new("/data")).unwrap();
        assert_eq!(reopened.load_settings().unwrap(), snapshot);
        assert_eq!(snapshot.theme, core::Theme::Dark);
    });
}
