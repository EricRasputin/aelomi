use std::path::{Path, PathBuf};

use yrs::types::ToJson;
use yrs::{
    Any, Doc, GetString, Map, MapRef, Transact, XmlElementPrelim, XmlFragment, XmlFragmentRef,
    XmlOut, XmlTextPrelim,
};

use super::{Env, doc, soon};
use crate::storage::doc_id::DocId;
use crate::storage::log::{self, LogEnd, LogFormat};
use crate::storage::sim_fs::OpKind;
use crate::storage::store::Store;
use crate::storage::testkit::doc_from;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data-folders/format-1")
}

struct Editor {
    doc: Doc,
    content: XmlFragmentRef,
    meta: MapRef,
}

impl Editor {
    fn new(client: u64) -> Self {
        let doc = Doc::with_client_id(client);
        let content = doc.get_or_insert_xml_fragment("content");
        let meta = doc.get_or_insert_map("meta");
        Self { doc, content, meta }
    }

    fn paragraph(&self, text: &str) -> Vec<u8> {
        let mut txn = self.doc.transact_mut();
        let len = self.content.len(&txn);
        let p = self
            .content
            .insert(&mut txn, len, XmlElementPrelim::empty("paragraph"));
        p.push_back(&mut txn, XmlTextPrelim::new(text));
        txn.encode_update_v1()
    }

    fn extend_first_paragraph(&self, text: &str) -> Vec<u8> {
        let mut txn = self.doc.transact_mut();
        let Some(XmlOut::Element(p)) = self.content.get(&txn, 0) else {
            panic!("a paragraph")
        };
        let Some(XmlOut::Text(t)) = p.get(&txn, 0) else {
            panic!("its text")
        };
        let len = yrs::Text::len(&t, &txn);
        yrs::Text::insert(&t, &mut txn, len, text);
        txn.encode_update_v1()
    }

    fn set(&self, key: &str, value: impl Into<Any>) -> Vec<u8> {
        let mut txn = self.doc.transact_mut();
        self.meta.insert(&mut txn, key, value.into());
        txn.encode_update_v1()
    }
}

fn write_fixture(target: &Path) {
    let env = Env::manual();
    let store = env.open();
    let (page, workspace) = (doc("page-1"), DocId::workspace());
    let editor = Editor::new(1);
    let append = |d: &DocId, update: Vec<u8>| store.append(d, &update).unwrap();

    let owner = Editor::new(2);
    append(&workspace, owner.set("name", "Fixture workspace"));
    append(&workspace, owner.set("pages", 1));
    append(&page, editor.paragraph("Storage keeps every edit."));
    append(&page, editor.paragraph("Snapshots replace old logs."));
    assert_eq!(store.flush(soon()), []);
    store.compact(&page).unwrap();

    append(&page, editor.set("status", "draft"));
    append(&page, editor.extend_first_paragraph(" Even after a crash."));
    assert_eq!(store.flush(soon()), []);

    env.fs
        .fail(OpKind::Create, "snapshot-0000000002", libc::EIO);
    assert!(
        store.compact(&page).is_err(),
        "rotated to generation 2, then failed"
    );
    env.fs.clear_faults();
    append(&page, editor.paragraph("This line is in generation two."));
    append(&page, editor.set("status", "final"));
    assert_eq!(store.flush(soon()), []);

    env.fs
        .fail(OpKind::Append, "log-0000000002-0000000000", libc::EIO);
    append(&page, editor.paragraph("This write was torn."));
    store.run_writer_once();

    if target.exists() {
        std::fs::remove_dir_all(target).unwrap();
    }
    for (path, bytes) in env.fs.files() {
        let relative = path.strip_prefix(env.root()).unwrap();
        if relative == Path::new(".lock") {
            continue;
        }
        let out = target.join(relative);
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        std::fs::write(out, bytes).unwrap();
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn the_format_1_fixture_opens_with_its_exact_contents() {
    if std::env::var_os("AELOMI_WRITE_FIXTURES").is_some_and(|v| v == "1") {
        write_fixture(&fixture());
    }
    let page_dir = fixture().join("docs/page-1");
    let mut files: Vec<String> = std::fs::read_dir(&page_dir)
        .expect("the fixture exists; run AELOMI_WRITE_FIXTURES=1 cargo test -p aelomi-core fixture")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(
        files,
        [
            "log-0000000001-0000000000.ylog",
            "log-0000000002-0000000000.ylog",
            "snapshot-0000000001.ysnap",
        ]
    );
    let torn = std::fs::read(page_dir.join("log-0000000002-0000000000.ylog")).unwrap();
    assert_eq!(
        log::read(LogFormat::V1, &torn).end,
        LogEnd::Torn,
        "the fixture keeps its torn tail"
    );
    assert!(!fixture().join(".lock").exists(), "no .lock is committed");

    let copy = tempfile::tempdir().unwrap();
    copy_dir(&fixture(), copy.path());
    let store = Store::open(copy.path()).unwrap();
    let read = |d: &DocId| doc_from([store.load(d).unwrap().as_slice()]);

    let page = read(&doc("page-1"));
    let (content, meta) = (
        page.get_or_insert_xml_fragment("content"),
        page.get_or_insert_map("meta"),
    );
    let txn = page.transact();
    assert_eq!(
        content.get_string(&txn),
        "<paragraph>Storage keeps every edit. Even after a crash.</paragraph>\
         <paragraph>Snapshots replace old logs.</paragraph>\
         <paragraph>This line is in generation two.</paragraph>"
    );
    assert_eq!(
        meta.to_json(&txn),
        Any::from_json(r#"{"status": "final"}"#).unwrap()
    );
    drop(txn);

    let workspace = read(&DocId::workspace());
    let meta = workspace.get_or_insert_map("meta");
    assert_eq!(
        meta.to_json(&workspace.transact()),
        Any::from_json(r#"{"name": "Fixture workspace", "pages": 1}"#).unwrap()
    );
}
