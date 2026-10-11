use std::path::{Path, PathBuf};

use super::{Env, doc, soon};
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
