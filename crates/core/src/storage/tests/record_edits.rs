use yrs::updates::decoder::Decode;
use yrs::{GetString, ReadTxn, Text, Transact, Update, WriteTxn};

use super::{Env, doc_from, soon};
use crate::storage::DocId;
use crate::storage::layout::Layout;
use crate::storage::recover::stored_updates;
use crate::storage::sim_fs::Crash;
use crate::workspace::fixture::Editor;
use crate::workspace::records::read_pages;
use crate::workspace::{PageEdit, Workspace};

#[test]
fn a_record_edit_never_reencodes_pending_foreign_operations_and_they_survive_reopen() {
    let env = Env::manual();
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::with_store(
        env.open(),
        std::fs::canonicalize(dir.path())
            .unwrap()
            .join("index.sqlite"),
        |_| {},
    );
    let page = workspace
        .edit_page(PageEdit::Create {
            parent: None,
            title: "Before".into(),
        })
        .unwrap()
        .created_page()
        .unwrap();
    let id = DocId::workspace();
    let mut foreign = Editor::new(9001);
    let dependency = foreign.edit(&id, |txn| {
        txn.get_or_insert_text("future")
            .insert(txn, 0, "dependency ");
    });
    let pending = foreign.edit(&id, |txn| {
        let text = txn.get_or_insert_text("future");
        let end = text.len(txn);
        text.insert(txn, end, "arrives");
    });
    workspace.append(&id, &pending).unwrap();
    workspace
        .edit_page(PageEdit::Rename {
            page: page.clone(),
            title: "After".into(),
        })
        .unwrap();
    assert_eq!(workspace.flush(soon()), []);
    let stored = stored_updates(&*env.fs, &Layout::new(env.root()), &id).unwrap();
    let accepted = Update::decode_v1(stored.updates.last().unwrap()).unwrap();
    assert_eq!(
        accepted.state_vector_lower().get(&yrs::ClientID::new(9001)),
        0,
        "the record edit carries no pending operations from client 9001"
    );
    drop(workspace);
    env.fs.crash(Crash::DropUnsynced);
    let store = env.open();
    store.append(&id, &dependency).unwrap();
    assert_eq!(store.flush(soon()), []);
    let doc = doc_from([store.load(&id).unwrap().as_slice()]);
    let txn = doc.transact();
    assert_eq!(read_pages(&txn).unwrap()[&page].title, "After");
    assert_eq!(
        txn.get_text("future").unwrap().get_string(&txn),
        "dependency arrives"
    );
}
