use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::Env;
use crate::storage::sim_fs::OpKind;
use crate::storage::store::{OpenError, Store};

fn tree(dir: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.insert(path.clone(), None);
            out.extend(tree(&path));
        } else {
            out.insert(path.clone(), Some(std::fs::read(&path).unwrap()));
        }
    }
    out
}

fn folder(files: &[(&str, &[u8])]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in files {
        let path = dir.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    dir
}

#[test]
fn a_second_open_of_the_same_folder_is_locked() {
    let dir = tempfile::tempdir().unwrap();
    let first = Store::open(dir.path()).unwrap();
    assert!(matches!(Store::open(dir.path()), Err(OpenError::Locked)));
    drop(first);
    Store::open(dir.path()).expect("the lock is released when the store drops");
}

#[test]
fn a_newer_format_is_refused_and_the_folder_is_left_byte_for_byte_unchanged() {
    let dir = folder(&[
        (
            "format.json",
            b"{\"format\": 2, \"migrated\": {\"from\": [1]}}",
        ),
        (
            "docs/a/log-0000000000-0000000000.ylog",
            b"AELOG\0\0\x02 future bytes",
        ),
        ("images/1f3a.png", b"png"),
    ]);
    let before = tree(dir.path());
    let refused = Store::open(dir.path()).map(drop);
    assert!(
        matches!(
            refused,
            Err(OpenError::NewerFormat {
                found: 2,
                supported: 1
            })
        ),
        "{refused:?}"
    );
    assert_eq!(tree(dir.path()), before, "not even a .lock");

    let env = Env::manual();
    env.fs
        .put_durable(&env.path("format.json"), b"{\"format\": 3}");
    let before = env.fs.ops().len();
    assert!(matches!(
        env.try_open(),
        Err(OpenError::NewerFormat {
            found: 3,
            supported: 1
        })
    ));
    let kinds: Vec<OpKind> = env.fs.ops()[before..].iter().map(|op| op.kind).collect();
    assert_eq!(kinds, [OpKind::List, OpKind::Read], "only reads");
}

#[test]
fn a_folder_with_data_but_no_readable_format_is_refused() {
    for files in [
        &[("docs/a/log-0000000000-0000000000.ylog", &b"x"[..])][..],
        &[
            ("format.json", b"{\"format\": \"one\"}"),
            ("docs/a/x", b"x"),
        ],
        &[("format.json", b"")],
        &[("format.json", b"{\"format\": 0}")],
    ] {
        let dir = folder(files);
        let before = tree(dir.path());
        let refused = Store::open(dir.path()).map(drop);
        assert!(
            matches!(refused, Err(OpenError::UnknownFormat)),
            "{files:?}: {refused:?}"
        );
        assert_eq!(tree(dir.path()), before, "{files:?}");
    }
}

#[test]
fn a_new_or_empty_folder_gets_format_json_and_docs() {
    let parent = tempfile::tempdir().unwrap();
    for root in [parent.path().join("new/data"), parent.path().to_path_buf()] {
        drop(Store::open(&root).unwrap());
        let names: Vec<String> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n != "new")
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(names, [".lock", "docs", "format.json"], "{root:?}");
        assert_eq!(
            std::fs::read_to_string(root.join("format.json")).unwrap(),
            "{\n  \"format\": 1\n}\n"
        );
        drop(Store::open(&root).expect("reopens"));
    }
}

#[test]
fn a_format_json_left_half_written_by_a_crash_is_redone() {
    let env = Env::manual();
    env.fs.put_durable(&env.path("format.json.tmp"), b"{\"for");
    drop(env.open());
    let files = env.fs.files();
    assert_eq!(
        files.get(&env.path("format.json")).map(Vec::as_slice),
        Some(&b"{\n  \"format\": 1\n}\n"[..])
    );
    assert!(!files.contains_key(&env.path("format.json.tmp")));
}
