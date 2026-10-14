use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use aelomi_core::index::IndexStatus;
use aelomi_core::storage::{DocId, Store};
use aelomi_core::workspace::Workspace;
use aelomi_core::workspace::fixture::{
    BrokenIndex, Editor, SEARCH_CREATED_AT, blocknote_blocks, break_index, index_fixture, put_page,
    search_fixture, write_body,
};
use aelomi_core::workspace::records::{PageId, PageRecord, Placement};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((folder, rest)) = args.split_first() else {
        fail("usage: seed_index <folder> <missing|garbage|old-schema|valid|search-10k>");
    };
    let folder = new_folder(folder);
    match rest {
        [state] if state != "search-10k" => seed_fixture(&folder, state),
        [state, options @ ..] if state == "search-10k" => seed_search(&folder, options),
        _ => fail("usage: seed_index <folder> <missing|garbage|old-schema|valid|search-10k>"),
    }
}

fn new_folder(folder: &str) -> PathBuf {
    let folder = PathBuf::from(folder);
    if std::fs::read_dir(&folder).is_ok_and(|mut entries| entries.next().is_some()) {
        fail(&format!(
            "{} is not empty; seed a new folder",
            folder.display()
        ));
    }
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| fail(&e.to_string()));
    std::fs::canonicalize(&folder).unwrap_or_else(|e| fail(&e.to_string()))
}

fn seed_fixture(folder: &Path, state: &str) {
    let broken = match state {
        "missing" => Some(BrokenIndex::Missing),
        "garbage" => Some(BrokenIndex::Garbage),
        "old-schema" => Some(BrokenIndex::OldSchema),
        "valid" => None,
        other => fail(&format!("unknown state {other:?}")),
    };
    let workspace = Workspace::open(folder, |_| {}).unwrap_or_else(|e| fail(&e.to_string()));
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

fn seed_search(folder: &Path, options: &[String]) {
    let mut copies = 0;
    let mut big_page = false;
    let mut options = options.iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--copies" => {
                copies = options
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or_else(|| fail("--copies takes a number"));
            }
            "--big-page" => big_page = true,
            other => fail(&format!("unknown option {other:?}")),
        }
    }
    let generated = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/generated");
    let jsonl = std::fs::read_to_string(generated.join("search-10k-pages.jsonl"))
        .unwrap_or_else(|e| fail(&e.to_string()));
    let started = Instant::now();
    let mut updates = search_fixture(&jsonl, copies);
    let pages = 10_000 * (copies + 1);
    if big_page {
        let json = std::fs::read_to_string(generated.join("page-5000-blocks.json"))
            .unwrap_or_else(|e| fail(&e.to_string()));
        let blocks =
            blocknote_blocks(&serde_json::from_str(&json).unwrap_or_else(|e| fail(&e.to_string())));
        let mut editor = Editor::new(12);
        let page = PageId::new("big-page").unwrap_or_else(|e| fail(&e.to_string()));
        let created_at = SEARCH_CREATED_AT + pages as u64 * 1000;
        let record = PageRecord {
            title: "Kestrel field notes".into(),
            placement: Placement {
                parent: None,
                order: pages as u64,
            },
            favorite: None,
            trash: None,
            created_at,
            updated_at: created_at,
        };
        updates.push((
            DocId::workspace(),
            editor.edit(&DocId::workspace(), |txn| put_page(txn, &page, &record)),
        ));
        updates.push((
            page.doc_id().clone(),
            editor.edit(page.doc_id(), |txn| write_body(txn, &blocks)),
        ));
    }
    let store = Store::open(folder).unwrap_or_else(|e| fail(&e.to_string()));
    for (doc, update) in &updates {
        store
            .append(doc, update)
            .unwrap_or_else(|e| fail(&e.to_string()));
    }
    if let Err((_, pending)) = store.close(Instant::now() + Duration::from_secs(600)) {
        fail(&format!("could not flush {pending:?}"));
    }
    let (files, bytes) = files_in(folder);
    println!(
        "seeded {} with the search fixture: pages={} copies={copies} big_page={big_page} \
         updates={} fixture_crc32={:08x} source_files={files} source_bytes={bytes} seconds={:.1}",
        folder.display(),
        pages + usize::from(big_page),
        updates.len(),
        crc32fast::hash(jsonl.as_bytes()),
        started.elapsed().as_secs_f64(),
    );
    println!("no index exists: the first launch builds it (cache=created)");
}

fn files_in(dir: &Path) -> (usize, u64) {
    let mut stack = vec![dir.to_path_buf()];
    let (mut files, mut bytes) = (0, 0);
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| fail(&e.to_string())) {
            let entry = entry.unwrap_or_else(|e| fail(&e.to_string()));
            let meta = entry.metadata().unwrap_or_else(|e| fail(&e.to_string()));
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                files += 1;
                bytes += meta.len();
            }
        }
    }
    (files, bytes)
}

fn fail(message: &str) -> ! {
    eprintln!("seed_index: {message}");
    std::process::exit(1);
}
