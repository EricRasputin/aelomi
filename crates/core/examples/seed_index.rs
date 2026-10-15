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

const USAGE: &str = "usage: seed_index <new folder> <missing|garbage|old-schema|valid|not-a-fragment|foreign-pages-root>
       seed_index <new folder> search-10k [--copies <n>] [--big-page]
then launch the app with AELOMI_DATA_DIR=<folder>";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((folder, rest)) = args.split_first() else {
        fail(USAGE);
    };
    let folder = new_folder(folder);
    match rest {
        [state] if state != "search-10k" => seed_fixture(&folder, state),
        [state, options @ ..] if state == "search-10k" => seed_search(&folder, options),
        _ => fail(USAGE),
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
        "valid" | "not-a-fragment" | "foreign-pages-root" => None,
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
    match state {
        "not-a-fragment" => append_plain_text_page(&workspace),
        "foreign-pages-root" => append_foreign_pages_sequence(&workspace),
        _ => {}
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
    match state {
        "not-a-fragment" => println!(
            "plus Plain text body, whose prosemirror root is a Y.Text: search names it as only partly searchable"
        ),
        "foreign-pages-root" => println!(
            "plus sequence content under the pages root: the workspace fails, no page opens, and the index cache is kept"
        ),
        _ => {}
    }
}

/// A page whose `prosemirror` root another writer made a Y.Text, so the index reports it as
/// only partly searchable (issue #74).
fn append_plain_text_page(workspace: &Workspace) {
    use yrs::{ReadTxn, Text, Transact};
    let page = PageId::new("plain-text").unwrap_or_else(|e| fail(&e.to_string()));
    let record = PageRecord {
        title: "Plain text body".into(),
        placement: Placement {
            parent: None,
            order: 50,
        },
        favorite: None,
        trash: None,
        created_at: 1_760_000_000_000,
        updated_at: 1_760_000_000_000,
    };
    let mut editor = Editor::new(31);
    let records = editor.edit(&DocId::workspace(), |txn| put_page(txn, &page, &record));
    let body = yrs::Doc::with_client_id(32);
    let text = body.get_or_insert_text("prosemirror");
    text.insert(
        &mut body.transact_mut(),
        0,
        "kestrel notes in a Y.Text, café 😀",
    );
    let body = body
        .transact()
        .encode_state_as_update_v1(&Default::default());
    for (doc, update) in [(DocId::workspace(), records), (page.doc_id().clone(), body)] {
        workspace
            .append(&doc, &update)
            .unwrap_or_else(|e| fail(&e.to_string()));
    }
}

/// Sequence content under the reserved `pages` root, which this build cannot read, so the
/// workspace fails instead of reading as empty (issue #77).
fn append_foreign_pages_sequence(workspace: &Workspace) {
    use yrs::{Array, WriteTxn};
    let mut foreign = Editor::new(33);
    let state = workspace
        .load(&DocId::workspace())
        .unwrap_or_else(|e| fail(&e.to_string()));
    foreign.apply(&DocId::workspace(), &state);
    let update = foreign.edit(&DocId::workspace(), |txn| {
        txn.get_or_insert_array("pages").insert(txn, 0, "foreign");
    });
    workspace
        .append(&DocId::workspace(), &update)
        .unwrap_or_else(|e| fail(&e.to_string()));
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
