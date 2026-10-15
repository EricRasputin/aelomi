use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use yrs::types::ToJson;
use yrs::{
    Any, Doc, GetString, Map, MapRef, ReadTxn, StateVector, Transact, XmlElementPrelim,
    XmlFragment, XmlFragmentRef, XmlOut, XmlTextPrelim,
};

use super::{
    ATTRIBUTED_PAGE, Env, attributed_page, doc, log_name, render_body, soon, v1_snapshot_name,
};
use crate::storage::doc_id::DocId;
use crate::storage::log::{self, LogEnd, LogFormat, encode_segment};
use crate::storage::sim_fs::{Crash, OpKind};
use crate::storage::snapshot::{self, Condition, SnapshotFormat, encode_snapshot};
use crate::storage::state::Limits;
use crate::storage::store::Store;
use crate::storage::testkit::{apply, doc_from};

fn fixture() -> PathBuf {
    data_folder(1)
}

fn data_folder(format: u64) -> PathBuf {
    let folders = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data-folders");
    folders.join(format!("format-{format}"))
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
    assert_eq!(contents(&store), Contents::fixture());
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

fn write_format_2_fixture(target: &Path) {
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

#[test]
fn the_format_2_fixture_opens_with_its_exact_contents() {
    if std::env::var_os("AELOMI_WRITE_FIXTURES").is_some_and(|v| v == "1") {
        write_format_2_fixture(&data_folder(2));
    }
    let fixture = data_folder(2);
    let committed = tree(&fixture);
    assert_eq!(
        committed.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "docs/page-1/log-0000000001-0000000000.ylog2",
            "docs/page-1/log-0000000002-0000000000.ylog2",
            "docs/page-1/snapshot-0000000001.ysnap2",
            "docs/workspace/log-0000000000-0000000000.ylog2",
            "format.json",
        ],
        "the format-2 fixture is committed; regenerate it with AELOMI_WRITE_FIXTURES=1"
    );
    assert_eq!(committed["format.json"], MARKER_2.as_bytes());
    let snapshot = &committed["docs/page-1/snapshot-0000000001.ysnap2"];
    assert_eq!(
        snapshot::read(SnapshotFormat::V2, snapshot).map(|(_, condition)| condition),
        Some(Condition::Whole)
    );
    let torn = &committed["docs/page-1/log-0000000002-0000000000.ylog2"];
    assert_eq!(
        log::read(LogFormat::V2, torn).end,
        LogEnd::Torn,
        "the fixture keeps its torn tail"
    );
    assert!(!fixture.join(".lock").exists(), "no .lock is committed");

    let copy = tempfile::tempdir().unwrap();
    copy_dir(&fixture, copy.path());
    let store = Store::open(copy.path()).unwrap();
    assert_eq!(contents(&store), Contents::fixture());
    assert_eq!(
        tree(copy.path()),
        committed,
        "opening and loading write nothing"
    );
}

const FIXTURE_PAGE: &str = "<paragraph>Storage keeps every edit. Even after a crash.</paragraph>\
     <paragraph>Snapshots replace old logs.</paragraph>\
     <paragraph>This line is in generation two.</paragraph>";
const MARKER_2: &str = "{\n  \"format\": 2\n}\n";
const WRITTEN: &str = "Written by format 2.";
const AFTER_CRASH: &str = "Written after the crash.";

#[derive(Debug, PartialEq)]
struct Contents {
    page: String,
    page_meta: Any,
    workspace_meta: Any,
}

impl Contents {
    fn fixture() -> Self {
        Self::with(&[], 1)
    }

    fn with(paragraphs: &[&str], pages: u32) -> Self {
        let added: String = paragraphs
            .iter()
            .map(|p| format!("<paragraph>{p}</paragraph>"))
            .collect();
        let workspace = format!(r#"{{"name": "Fixture workspace", "pages": {pages}}}"#);
        Self {
            page: format!("{FIXTURE_PAGE}{added}"),
            page_meta: Any::from_json(r#"{"status": "final"}"#).unwrap(),
            workspace_meta: Any::from_json(&workspace).unwrap(),
        }
    }
}

fn contents(store: &Store) -> Contents {
    let page = doc_from([store.load(&doc("page-1")).unwrap().as_slice()]);
    let (content, meta) = (
        page.get_or_insert_xml_fragment("content"),
        page.get_or_insert_map("meta"),
    );
    let workspace = doc_from([store.load(&DocId::workspace()).unwrap().as_slice()]);
    let workspace_meta = workspace.get_or_insert_map("meta");
    let txn = page.transact();
    Contents {
        page: content.get_string(&txn),
        page_meta: meta.to_json(&txn),
        workspace_meta: workspace_meta.to_json(&workspace.transact()),
    }
}

fn add_paragraph(state: &[u8], client: u64, text: &str) -> Vec<u8> {
    let editor = Doc::with_client_id(client);
    apply(&editor, state);
    let content = editor.get_or_insert_xml_fragment("content");
    let mut txn = editor.transact_mut();
    let len = content.len(&txn);
    let p = content.insert(&mut txn, len, XmlElementPrelim::empty("paragraph"));
    p.push_back(&mut txn, XmlTextPrelim::new(text));
    txn.encode_update_v1()
}

fn set_pages(state: &[u8], client: u64, pages: u32) -> Vec<u8> {
    let editor = Doc::with_client_id(client);
    apply(&editor, state);
    let meta = editor.get_or_insert_map("meta");
    let mut txn = editor.transact_mut();
    meta.insert(&mut txn, "pages", pages);
    txn.encode_update_v1()
}

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
                out.insert(relative.to_owned(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out.remove(".lock");
    out
}

fn assert_format_1_files_unchanged(files: &BTreeMap<String, Vec<u8>>, case: &str) {
    for (path, bytes) in tree(&fixture()) {
        if path != "format.json" {
            assert!(files.get(&path) == Some(&bytes), "{case}: {path} changed");
        }
    }
}

#[test]
fn appending_to_the_format_1_fixture_publishes_format_2_and_never_rewrites_a_format_1_file() {
    let copy = tempfile::tempdir().unwrap();
    copy_dir(&fixture(), copy.path());
    let (page, workspace) = (doc("page-1"), DocId::workspace());
    let store = Store::open(copy.path()).unwrap();
    assert_eq!(contents(&store), Contents::fixture());
    assert_eq!(
        tree(copy.path()),
        tree(&fixture()),
        "opening and loading write nothing"
    );

    let on_page = add_paragraph(&store.load(&page).unwrap(), 3, WRITTEN);
    let on_workspace = set_pages(&store.load(&workspace).unwrap(), 3, 2);
    store.append(&page, &on_page).unwrap();
    store.append(&workspace, &on_workspace).unwrap();
    store.close(soon()).unwrap();

    let mut written = tree(copy.path());
    let marker = written.remove("format.json").unwrap();
    let legacy = tree(&fixture());
    let added: Vec<&str> = written
        .keys()
        .filter(|path| !legacy.contains_key(*path))
        .map(String::as_str)
        .collect();
    assert_eq!(
        (String::from_utf8(marker).unwrap().as_str(), added),
        (
            MARKER_2,
            vec![
                "docs/page-1/log-0000000002-0000000001.ylog2",
                "docs/workspace/log-0000000000-0000000001.ylog2",
            ]
        )
    );
    assert_format_1_files_unchanged(&written, "after the first format-2 write");
    let store = Store::open(copy.path()).unwrap();
    assert_eq!(contents(&store), Contents::with(&[WRITTEN], 2));
}

fn simulated_fixture() -> Env {
    let env = Env::manual();
    for (path, bytes) in tree(&fixture()) {
        env.fs.put_durable(&env.path(&path), &bytes);
    }
    env
}

fn durable_tree(env: &Env) -> BTreeMap<String, Vec<u8>> {
    let files = env.fs.durable_files().into_iter();
    files
        .map(|(path, bytes)| {
            let relative = path.strip_prefix(env.root()).unwrap().to_str().unwrap();
            (relative.to_owned(), bytes)
        })
        .filter(|(path, _)| path != ".lock")
        .collect()
}

fn first_write(env: &Env, die_after: usize) -> ([bool; 2], usize) {
    let docs = [doc("page-1"), DocId::workspace()];
    let store = env.open();
    let updates = [
        add_paragraph(&store.load(&docs[0]).unwrap(), 3, WRITTEN),
        set_pages(&store.load(&docs[1]).unwrap(), 3, 2),
    ];
    let before = env.fs.ops().len();
    env.fs.die_after(die_after);
    let seqs = [
        store.append(&docs[0], &updates[0]).unwrap(),
        store.append(&docs[1], &updates[1]).unwrap(),
    ];
    store.run_writer_once();
    let acked = [
        store.is_durable(&docs[0], seqs[0]),
        store.is_durable(&docs[1], seqs[1]),
    ];
    (acked, env.fs.ops().len() - before)
}

#[test]
fn a_crash_anywhere_in_the_first_format_2_write_keeps_the_fixture_and_every_acknowledged_update() {
    let (_, ops) = first_write(&simulated_fixture(), usize::MAX);
    let modes = std::iter::once(Crash::DropUnsynced).chain((0..4).map(Crash::KeepTornPrefix));
    for mode in modes {
        for die_after in 0..=ops {
            let case = format!("{mode:?} after {die_after} of {ops} operations");
            let env = simulated_fixture();
            let (acked, _) = first_write(&env, die_after);
            env.fs.crash(mode);

            let durable = durable_tree(&env);
            let marker = String::from_utf8(durable["format.json"].clone()).unwrap();
            assert!(
                ["{\n  \"format\": 1\n}\n", MARKER_2].contains(&marker.as_str()),
                "{case}: {marker:?}"
            );
            let format_2_logs: Vec<&String> =
                durable.keys().filter(|p| p.ends_with(".ylog2")).collect();
            assert!(
                format_2_logs.is_empty() || marker == MARKER_2,
                "{case}: {format_2_logs:?} under {marker:?}"
            );
            assert_format_1_files_unchanged(&durable, &case);

            let store = env.open();
            let after_crash = contents(&store);
            let page_kept = after_crash.page != FIXTURE_PAGE;
            let pages_kept = after_crash.workspace_meta != Contents::fixture().workspace_meta;
            let paragraphs: &[&str] = if page_kept { &[WRITTEN] } else { &[] };
            assert_eq!(
                after_crash,
                Contents::with(paragraphs, if pages_kept { 2 } else { 1 }),
                "{case}"
            );
            assert!(
                (page_kept || !acked[0]) && (pages_kept || !acked[1]),
                "{case}: acknowledged {acked:?}, loaded {after_crash:?}"
            );

            let page = add_paragraph(&store.load(&doc("page-1")).unwrap(), 4, AFTER_CRASH);
            let workspace = set_pages(&store.load(&DocId::workspace()).unwrap(), 4, 3);
            store.append(&doc("page-1"), &page).unwrap();
            store.append(&DocId::workspace(), &workspace).unwrap();
            assert_eq!(store.flush(soon()), [], "{case}");
            let durable = durable_tree(&env);
            assert_eq!(
                (
                    String::from_utf8_lossy(&durable["format.json"]),
                    env.fs.files().contains_key(&env.path("format.json.tmp"))
                ),
                (MARKER_2.into(), false),
                "{case}"
            );
            assert_format_1_files_unchanged(&durable, &case);
            drop(store);
            let paragraphs: &[&str] = if page_kept {
                &[WRITTEN, AFTER_CRASH]
            } else {
                &[AFTER_CRASH]
            };
            assert_eq!(
                contents(&env.open()),
                Contents::with(paragraphs, 3),
                "{case}"
            );
        }
    }
}

#[test]
fn an_attributed_page_in_a_format_1_folder_compacts_to_its_exact_content() {
    let mut env = simulated_fixture();
    env.limits = Limits {
        records: 1,
        ..Limits::DATA_6
    };
    let page = doc("page-2");
    let sent = attributed_page();
    let first = doc_from(sent[..2].iter().map(Vec::as_slice));
    let state = first
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let dir = env.path("docs/page-2");
    let format_1_log = dir.join("log-0000000001-0000000000.ylog");
    env.fs
        .put_durable(&dir.join(v1_snapshot_name(1)), &encode_snapshot(&state));
    env.fs
        .put_durable(&format_1_log, &encode_segment(&sent[2..]));
    assert_eq!(
        log_name(1, 0),
        "log-0000000001-0000000000.ylog2",
        "the writer's name differs"
    );

    let store = env.open();
    let loaded = doc_from([store.load(&page).unwrap().as_slice()]);
    assert_eq!(render_body(&loaded), ATTRIBUTED_PAGE);
    assert_eq!(store.queued_compactions(), std::slice::from_ref(&page));
    let (compacted, result) = store.run_compactor_once().unwrap();
    assert_eq!(compacted, page);
    result.unwrap();
    assert_eq!(env.doc_files(&page), [v1_snapshot_name(2)]);
    assert_eq!(env.quarantine(), [].into());
    let durable = durable_tree(&env);
    assert_eq!(
        String::from_utf8_lossy(&durable["format.json"]),
        "{\n  \"format\": 1\n}\n",
        "compaction never changes format.json"
    );
    assert_format_1_files_unchanged(&durable, "after compaction");

    drop(store);
    let store = env.open();
    let loaded = doc_from([store.load(&page).unwrap().as_slice()]);
    assert_eq!(render_body(&loaded), ATTRIBUTED_PAGE);
    assert_eq!(store.queued_compactions(), []);
    assert_eq!(contents(&store), Contents::fixture());
}
