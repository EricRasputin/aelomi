use std::path::{Path, PathBuf};

use super::{Env, assert_doc, doc, soon};
use crate::storage::sim_fs::{Crash, OpKind};
use crate::storage::testkit::{UpdateGen, doc_from, fingerprint};

#[test]
fn a_retried_open_syncs_the_entries_a_failed_open_left_unsynced() {
    for (root, failing) in [
        ("/data", "/"),
        ("/data", "/data"),
        ("/home/me/data", "/home"),
    ] {
        let mut env = Env::manual();
        env.root = PathBuf::from(root);
        env.fs.fail_at(OpKind::Fsync, Path::new(failing), libc::EIO);
        assert!(env.try_open().is_err(), "{root}: syncing {failing} fails");
        env.fs.clear_faults();

        let store = env.open();
        let sent = UpdateGen::new(61, 2).updates(3);
        for update in &sent {
            store.append(&doc("a"), update).unwrap();
        }
        assert_eq!(store.flush(soon()), [], "{root}");
        drop(store);
        env.fs.crash(Crash::DropUnsynced);

        let loaded = doc_from([env.open().load(&doc("a")).unwrap().as_slice()]);
        assert_eq!(
            fingerprint(&loaded),
            fingerprint(&doc_from(sent.iter().map(Vec::as_slice))),
            "{root}: acknowledged updates survive after syncing {failing} once failed"
        );
    }
}

#[test]
fn a_doc_folder_an_earlier_session_left_unsynced_is_synced_before_an_acknowledgement() {
    let env = Env::manual();
    let a = doc("a");
    let mut updates = UpdateGen::new(62, 2);
    let (first, second) = (updates.next_update(), updates.next_update());

    let store = env.open();
    env.fs.fail_at(OpKind::Fsync, &env.path("docs"), libc::EIO);
    store.append(&a, &first).unwrap();
    assert_eq!(
        store.flush(soon()),
        std::slice::from_ref(&a),
        "docs/ fails to sync"
    );
    drop(store);
    env.fs.clear_faults();

    let store = env.open();
    store.append(&a, &second).unwrap();
    assert_eq!(store.flush(soon()), []);
    drop(store);
    env.fs.crash(Crash::DropUnsynced);
    assert_doc(&env.open(), &a, &[first, second]);
}
