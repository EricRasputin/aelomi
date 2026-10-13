use std::time::{Duration, Instant};

use super::{eventually, open, page};
use crate::index::Reply;
use crate::storage::DocId;
use crate::workspace::fixture::{Editor, Inline, insert_block, paragraph, put_page, write_body};
use crate::workspace::records::{PageRecord, Placement};

const LINK_8: Duration = Duration::from_millis(250);

fn record(title: &str) -> PageRecord {
    PageRecord {
        title: title.into(),
        placement: Placement {
            parent: None,
            order: 0,
        },
        favorite: None,
        trash: None,
        created_at: 1,
        updated_at: 1,
    }
}

fn linking_latencies(blocks: usize, edits: usize) -> Vec<Duration> {
    let dir = tempfile::tempdir().unwrap();
    let opened = open(dir.path());
    let ws = &opened.workspace;
    let mut editor = Editor::new(3);
    let workspace = DocId::workspace();
    let source = DocId::new("source").unwrap();
    let records = editor.edit(&workspace, |txn| {
        put_page(txn, &page("source"), &record("Source"));
        put_page(txn, &page("target"), &record("Target"));
    });
    ws.append(&workspace, &records).unwrap();
    let filler: Vec<_> = (0..blocks)
        .map(|i| {
            paragraph(
                &format!("f{i}"),
                vec![Inline::Text(format!("filler block {i}"))],
            )
        })
        .collect();
    ws.append(
        &source,
        &editor.edit(&source, |txn| write_body(txn, &filler)),
    )
    .unwrap();
    opened.settle();

    (0..edits)
        .map(|i| {
            let block = format!("link-{i}");
            let spec = paragraph(
                &block,
                vec![Inline::Text("see ".into()), Inline::Link(page("target"))],
            );
            let update = editor.edit(&source, |txn| insert_block(txn, (blocks + i) as u32, &spec));
            let started = Instant::now();
            ws.append(&source, &update).unwrap();
            eventually(Duration::from_secs(10), || {
                let Ok(Reply::Ready(backlinks)) = ws.backlinks(&page("target")) else {
                    return None;
                };
                backlinks
                    .iter()
                    .flat_map(|b| &b.blocks)
                    .any(|b| b.block_id.as_str() == block)
                    .then(|| started.elapsed())
            })
        })
        .collect()
}

#[test]
#[ignore = "LINK-8 is gated separately in release with one test thread"]
fn a_linking_edit_reaches_the_backlinks_within_250_ms_on_real_threads() {
    let latencies = linking_latencies(200, 20);
    let max = *latencies.iter().max().unwrap();
    eprintln!(
        "200-block page: linking edit to backlink, max {max:?} over {} edits",
        latencies.len()
    );
    assert!(max < LINK_8, "{latencies:?}");
}

#[test]
#[ignore = "LINK-8 is gated separately in release with one test thread"]
fn a_linking_edit_on_a_5000_block_page_reaches_the_backlinks_within_250_ms() {
    let latencies = linking_latencies(5_000, 5);
    let max = *latencies.iter().max().unwrap();
    eprintln!(
        "5,000-block page: linking edit to backlink, max {max:?} over {} edits",
        latencies.len()
    );
    assert!(max < LINK_8, "{latencies:?}");
}
