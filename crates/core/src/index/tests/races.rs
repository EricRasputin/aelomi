use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use super::{Opened, all, eventually, hit_ids, index_path, open, open_with, page, ready, snapshot};
use crate::index::gate::Gate;
use crate::index::{Config, HoldPoint, IndexStatus, Reply};
use crate::storage::{AppendError, DocId, Store};
use crate::workspace::fixture::{
    BrokenIndex, Inline, break_index, index_fixture, insert_block, link, paragraph, text,
    write_body,
};

fn words(opened: &Opened, query: &str) -> Vec<String> {
    let hits = ready(opened.workspace.search(&all(query, 50)).unwrap());
    let mut ids: Vec<String> = hit_ids(&hits).into_iter().map(str::to_owned).collect();
    ids.sort();
    ids
}

fn add_block(opened: &Opened, page: &str, id: &str, word: &str) {
    let spec = paragraph(id, vec![Inline::Text(word.into())]);
    opened.edit_body(page, |txn| insert_block(txn, 0, &spec));
}

fn seeded(dir: &Path) -> Opened {
    let opened = open(dir);
    opened.append(&index_fixture());
    opened.settle();
    opened
}

#[test]
fn an_append_while_the_worker_holds_a_taken_mark_is_indexed() {
    let dir = tempfile::tempdir().unwrap();
    let opened = seeded(dir.path());
    let daily = HoldPoint::BeforeLoad(DocId::new("daily").unwrap());
    opened.gate.arm(daily.clone());
    add_block(&opened, "daily", "x1", "firstword");
    opened.gate.wait_held(&daily);
    add_block(&opened, "daily", "x2", "secondword");
    opened.gate.release();
    opened.settle();
    assert_eq!(words(&opened, "firstword"), ["daily"]);
    assert_eq!(words(&opened, "secondword"), ["daily"]);
}

#[test]
fn an_append_while_the_worker_holds_its_commit_is_indexed() {
    let dir = tempfile::tempdir().unwrap();
    let opened = seeded(dir.path());
    opened.gate.arm(HoldPoint::BeforeCommit);
    add_block(&opened, "daily", "x1", "firstword");
    opened.gate.wait_held(&HoldPoint::BeforeCommit);
    add_block(&opened, "daily", "x2", "secondword");
    assert_eq!(
        words(&opened, "firstword"),
        Vec::<String>::new(),
        "not committed yet"
    );
    opened.gate.release();
    opened.settle();
    assert_eq!(words(&opened, "firstword"), ["daily"]);
    assert_eq!(words(&opened, "secondword"), ["daily"]);
}

#[test]
fn a_query_while_a_trash_is_unapplied_answers_indexing_never_the_trashed_page() {
    let dir = tempfile::tempdir().unwrap();
    let opened = seeded(dir.path());
    assert_eq!(words(&opened, "cafe"), ["reading"]);
    opened.gate.arm(HoldPoint::BeforeCommit);
    opened.edit_records(|records| {
        let reading = records.get_mut(&page("reading")).unwrap();
        reading.trash = Some(crate::workspace::records::Trash {
            original_parent: reading.placement.parent.clone(),
            original_order: 1,
            batch_id: "b".into(),
            trashed_at: 5,
        });
    });
    opened.gate.wait_held(&HoldPoint::BeforeCommit);
    assert_eq!(
        opened.workspace.search(&all("cafe", 10)),
        Ok(Reply::Indexing)
    );
    assert_eq!(
        opened.workspace.backlinks(&page("reading")),
        Ok(Reply::Indexing)
    );
    opened.gate.release();
    opened.settle();
    assert_eq!(words(&opened, "cafe"), Vec::<String>::new());
    assert_eq!(
        ready(opened.workspace.backlinks(&page("reading")).unwrap()),
        []
    );
}

#[test]
fn a_workspace_mark_during_a_query_turns_its_answer_into_indexing() {
    let dir = tempfile::tempdir().unwrap();
    let opened = seeded(dir.path());
    let index = opened.workspace.index();
    let answer = index.shared().read_if_current(|_| {
        index.mark(&DocId::workspace());
        Ok(())
    });
    assert_eq!(answer, Ok(Reply::Indexing));
    opened.settle();
    assert_eq!(
        index.shared().read_if_current(|_| Ok(())),
        Ok(Reply::Ready {
            value: (),
            issues: std::sync::Arc::from([])
        })
    );
}

#[test]
fn edits_during_the_startup_reconcile_converge_to_a_fresh_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    {
        let opened = seeded(dir.path());
        assert_eq!(
            opened
                .workspace
                .flush(Instant::now() + Duration::from_secs(10))
                .docs,
            []
        );
    }
    let gate = Arc::new(Gate::default());
    let daily = HoldPoint::BeforeLoad(DocId::new("daily").unwrap());
    gate.arm(daily.clone());
    let opened = open_with(
        Store::open(dir.path()).unwrap(),
        index_path(dir.path()),
        Config::DEFAULT,
        gate.clone(),
    );
    gate.wait_held(&daily);
    opened.edit_records(|records| {
        records.get_mut(&page("aelomi")).unwrap().title = "Aelomi renamed".into();
        let reading = records.get_mut(&page("reading")).unwrap();
        reading.trash = Some(crate::workspace::records::Trash {
            original_parent: reading.placement.parent.clone(),
            original_order: 1,
            batch_id: "b".into(),
            trashed_at: 5,
        });
        let mut fresh = records[&page("daily")].clone();
        fresh.title = "Fresh".into();
        records.insert(page("fresh"), fresh);
        records.remove(&page("untitled"));
    });
    opened.edit_body("fresh", |txn| {
        write_body(
            txn,
            &[paragraph("f1", vec![text("freshword "), link("aelomi")])],
        )
    });
    add_block(&opened, "daily", "late", "lateword");
    assert_eq!(opened.workspace.index_status(), IndexStatus::Rebuilding);
    assert_eq!(
        opened.workspace.search(&all("lateword", 10)),
        Ok(Reply::Indexing)
    );
    gate.release();
    opened.settle();
    assert_eq!(
        opened.statuses(),
        [IndexStatus::Rebuilding, IndexStatus::Ready]
    );
    assert_eq!(words(&opened, "lateword"), ["daily"]);
    assert_eq!(words(&opened, "freshword"), ["fresh"]);
    assert_eq!(words(&opened, "cafe"), Vec::<String>::new());
    assert_eq!(
        words(&opened, "renamed"),
        ["aelomi", "daily", "fresh", "roadmap"]
    );
    let backlinks = ready(opened.workspace.backlinks(&page("aelomi")).unwrap());
    let texts: Vec<(&str, Vec<&str>)> = backlinks
        .iter()
        .map(|b| {
            (
                b.page_id.as_str(),
                b.blocks.iter().map(|x| x.text.as_str()).collect(),
            )
        })
        .collect();
    assert_eq!(
        texts,
        [
            (
                "daily",
                vec![
                    "see Aelomi renamed today",
                    "Aelomi renamed and Aelomi renamed"
                ]
            ),
            ("fresh", vec!["freshword Aelomi renamed"]),
            ("roadmap", vec!["Milestones for Aelomi renamed"]),
        ]
    );
    let incremental = snapshot(&opened.workspace);
    assert_eq!(
        opened
            .workspace
            .flush(Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
    drop(opened);

    break_index(&index_path(dir.path()), BrokenIndex::Missing).unwrap();
    let rebuilt = open(dir.path());
    rebuilt.settle();
    assert_eq!(snapshot(&rebuilt.workspace), incremental);
}

const HELD_EDITS: usize = 2;

#[test]
fn continuous_edits_never_starve_the_coalesced_update() {
    let dir = tempfile::tempdir().unwrap();
    let opened = Arc::new(seeded(dir.path()));
    opened.gate.arm(HoldPoint::BeforeCommit);
    add_block(&opened, "daily", "held", "heldword");
    opened.gate.wait_held(&HoldPoint::BeforeCommit);
    let (stop, stopping) = mpsc::channel::<()>();
    let (edits_accepted, started) = mpsc::channel();
    let typing = {
        let opened = opened.clone();
        std::thread::spawn(move || {
            let mut n = 0;
            while matches!(stopping.try_recv(), Err(mpsc::TryRecvError::Empty)) {
                add_block(&opened, "daily", &format!("t{n}"), &format!("keystroke{n}"));
                n += 1;
                if n == HELD_EDITS {
                    edits_accepted.send(()).unwrap();
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            n
        })
    };
    if let Err(error) = started.recv_timeout(Duration::from_secs(5)) {
        drop(stop);
        opened.gate.release();
        match typing.join() {
            Err(panic) => std::panic::resume_unwind(panic),
            Ok(accepted) => {
                panic!("producer accepted {accepted} edits while the index was held: {error}")
            }
        }
    }
    opened.gate.release();
    let hits = eventually(Duration::from_secs(5), || {
        match opened.workspace.search(&all("keystroke0", 5)) {
            Ok(Reply::Ready { value: hits, .. }) if !hits.is_empty() => Some(hits),
            _ => None,
        }
    });
    drop(stop);
    let accepted = typing
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    assert!(
        accepted >= HELD_EDITS,
        "producer accepted {accepted} edits while the index was held"
    );
    assert_eq!(hit_ids(&hits), ["daily"]);
}

#[test]
fn a_refused_append_marks_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let opened = seeded(dir.path());
    let index = opened.workspace.index();
    for doc in [DocId::workspace(), DocId::new("daily").unwrap()] {
        assert!(matches!(
            opened.workspace.append(&doc, b"not a yjs update"),
            Err(AppendError::InvalidUpdate)
        ));
    }
    assert!(index.shared().is_clean());
}

#[test]
fn the_index_never_delays_an_acknowledgement() {
    let dir = tempfile::tempdir().unwrap();
    let opened = seeded(dir.path());
    opened.gate.arm(HoldPoint::BeforeCommit);
    add_block(&opened, "daily", "x1", "heldword");
    opened.gate.wait_held(&HoldPoint::BeforeCommit);
    let started = Instant::now();
    add_block(&opened, "roadmap", "x2", "flushedword");
    assert_eq!(
        opened
            .workspace
            .flush(Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
    eprintln!(
        "append and flush with the index held mid-commit: {:?}",
        started.elapsed()
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    let loaded = opened
        .workspace
        .load(&DocId::new("roadmap").unwrap())
        .unwrap();
    assert!(!loaded.is_empty());
    opened.gate.release();
    opened.settle();
    assert_eq!(words(&opened, "flushedword"), ["roadmap"]);
}
