use std::time::{Duration, Instant};
use yrs::{ReadTxn, Transact};

use super::doc_of;
use crate::storage::{DocId, Store};
use crate::workspace::fixture::{Editor, erase_page};
use crate::workspace::records::read_pages;
use crate::workspace::{PageEdit, Workspace, body};

fn open_seeded(path: &std::path::Path) -> Workspace {
    let mut prepared = Workspace::prepare(path).unwrap();
    prepared.seed_getting_started(Instant::now() + Duration::from_secs(5));
    prepared.start(|_| {})
}

fn flush(w: &Workspace) {
    assert_eq!(
        w.flush(Instant::now() + Duration::from_secs(5)),
        crate::storage::Pending::default()
    );
}
fn assert_seed(w: &Workspace) -> String {
    let tree = w.tree().unwrap();
    assert_eq!(tree.pages.len(), 1);
    assert_eq!(tree.pages[0].title, "Getting started");
    let page = &tree.pages[0].id;
    let doc = doc_of(&w.load(page.doc_id()).unwrap());
    let body = body::read_body(&doc.transact());
    assert_eq!(body.problem, None);
    assert_eq!(
        body.blocks
            .iter()
            .map(|b| b.id.as_str())
            .collect::<Vec<_>>(),
        [
            "getting-started-0",
            "getting-started-1",
            "getting-started-2"
        ]
    );
    assert_eq!(
        body.blocks
            .into_iter()
            .map(|b| b.segments)
            .collect::<Vec<_>>(),
        [
            vec![body::Segment::Text("⌘K to search and run commands".into())],
            vec![body::Segment::Text("/ to insert a block".into())],
            vec![body::Segment::Text("[[ to link a page".into())],
        ]
    );
    page.to_string()
}

#[test]
fn a_new_folder_opened_twice_has_one_getting_started_page_with_the_same_id_and_three_lines() {
    let dir = tempfile::tempdir().unwrap();
    let first = open_seeded(dir.path());
    let id = assert_seed(&first);
    flush(&first);
    drop(first);
    let second = open_seeded(dir.path());
    assert_eq!(assert_seed(&second), id);
    assert_eq!(
        read_pages(&doc_of(&second.load(&DocId::workspace()).unwrap()).transact())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn all_trashed_or_all_erased_workspaces_are_never_reseeded() {
    for erase in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let w = open_seeded(dir.path());
        let id = w.tree().unwrap().pages[0].id.clone();
        if erase {
            let mut editor = Editor::new(199);
            editor.apply(&DocId::workspace(), &w.load(&DocId::workspace()).unwrap());
            let update = editor.edit(&DocId::workspace(), |txn| erase_page(txn, &id));
            w.append(&DocId::workspace(), &update).unwrap();
        } else {
            w.edit_page(PageEdit::Trash { page: id.clone() }).unwrap();
        }
        flush(&w);
        drop(w);
        let w = open_seeded(dir.path());
        assert_eq!(w.tree().unwrap().pages, []);
        let records =
            read_pages(&doc_of(&w.load(&DocId::workspace()).unwrap()).transact()).unwrap();
        if erase {
            assert_eq!(records.len(), 0);
        } else {
            assert_eq!(records.len(), 1);
            assert!(records[&id].trash.is_some());
        }
    }
}

#[test]
fn a_crash_after_the_seed_body_but_before_the_record_reuses_the_body_and_page_id() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let paragraphs = [
        (
            body::BlockId::new("getting-started-0").unwrap(),
            "⌘K to search and run commands",
        ),
        (
            body::BlockId::new("getting-started-1").unwrap(),
            "/ to insert a block",
        ),
        (
            body::BlockId::new("getting-started-2").unwrap(),
            "[[ to link a page",
        ),
    ];
    store
        .append(
            &DocId::new("getting-started").unwrap(),
            &body::paragraphs_update(&paragraphs),
        )
        .unwrap();
    assert_eq!(
        store.flush(Instant::now() + Duration::from_secs(5)),
        crate::storage::Pending::default()
    );
    let before = store.load(&DocId::new("getting-started").unwrap()).unwrap();
    drop(store);
    let w = open_seeded(dir.path());
    assert_eq!(assert_seed(&w), "getting-started");
    let after = w.load(&DocId::new("getting-started").unwrap()).unwrap();
    assert_eq!(
        doc_of(&after).transact().state_vector(),
        doc_of(&before).transact().state_vector()
    );
    flush(&w);
}

#[test]
fn even_an_empty_stored_workspace_update_prevents_seeding() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    store.append(&DocId::workspace(), &[0, 0]).unwrap();
    assert_eq!(
        store.flush(Instant::now() + Duration::from_secs(5)),
        crate::storage::Pending::default()
    );
    drop(store);
    let w = open_seeded(dir.path());
    assert_eq!(w.tree().unwrap().pages, []);
}

#[test]
fn core_workspace_open_never_adds_a_page() {
    let dir = tempfile::tempdir().unwrap();
    let w = Workspace::open(dir.path(), |_| {}).unwrap();
    assert_eq!(w.tree().unwrap().pages, []);
}

#[test]
fn prepared_workspace_starts_no_index_reader_and_seeds_at_most_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = Workspace::prepare(dir.path()).unwrap();
    assert!(!dir.path().join("index.sqlite").exists());
    prepared.seed_getting_started(Instant::now() + Duration::from_secs(5));
    prepared.seed_getting_started(Instant::now() + Duration::from_secs(5));
    assert!(!dir.path().join("index.sqlite").exists());
    let w = prepared.start(|_| {});
    let id = assert_seed(&w);
    w.edit_page(PageEdit::Rename {
        page: crate::workspace::records::PageId::new(id).unwrap(),
        title: "renamed".into(),
    })
    .unwrap();
    assert_eq!(
        doc_of(&w.load(&DocId::workspace()).unwrap())
            .transact()
            .state_vector()
            .len(),
        1
    );
    assert_eq!(w.tree().unwrap().pages[0].title, "renamed");
}
