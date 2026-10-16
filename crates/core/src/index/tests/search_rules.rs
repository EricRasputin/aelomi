use super::{Opened, all, content, open, page, ready};
use crate::index::{InvalidQuery, QueryError, SearchHit};
use crate::storage::DocId;
use crate::workspace::fixture::{Editor, paragraph, put_page, text, write_body};
use crate::workspace::records::{PageId, PageRecord, Placement, Trash};

type Parts = Vec<(String, bool)>;

fn p(parts: &[(&str, bool)]) -> Parts {
    parts.iter().map(|(t, h)| ((*t).to_owned(), *h)).collect()
}

fn record(title: &str, created_at: u64, trashed: bool) -> PageRecord {
    PageRecord {
        title: title.into(),
        placement: Placement {
            parent: None,
            order: created_at,
        },
        favorite: None,
        trash: trashed.then(|| Trash {
            original_parent: None,
            original_order: created_at,
            batch_id: "batch".into(),
            trashed_at: 9,
        }),
        created_at,
        updated_at: created_at,
    }
}

fn ranked(dir: &std::path::Path) -> Opened {
    let opened = open(dir);
    let mut editor = Editor::new(21);
    let workspace = DocId::workspace();
    let pages = [
        ("a", record("Alpha", 1, false)),
        ("b", record("Roadmap", 2, false)),
        ("c", record("Roadmap", 3, false)),
        ("d", record("Roadmap", 5, true)),
        ("e", record("Echo", 0, false)),
    ];
    let update = editor.edit(&workspace, |txn| {
        for (id, record) in &pages {
            put_page(txn, &page(id), record);
        }
    });
    opened.workspace.append(&workspace, &update).unwrap();
    for (id, blocks) in [
        (
            "a",
            vec![
                paragraph("a0", vec![text("quarterly roadmap")]),
                paragraph("a1", vec![text("roadmap roadmap")]),
            ],
        ),
        ("c", vec![paragraph("c0", vec![text("roadmaps")])]),
        ("d", vec![paragraph("d0", vec![text("roadmap")])]),
        (
            "e",
            vec![
                paragraph("x", vec![text("unrelated")]),
                paragraph("x", vec![text("a roadmap twin")]),
            ],
        ),
    ] {
        let doc = DocId::new(id).unwrap();
        let update = editor.edit(&doc, |txn| write_body(txn, &blocks));
        opened.workspace.append(&doc, &update).unwrap();
    }
    opened.settle();
    opened
}

type View<'a> = (&'a str, Parts, Option<(&'a str, Parts)>);

fn view(hits: &[SearchHit]) -> Vec<View<'_>> {
    let parts = |parts: &[crate::index::TextPart]| -> Parts {
        parts
            .iter()
            .map(|p| (p.text.clone(), p.highlighted))
            .collect()
    };
    hits.iter()
        .map(|hit| {
            (
                hit.page_id.as_str(),
                parts(&hit.title),
                hit.block
                    .as_ref()
                    .map(|b| (b.block_id.as_str(), parts(&b.text))),
            )
        })
        .collect()
}

#[test]
fn titles_come_first_then_bodies_newest_first_with_their_first_matching_block() {
    let dir = tempfile::tempdir().unwrap();
    let opened = ranked(dir.path());
    let ws = &opened.workspace;
    assert_eq!(
        view(&ready(ws.search(&all("roadm", 20)).unwrap())),
        [
            ("c", p(&[("Roadmap", true)]), None),
            ("b", p(&[("Roadmap", true)]), None),
            (
                "a",
                p(&[("Alpha", false)]),
                Some(("a0", p(&[("quarterly ", false), ("roadmap", true)])))
            ),
            (
                "e",
                p(&[("Echo", false)]),
                Some((
                    "x",
                    p(&[("a ", false), ("roadmap", true), (" twin", false)])
                ))
            ),
        ]
    );
    assert_eq!(
        view(&ready(ws.search(&all("roadm", 2)).unwrap())),
        [
            ("c", p(&[("Roadmap", true)]), None),
            ("b", p(&[("Roadmap", true)]), None),
        ]
    );
    assert_eq!(
        view(&ready(ws.search(&content("roadm", 20, &[])).unwrap())),
        [
            (
                "c",
                p(&[("Roadmap", true)]),
                Some(("c0", p(&[("roadmaps", true)])))
            ),
            (
                "a",
                p(&[("Alpha", false)]),
                Some(("a0", p(&[("quarterly ", false), ("roadmap", true)])))
            ),
            (
                "e",
                p(&[("Echo", false)]),
                Some((
                    "x",
                    p(&[("a ", false), ("roadmap", true), (" twin", false)])
                ))
            ),
        ]
    );
    assert_eq!(
        view(&ready(
            ws.search(&content("roadm", 1, &[page("c"), page("c")]))
                .unwrap()
        )),
        [(
            "a",
            p(&[("Alpha", false)]),
            Some(("a0", p(&[("quarterly ", false), ("roadmap", true)])))
        )],
        "an excluded page takes no slot of the limit"
    );
}

#[test]
fn removing_a_winner_promotes_the_next_page_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let opened = ranked(dir.path());
    let ids = |hits: Vec<SearchHit>| -> Vec<String> {
        hits.into_iter()
            .map(|h| h.page_id.as_str().to_owned())
            .collect()
    };
    opened
        .workspace
        .edit_page(crate::workspace::PageEdit::Trash { page: page("c") })
        .unwrap();
    opened.settle();
    assert_eq!(
        ids(ready(opened.workspace.search(&all("roadm", 2)).unwrap())),
        ["b", "a"]
    );
    opened.edit_body("e", |txn| {
        write_body(txn, &[paragraph("x", vec![text("a roadmap twin")])])
    });
    opened.edit_body("a", |txn| write_body(txn, &[]));
    opened.settle();
    assert_eq!(
        ids(ready(
            opened.workspace.search(&content("roadm", 5, &[])).unwrap()
        )),
        ["e"]
    );
}

#[test]
fn a_content_search_names_at_most_200_distinct_pages_to_skip() {
    let dir = tempfile::tempdir().unwrap();
    let opened = ranked(dir.path());
    let many: Vec<PageId> = (0..201).map(|n| page(&format!("p{n}"))).collect();
    assert_eq!(
        opened.workspace.search(&content("roadm", 5, &many)),
        Err(QueryError::InvalidQuery(InvalidQuery::TooManyExcluded {
            max: 200
        }))
    );
    let repeated: Vec<PageId> = (0..400).map(|n| page(&format!("p{}", n % 200))).collect();
    assert_eq!(
        ready(
            opened
                .workspace
                .search(&content("roadm", 5, &repeated))
                .unwrap()
        )
        .len(),
        3,
        "duplicates count once and unknown pages are ignored"
    );
}

#[test]
fn limits_clamp_to_one_through_two_hundred() {
    let dir = tempfile::tempdir().unwrap();
    let opened = ranked(dir.path());
    assert_eq!(
        ready(opened.workspace.search(&all("roadm", 0)).unwrap()).len(),
        1
    );
    assert_eq!(
        ready(opened.workspace.search(&all("roadm", 100_000)).unwrap()).len(),
        4
    );
}
