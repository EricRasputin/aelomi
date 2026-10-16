use std::sync::Arc;
use std::time::Duration;

use yrs::Transact;

use super::doc_of;
use crate::storage::clock::{Clock, ManualClock};
use crate::storage::{DocId, Driver, Limits, OpKind, Seq, SimFs, Store};
use crate::workspace::fixture::{Editor, insert_block, paragraph, remove_block, text};
use crate::workspace::{PageEdit, Workspace, body};

#[test]
fn typing_paste_drag_rename_move_trash_and_restore_are_flushed_within_100_ms() {
    let clock = Arc::new(ManualClock::default());
    let fs = SimFs::new(clock.clone());
    fs.set_latency(OpKind::FullSync, Duration::from_millis(6));
    fs.set_latency(OpKind::Fsync, Duration::from_micros(500));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open_with(
        std::path::Path::new("/data"),
        fs.clone(),
        clock.clone(),
        Driver::Manual,
        Limits::DATA_6,
    )
    .unwrap();
    let w = Workspace::with_store(
        store,
        std::fs::canonicalize(dir.path())
            .unwrap()
            .join("index.sqlite"),
        |_| {},
    );
    let ack = |name: &str, doc: &DocId, seq: Seq, applied: Duration| {
        w.store.run_writer_once();
        assert!(w.store.is_durable(doc, seq), "{name} was acknowledged");
        assert!(
            clock.now() - applied <= Duration::from_millis(100),
            "{name} exceeded SHELL-9"
        );
    };
    let start = clock.now();
    let create = w
        .edit_page(PageEdit::Create {
            parent: None,
            title: "A".into(),
        })
        .unwrap();
    ack("create", &DocId::workspace(), create.seq, start);
    let page = create.created_page().unwrap();
    let mut editor = Editor::new(180);
    for name in ["typing", "paste", "drag"] {
        let start = clock.now();
        let update = editor.edit(page.doc_id(), |txn| match name {
            "typing" => insert_block(txn, 0, &paragraph("typed", vec![text("typed text")])),
            "paste" => insert_block(txn, 1, &paragraph("pasted", vec![text("pasted text")])),
            "drag" => {
                remove_block(txn, 0);
                insert_block(txn, 1, &paragraph("typed", vec![text("typed text")]));
            }
            _ => unreachable!(),
        });
        let seq = w.append(page.doc_id(), &update).unwrap();
        ack(name, page.doc_id(), seq, start);
    }
    for (name, edit) in [
        (
            "rename",
            PageEdit::Rename {
                page: page.clone(),
                title: "renamed".into(),
            },
        ),
        (
            "move",
            PageEdit::Move {
                page: page.clone(),
                parent: None,
                position: 0,
            },
        ),
    ] {
        let start = clock.now();
        let reply = w.edit_page(edit).unwrap();
        ack(name, &DocId::workspace(), reply.seq, start);
    }
    let start = clock.now();
    let trash = w.edit_page(PageEdit::Trash { page: page.clone() }).unwrap();
    ack("trash", &DocId::workspace(), trash.seq, start);
    let start = clock.now();
    let restore = w
        .edit_page(PageEdit::Restore {
            batch: trash.trash_batch().unwrap(),
        })
        .unwrap();
    ack("restore", &DocId::workspace(), restore.seq, start);
    let before = w.load(&DocId::workspace()).unwrap();
    drop(w);
    fs.crash(crate::storage::Crash::DropUnsynced);
    let store = Store::open_with(
        std::path::Path::new("/data"),
        fs,
        clock,
        Driver::Manual,
        Limits::DATA_6,
    )
    .unwrap();
    let read = body::read_body(&doc_of(&store.load(page.doc_id()).unwrap()).transact());
    assert_eq!(
        read.blocks
            .iter()
            .map(|b| b.id.as_str())
            .collect::<Vec<_>>(),
        ["pasted", "typed"]
    );
    assert_eq!(
        read.blocks
            .into_iter()
            .map(|b| b.segments)
            .collect::<Vec<_>>(),
        [
            vec![body::Segment::Text("pasted text".into())],
            vec![body::Segment::Text("typed text".into())],
        ]
    );
    let records = crate::workspace::records::read_pages(
        &doc_of(&store.load(&DocId::workspace()).unwrap()).transact(),
    )
    .unwrap();
    assert_eq!(
        records,
        crate::workspace::records::read_pages(&doc_of(&before).transact()).unwrap()
    );
    assert_eq!(records[&page].title, "renamed");
    assert_eq!(records[&page].trash, None);
}

#[test]
fn rename_move_trash_and_restore_replays_preserve_records_with_a_later_clock() {
    let clock = Arc::new(ManualClock::default());
    let fs = SimFs::new(clock.clone());
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open_with(
        std::path::Path::new("/data"),
        fs,
        clock.clone(),
        Driver::Manual,
        Limits::DATA_6,
    )
    .unwrap();
    let w = Workspace::with_store(store, dir.path().join("index.sqlite"), |_| {});
    let parent = w
        .edit_page(PageEdit::Create {
            parent: None,
            title: "Parent".into(),
        })
        .unwrap()
        .created_page()
        .unwrap();
    let page = w
        .edit_page(PageEdit::Create {
            parent: None,
            title: "Before".into(),
        })
        .unwrap()
        .created_page()
        .unwrap();
    let check = |edit: PageEdit| {
        w.edit_page(edit.clone()).unwrap();
        let before = crate::workspace::records::read_pages(
            &doc_of(&w.load(&DocId::workspace()).unwrap()).transact(),
        )
        .unwrap();
        clock.advance(Duration::from_secs(10));
        w.edit_page(edit).unwrap();
        let after = crate::workspace::records::read_pages(
            &doc_of(&w.load(&DocId::workspace()).unwrap()).transact(),
        )
        .unwrap();
        assert_eq!(after, before);
    };
    check(PageEdit::Rename {
        page: page.clone(),
        title: "After".into(),
    });
    check(PageEdit::Move {
        page: page.clone(),
        parent: Some(parent),
        position: 0,
    });
    check(PageEdit::Trash { page: page.clone() });
    let batch = w
        .edit_page(PageEdit::Trash { page })
        .unwrap()
        .trash_batch()
        .unwrap();
    check(PageEdit::Restore { batch });
}
