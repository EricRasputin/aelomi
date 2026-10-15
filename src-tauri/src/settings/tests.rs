use std::path::Path;
use std::sync::Barrier;
use std::thread;

use aelomi_core::storage::test_support::SimDisk;

use super::*;
use crate::storage::{DataLocation, Storage};

fn session_patch() -> SettingsPatch {
    SettingsPatch {
        theme: Some(Theme::Dark),
        open_page: Some(OpenPagePatch::Set(SettingsOpenPage {
            page_id: "page-a".into(),
            top_visible_block_id: Some("block-7".into()),
        })),
        recent_pages: Some(vec!["page-b".into(), "page-a".into()]),
        expanded_rows: Some(vec!["page-a".into(), "page-b".into()]),
    }
}

fn owner(storage: &Storage) -> Arc<SettingsOwner> {
    let owner = SettingsOwner::new(storage.workspace().unwrap());
    owner.initialize();
    owner
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
                    theme: Some(Theme::Light),
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
        Settings::from(&store.load_settings()),
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
            theme: Some(Theme::Light),
            ..Default::default()
        })
        .unwrap();
    disk.crash();
    let reopened = disk.open(Path::new("/data")).unwrap();
    assert_eq!(Settings::from(&reopened.load_settings()), saved);
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
        theme: Some(Theme::Dark),
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
    app.manage(SettingsState(None));
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
    let owner = owner(&storage);
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage(SettingsState(Some(owner)));
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
        assert_eq!(Settings::from(&reopened.load_settings()), updated);
    });
}
