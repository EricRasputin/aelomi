use super::{Env, v2_log};
use crate::storage::fs::Fs;
use crate::storage::layout::Layout;
use crate::storage::store::{DestinationFolder, destination_folder};

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
