use std::sync::Arc;

use super::{index_path, open, open_with, page};
use crate::index::gate::Gate;
use crate::index::{Config, HoldPoint};
use crate::storage::{AppendError, DocId, Store};
use crate::workspace::fixture::index_fixture;

fn title(opened: &super::Opened, id: &str) -> String {
    let tree = opened.workspace.tree().unwrap();
    tree.pages
        .iter()
        .find(|p| p.id == page(id))
        .map(|p| p.title.clone())
        .unwrap_or_default()
}

fn rename(opened: &super::Opened, id: &str, to: &str) {
    opened.edit_records(|records| records.get_mut(&page(id)).unwrap().title = to.into());
}

#[test]
fn the_tree_is_reused_until_a_workspace_append_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    let first = opened.workspace.tree().unwrap();
    assert!(Arc::ptr_eq(&first, &opened.workspace.tree().unwrap()));

    opened.edit_body("daily", |txn| {
        crate::workspace::fixture::write_body(txn, &[])
    });
    assert!(
        Arc::ptr_eq(&first, &opened.workspace.tree().unwrap()),
        "a body edit leaves the tree"
    );
    assert!(matches!(
        opened
            .workspace
            .append(&DocId::workspace(), b"not an update"),
        Err(AppendError::InvalidUpdate)
    ));
    assert!(
        Arc::ptr_eq(&first, &opened.workspace.tree().unwrap()),
        "a refused append leaves the tree"
    );

    rename(&opened, "daily", "Journal");
    let renamed = opened.workspace.tree().unwrap();
    assert!(!Arc::ptr_eq(&first, &renamed));
    assert_eq!(title(&opened, "daily"), "Journal");
}

#[test]
fn an_append_racing_the_projection_is_never_hidden_by_the_memo() {
    for point in [HoldPoint::TreeBeforeLoad, HoldPoint::TreeBeforeInstall] {
        let dir = tempfile::tempdir().unwrap();
        let gate = Arc::new(Gate::default());
        let opened = open_with(
            Store::open(dir.path()).unwrap(),
            index_path(dir.path()),
            Config::DEFAULT,
            gate.clone(),
        );
        opened.append(&index_fixture());
        gate.arm(point.clone());
        let raced = std::thread::scope(|scope| {
            let projection = scope.spawn(|| opened.workspace.tree().unwrap());
            gate.wait_held(&point);
            rename(&opened, "daily", "Journal");
            gate.release();
            projection.join().unwrap()
        });
        let raced_title = raced
            .pages
            .iter()
            .find(|p| p.id == page("daily"))
            .unwrap()
            .title
            .clone();
        assert!(
            ["Daily", "Journal"].contains(&raced_title.as_str()),
            "{point:?}: {raced_title}"
        );
        assert_eq!(title(&opened, "daily"), "Journal", "{point:?}");
        assert_eq!(title(&opened, "daily"), "Journal", "{point:?}");
    }
}

#[test]
fn a_tree_installed_just_before_an_append_is_replaced_on_the_next_call() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    assert_eq!(title(&opened, "daily"), "Daily");
    rename(&opened, "daily", "Journal");
    assert_eq!(title(&opened, "daily"), "Journal");
    opened.edit_records(|records| {
        records.get_mut(&page("daily")).unwrap().trash = Some(crate::workspace::records::Trash {
            original_parent: None,
            original_order: 1,
            batch_id: "b".into(),
            trashed_at: 1,
        });
    });
    assert_eq!(
        title(&opened, "daily"),
        "",
        "a trashed page leaves the tree at once"
    );
}

#[test]
fn concurrent_projections_and_appends_end_on_the_last_title() {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    opened.append(&index_fixture());
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..50 {
                    opened.workspace.tree().unwrap();
                }
            });
        }
        for n in 0..20 {
            rename(&opened, "daily", &format!("Daily {n}"));
        }
    });
    assert_eq!(title(&opened, "daily"), "Daily 19");
}
