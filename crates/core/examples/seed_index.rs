//! Seeds a new data folder with the index fixture and leaves its index in a chosen state, for
//! native checks of SHELL-12. Refuses a folder that already holds anything.
//!
//! cargo run -p aelomi-core --features test-support --example seed_index -- <folder> <state>
//!
//! `<state>` is `missing`, `garbage`, `old-schema`, or `valid`. Launch the app with
//! `AELOMI_DATA_DIR=<folder>`; the shell log shows `index cache=…` and `index status=…`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use aelomi_core::index::IndexStatus;
use aelomi_core::workspace::Workspace;
use aelomi_core::workspace::fixture::{BrokenIndex, break_index, index_fixture};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [folder, state] = args.as_slice() else {
        fail("usage: seed_index <folder> <missing|garbage|old-schema|valid>");
    };
    let broken = match state.as_str() {
        "missing" => Some(BrokenIndex::Missing),
        "garbage" => Some(BrokenIndex::Garbage),
        "old-schema" => Some(BrokenIndex::OldSchema),
        "valid" => None,
        other => fail(&format!("unknown state {other:?}")),
    };
    let folder = PathBuf::from(folder);
    if std::fs::read_dir(&folder).is_ok_and(|mut entries| entries.next().is_some()) {
        fail(&format!(
            "{} is not empty; seed a new folder",
            folder.display()
        ));
    }
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| fail(&e.to_string()));
    let folder = std::fs::canonicalize(&folder).unwrap_or_else(|e| fail(&e.to_string()));

    let workspace = Workspace::open(&folder, |_| {}).unwrap_or_else(|e| fail(&e.to_string()));
    for (doc, update) in index_fixture() {
        workspace
            .append(&doc, &update)
            .unwrap_or_else(|e| fail(&e.to_string()));
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    while workspace.index_status() != IndexStatus::Ready {
        if Instant::now() > deadline {
            fail(&format!("the index is {:?}", workspace.index_status()));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let pending = workspace.flush(Instant::now() + Duration::from_secs(10));
    if !pending.is_empty() {
        fail(&format!("could not flush {pending:?}"));
    }
    drop(workspace);

    if let Some(broken) = broken {
        break_index(&folder.join(aelomi_core::index::FILE), broken)
            .unwrap_or_else(|e| fail(&e.to_string()));
    }
    println!("seeded {} with index state {state}", folder.display());
    println!(
        "active pages: Projects > Aelomi > Roadmap, Projects > Reading list, Daily, (untitled)"
    );
    println!("trash: Archive (with Old notes); favorites: Reading list, Aelomi");
}

fn fail(message: &str) -> ! {
    eprintln!("seed_index: {message}");
    std::process::exit(1);
}
