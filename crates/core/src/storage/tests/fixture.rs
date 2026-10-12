use std::path::{Path, PathBuf};

use yrs::types::ToJson;
use yrs::{Any, GetString, Transact};

use super::doc;
use crate::storage::doc_id::DocId;
use crate::storage::log::{self, LogEnd, LogFormat};
use crate::storage::store::Store;
use crate::storage::testkit::doc_from;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data-folders/format-1")
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
    let page_dir = fixture().join("docs/page-1");
    let mut files: Vec<String> = std::fs::read_dir(&page_dir)
        .expect("the format-1 fixture is committed")
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
