use super::{Env, v2_log};
use crate::storage::fs::Fs;
use crate::storage::layout::Layout;
use crate::storage::sim_fs::OpKind;
use crate::storage::store::{DestinationFolder, destination_folder};
use crate::workspace::Workspace;

#[test]
fn an_empty_frame_can_land_after_initialization_flush_reports_pending() {
    let env = Env::threaded();
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = Workspace::prepare_with_store(env.open(), dir.path().join("index.sqlite"));
    env.fs
        .set_latency(OpKind::FullSync, std::time::Duration::from_millis(100));
    let error = prepared
        .initialize_empty(std::time::Instant::now() + std::time::Duration::from_millis(5))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "could not finish creating the empty workspace"
    );
    drop(prepared);
    assert_eq!(
        destination_folder(env.fs.as_ref(), &Layout::new(env.root())).unwrap(),
        DestinationFolder::Workspace
    );
    env.fs
        .set_latency(OpKind::FullSync, std::time::Duration::ZERO);
    let workspace = Workspace::with_store(env.open(), dir.path().join("index.sqlite"), |_| {});
    assert_eq!(
        workspace.load(&crate::storage::DocId::workspace()).unwrap(),
        [0, 0]
    );
    assert_eq!(workspace.tree().unwrap().pages.len(), 0);
}

#[test]
fn failed_marker_publication_leaves_a_retryable_empty_destination() {
    let env = Env::manual();
    let layout = Layout::new(env.root());
    env.fs.fail(OpKind::Append, "format.json.tmp", libc::ENOSPC);
    assert!(
        matches!(env.try_open(), Err(crate::storage::OpenError::Io(error)) if error.raw_os_error() == Some(libc::ENOSPC))
    );
    assert_eq!(
        env.fs.read(&env.path("format.json.tmp")).unwrap(),
        b"{\n  \"form"
    );
    let before = env.fs.files();
    assert_eq!(
        destination_folder(env.fs.as_ref(), &layout).unwrap(),
        DestinationFolder::Empty
    );
    assert_eq!(env.fs.files(), before);
    env.fs.clear_faults();
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = Workspace::prepare_with_store(env.open(), dir.path().join("index.sqlite"));
    prepared.initialize_empty(super::soon()).unwrap();
    let workspace = prepared.start(|_| {});
    assert_eq!(workspace.tree().unwrap().pages.len(), 0);
    assert_eq!(
        destination_folder(env.fs.as_ref(), &layout).unwrap(),
        DestinationFolder::Workspace
    );
}

#[test]
fn torn_first_root_frame_is_retryable_without_changing_it_during_classification() {
    let env = Env::manual();
    drop(env.open());
    let frame = v2_log(&[vec![0, 0]]);
    let path = env.path("docs/workspace/log-0000000000-0000000000.ylog2");
    env.fs.put_durable(&path, &frame[..21]);
    let before = env.fs.files();
    assert_eq!(
        destination_folder(env.fs.as_ref(), &Layout::new(env.root())).unwrap(),
        DestinationFolder::Empty
    );
    assert_eq!(env.fs.files(), before);
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = Workspace::prepare_with_store(env.open(), dir.path().join("index.sqlite"));
    prepared.initialize_empty(super::soon()).unwrap();
    let workspace = prepared.start(|_| {});
    assert_eq!(
        workspace.load(&crate::storage::DocId::workspace()).unwrap(),
        [0, 0]
    );
    assert_eq!(workspace.tree().unwrap().pages.len(), 0);
}

#[test]
fn blocking_names_exclude_metadata_and_publication_skeleton() {
    for marker in [None, Some(b"{\"format\":\"broken\"}".as_slice())] {
        let env = Env::manual();
        for (name, bytes) in [
            (".DS_Store", b"Finder".as_slice()),
            ("Icon\r", b"Finder"),
            (".lock", b""),
            ("format.json.tmp", b"torn marker"),
            ("Aelomi Notes", b"pages"),
        ] {
            env.fs.put_durable(&env.path(name), bytes);
        }
        if let Some(marker) = marker {
            env.fs.put_durable(&env.path("format.json"), marker);
        }
        assert_eq!(
            destination_folder(env.fs.as_ref(), &Layout::new(env.root())).unwrap(),
            if marker.is_some() {
                DestinationFolder::Unrecognized {
                    items: vec!["Aelomi Notes".into()],
                }
            } else {
                DestinationFolder::NotEmpty {
                    items: vec!["Aelomi Notes".into()],
                }
            }
        );
        assert_eq!(env.fs.read(&env.path("Aelomi Notes")).unwrap(), b"pages");
    }
}

#[test]
fn destination_classification_is_read_only_and_uses_workspace_content() {
    let env = Env::manual();
    let layout = Layout::new(env.root());
    assert_eq!(
        destination_folder(env.fs.as_ref(), &layout).unwrap(),
        DestinationFolder::Absent
    );
    env.fs.create_dir(env.root()).unwrap();
    for name in [".DS_Store", ".localized", "Icon\r"] {
        env.fs.put_durable(&env.path(name), b"Finder");
    }
    assert_eq!(
        destination_folder(env.fs.as_ref(), &layout).unwrap(),
        DestinationFolder::Empty
    );
    env.fs
        .put_durable(&env.path("format.json"), b"{\"format\":1}");
    env.fs.create_dir(&env.path("docs")).unwrap();
    env.fs.create_dir(&env.path("docs/workspace")).unwrap();
    env.fs.put_durable(&env.path(".lock"), b"");
    env.fs.put_durable(
        &env.path("docs/workspace/log-0000000000-0000000000.ylog2"),
        b"",
    );
    let before = env.fs.files();
    assert_eq!(
        destination_folder(env.fs.as_ref(), &layout).unwrap(),
        DestinationFolder::Empty
    );
    assert_eq!(env.fs.files(), before);
    env.fs.put_durable(
        &env.path("docs/workspace/log-0000000000-0000000000.ylog2"),
        &v2_log(&[vec![0, 0]]),
    );
    let before = env.fs.files();
    assert_eq!(
        destination_folder(env.fs.as_ref(), &layout).unwrap(),
        DestinationFolder::Workspace
    );
    assert_eq!(env.fs.files(), before);
}

#[test]
fn real_items_and_unrecognized_formats_stay_distinct() {
    let env = Env::manual();
    let layout = Layout::new(env.root());
    env.fs.put_durable(&env.path("notes.txt"), b"pages");
    assert_eq!(
        destination_folder(env.fs.as_ref(), &layout).unwrap(),
        DestinationFolder::NotEmpty {
            items: vec!["notes.txt".into()]
        }
    );
    env.fs.put_durable(&env.path("docs"), b"a file named docs");
    let before = env.fs.files();
    assert_eq!(
        destination_folder(env.fs.as_ref(), &layout).unwrap(),
        DestinationFolder::Unrecognized {
            items: vec!["docs".into(), "notes.txt".into()]
        }
    );
    assert_eq!(env.fs.files(), before);
}
