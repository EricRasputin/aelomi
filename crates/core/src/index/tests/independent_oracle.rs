use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use proptest::prelude::*;
use yrs::Transact;

use super::{Opened, doc_of, index_path, open_with, page, ready};
use crate::index::Config;
use crate::storage::{DocId, Store};
use crate::workspace::Workspace;
use crate::workspace::body::{Segment, read_body};
use crate::workspace::fixture::{Editor, Inline, insert_block, paragraph, put_page, remove_block};
use crate::workspace::records::{PageId, PageRecord, Placement, Trash, read_pages};

type Backlinks = Vec<(String, Vec<(String, String)>)>;

fn normalized_parents(records: &BTreeMap<PageId, PageRecord>) -> BTreeMap<PageId, Option<PageId>> {
    let mut parents: BTreeMap<PageId, Option<PageId>> = records
        .iter()
        .map(|(id, record)| {
            let parent = record.placement.parent.clone();
            (id.clone(), parent.filter(|p| records.contains_key(p)))
        })
        .collect();
    let bound = records.len() + 1;
    let returns_to_itself = |parents: &BTreeMap<PageId, Option<PageId>>, start: &PageId| {
        let mut at = parents[start].clone();
        for _ in 0..bound {
            match at {
                None => return false,
                Some(p) if p == *start => return true,
                Some(p) => at = parents[&p].clone(),
            }
        }
        false
    };
    let on_cycles: Vec<PageId> = records
        .keys()
        .filter(|id| returns_to_itself(&parents, id))
        .cloned()
        .collect();
    let mut cut = BTreeSet::new();
    for start in &on_cycles {
        let mut cycle = vec![start.clone()];
        let mut at = parents[start].clone().expect("a cycle has parents");
        while at != *start {
            cycle.push(at.clone());
            at = parents[&at].clone().expect("a cycle has parents");
        }
        cut.insert(cycle.into_iter().min().expect("a cycle has pages"));
    }
    for id in cut {
        parents.insert(id, None);
    }
    parents
}

fn active_pages(records: &BTreeMap<PageId, PageRecord>) -> BTreeSet<PageId> {
    let parents = normalized_parents(records);
    records
        .keys()
        .filter(|start| {
            let mut at = Some((*start).clone());
            for _ in 0..=records.len() {
                match at {
                    None => return true,
                    Some(id) if records[&id].trash.is_some() => return false,
                    Some(id) => at = parents[&id].clone(),
                }
            }
            panic!("normalized parents have no cycle")
        })
        .cloned()
        .collect()
}

fn from_scratch(workspace: &Workspace, target: &PageId) -> Backlinks {
    let ws = doc_of(&workspace.load(&DocId::workspace()).unwrap());
    let records = read_pages(&ws.transact()).unwrap();
    let active_set = active_pages(&records);
    let active = |id: &PageId| active_set.contains(id);
    if !active(target) {
        return Vec::new();
    }
    let label = |id: &PageId| match records.get(id) {
        None => "Deleted page".to_owned(),
        Some(r) if r.title.is_empty() => "Untitled".to_owned(),
        Some(r) => r.title.clone(),
    };
    let mut sources: Vec<(&PageId, &PageRecord)> =
        records.iter().filter(|(id, _)| active(id)).collect();
    sources.sort_by(|(a, x), (b, y)| y.updated_at.cmp(&x.updated_at).then(a.cmp(b)));
    let mut out = Vec::new();
    for (id, _) in sources {
        let body = doc_of(&workspace.load(id.doc_id()).unwrap());
        let blocks: Vec<(String, String)> = read_body(&body.transact())
            .blocks
            .into_iter()
            .filter(|b| b.segments.contains(&Segment::Link(target.clone())))
            .map(|b| {
                let text = b
                    .segments
                    .iter()
                    .map(|s| match s {
                        Segment::Text(t) => t.clone(),
                        Segment::Link(l) => label(l),
                    })
                    .collect();
                (b.id.as_str().to_owned(), text)
            })
            .collect();
        if !blocks.is_empty() {
            out.push((id.as_str().to_owned(), blocks));
        }
    }
    out
}

fn indexed(workspace: &Workspace, target: &PageId) -> Backlinks {
    ready(workspace.backlinks(target).unwrap())
        .into_iter()
        .map(|b| {
            (
                b.page_id.as_str().to_owned(),
                b.blocks
                    .into_iter()
                    .map(|block| (block.block_id.as_str().to_owned(), block.text))
                    .collect(),
            )
        })
        .collect()
}

const PAGES: usize = 6;
const TITLES: [&str; 4] = ["Alpha", "", "Beta gamma", "Alpha"];

fn id(n: usize) -> PageId {
    page(&format!("p{n}"))
}

#[derive(Clone, Debug)]
enum Op {
    Link {
        source: usize,
        target: usize,
        first: bool,
    },
    Unlink {
        source: usize,
        which: usize,
    },
    Rename {
        page: usize,
        title: usize,
    },
    Trash {
        page: usize,
    },
    Restore {
        page: usize,
    },
    Move {
        page: usize,
        parent: Option<usize>,
    },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0..PAGES, 0..PAGES + 1, any::<bool>())
            .prop_map(|(source, target, first)| Op::Link { source, target, first }),
        2 => (0..PAGES, any::<usize>()).prop_map(|(source, which)| Op::Unlink { source, which }),
        2 => (0..PAGES, 0..TITLES.len()).prop_map(|(page, title)| Op::Rename { page, title }),
        1 => (0..PAGES).prop_map(|page| Op::Trash { page }),
        1 => (0..PAGES).prop_map(|page| Op::Restore { page }),
        2 => (0..PAGES, prop::option::of(0..PAGES))
            .prop_map(|(page, parent)| Op::Move { page, parent }),
    ]
}

struct Model {
    opened: Opened,
    blocks: BTreeMap<usize, usize>,
    next_block: usize,
    clock: u64,
}

impl Model {
    fn new(dir: &std::path::Path) -> Self {
        let config = Config {
            first_edit_delay: Duration::ZERO,
            ..Config::DEFAULT
        };
        let opened = open_with(
            Store::open(dir).unwrap(),
            index_path(dir),
            config,
            Arc::default(),
        );
        let mut editor = Editor::new(5);
        let workspace = DocId::workspace();
        let update = editor.edit(&workspace, |txn| {
            for n in 0..PAGES {
                let parent = match n {
                    3 => Some(id(1)),
                    4 => Some(id(3)),
                    _ => None,
                };
                let record = PageRecord {
                    title: format!("Page {n}"),
                    placement: Placement {
                        parent,
                        order: n as u64,
                    },
                    favorite: None,
                    trash: None,
                    created_at: 1,
                    updated_at: 10 + n as u64,
                };
                put_page(txn, &id(n), &record);
            }
        });
        opened.workspace.append(&workspace, &update).unwrap();
        opened.settle();
        Self {
            opened,
            blocks: BTreeMap::new(),
            next_block: 0,
            clock: 100,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    fn touch(&mut self, source: usize) {
        let now = self.tick();
        self.opened.edit_records(|records| {
            records.get_mut(&id(source)).unwrap().updated_at = now;
        });
    }

    fn apply(&mut self, op: &Op) {
        match *op {
            Op::Link {
                source,
                target,
                first,
            } => {
                self.next_block += 1;
                let len = self.blocks.entry(source).or_default();
                let at = if first { 0 } else { *len as u32 };
                *len += 1;
                let spec = paragraph(
                    &format!("b{}", self.next_block),
                    vec![Inline::Text("see ".into()), Inline::Link(id(target))],
                );
                self.opened
                    .edit_body(&format!("p{source}"), |txn| insert_block(txn, at, &spec));
                self.touch(source);
            }
            Op::Unlink { source, which } => {
                let len = self.blocks.entry(source).or_default();
                if *len == 0 {
                    return;
                }
                let at = (which % *len) as u32;
                *len -= 1;
                self.opened
                    .edit_body(&format!("p{source}"), |txn| remove_block(txn, at));
                self.touch(source);
            }
            Op::Rename { page, title } => {
                let now = self.tick();
                self.opened.edit_records(|records| {
                    let record = records.get_mut(&id(page)).unwrap();
                    record.title = TITLES[title].into();
                    record.updated_at = now;
                });
            }
            Op::Trash { page } => {
                let now = self.tick();
                self.opened.edit_records(|records| {
                    if records[&id(page)].trash.is_some() {
                        return;
                    }
                    for member in descendants(records, &id(page)) {
                        let record = records.get_mut(&member).unwrap();
                        if record.trash.is_none() {
                            record.trash = Some(Trash {
                                original_parent: record.placement.parent.clone(),
                                original_order: record.placement.order,
                                batch_id: format!("batch-{now}"),
                                trashed_at: now,
                            });
                        }
                    }
                });
            }
            Op::Move { page, parent } => {
                let now = self.tick();
                self.opened.edit_records(|records| {
                    let record = records.get_mut(&id(page)).unwrap();
                    record.placement.parent = parent.map(id);
                    record.updated_at = now;
                });
            }
            Op::Restore { page } => {
                self.opened.edit_records(|records| {
                    let Some(batch) = records[&id(page)]
                        .trash
                        .as_ref()
                        .map(|t| t.batch_id.clone())
                    else {
                        return;
                    };
                    for record in records.values_mut() {
                        if record.trash.as_ref().is_some_and(|t| t.batch_id == batch) {
                            record.trash = None;
                        }
                    }
                });
            }
        }
    }
}

fn descendants(records: &BTreeMap<PageId, PageRecord>, root: &PageId) -> BTreeSet<PageId> {
    let mut found = BTreeSet::from([root.clone()]);
    loop {
        let more: Vec<PageId> = records
            .iter()
            .filter(|(id, r)| {
                !found.contains(*id)
                    && r.placement
                        .parent
                        .as_ref()
                        .is_some_and(|p| found.contains(p))
            })
            .map(|(id, _)| id.clone())
            .collect();
        if more.is_empty() {
            return found;
        }
        found.extend(more);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn backlinks_equal_a_from_scratch_computation_after_every_step(ops in prop::collection::vec(op(), 1..30)) {
        let dir = tempfile::tempdir().unwrap();
        let mut model = Model::new(dir.path());
        for op in &ops {
            model.apply(op);
            model.opened.settle();
            for n in 0..=PAGES {
                let target = id(n);
                prop_assert_eq!(
                    indexed(&model.opened.workspace, &target),
                    from_scratch(&model.opened.workspace, &target),
                    "after {:?}, target {}", op, target
                );
            }
            prop_assert_eq!(
                tree_of(&model.opened.workspace),
                tree_from_scratch(&model.opened.workspace),
                "after {:?}", op
            );
        }
    }
}

fn tree_of(workspace: &Workspace) -> BTreeMap<PageId, Option<PageId>> {
    workspace
        .tree()
        .unwrap()
        .pages
        .iter()
        .map(|p| (p.id.clone(), p.parent.clone()))
        .collect()
}

fn tree_from_scratch(workspace: &Workspace) -> BTreeMap<PageId, Option<PageId>> {
    let ws = doc_of(&workspace.load(&DocId::workspace()).unwrap());
    let records = read_pages(&ws.transact()).unwrap();
    let parents = normalized_parents(&records);
    active_pages(&records)
        .into_iter()
        .map(|id| {
            let parent = parents[&id].clone();
            (id, parent)
        })
        .collect()
}

fn placed(title: &str, parent: Option<&str>, order: u64, updated_at: u64) -> PageRecord {
    PageRecord {
        title: title.into(),
        placement: Placement {
            parent: parent.map(page),
            order,
        },
        favorite: None,
        trash: None,
        created_at: 1,
        updated_at,
    }
}

fn trashed(mut record: PageRecord) -> PageRecord {
    record.trash = Some(Trash {
        original_parent: record.placement.parent.clone(),
        original_order: record.placement.order,
        batch_id: "batch".into(),
        trashed_at: 5,
    });
    record
}

fn before_and_after_rebuild(
    pages: &[(&str, PageRecord)],
    bodies: &[(&str, Vec<Inline>)],
    expect: impl Fn(&Workspace),
) {
    let dir = tempfile::tempdir().unwrap();
    let opened = super::open(dir.path());
    let mut editor = Editor::new(41);
    let workspace = DocId::workspace();
    let update = editor.edit(&workspace, |txn| {
        for (id, record) in pages {
            put_page(txn, &page(id), record);
        }
    });
    opened.workspace.append(&workspace, &update).unwrap();
    for (id, inline) in bodies {
        let doc = DocId::new(*id).unwrap();
        let spec = paragraph(&format!("{id}1"), inline.clone());
        let update = editor.edit(&doc, |txn| insert_block(txn, 0, &spec));
        opened.workspace.append(&doc, &update).unwrap();
    }
    opened.settle();
    expect(&opened.workspace);
    assert_eq!(
        opened
            .workspace
            .flush(std::time::Instant::now() + Duration::from_secs(10))
            .docs,
        []
    );
    drop(opened);
    let source = super::source_files(dir.path());
    crate::workspace::fixture::break_index(
        &index_path(dir.path()),
        crate::workspace::fixture::BrokenIndex::Missing,
    )
    .unwrap();
    let rebuilt = super::open(dir.path());
    rebuilt.settle();
    expect(&rebuilt.workspace);
    drop(rebuilt);
    assert_eq!(super::source_files(dir.path()), source);
}

fn tree_pages(workspace: &Workspace) -> Vec<(String, Option<String>)> {
    workspace
        .tree()
        .unwrap()
        .pages
        .iter()
        .map(|p| {
            (
                p.id.as_str().to_owned(),
                p.parent.as_ref().map(|p| p.as_str().to_owned()),
            )
        })
        .collect()
}

fn see(target: &str) -> Vec<Inline> {
    vec![Inline::Text("see ".into()), Inline::Link(page(target))]
}

#[test]
fn a_trashed_page_inside_a_cut_cycle_hides_only_its_own_descendants() {
    let pages = [
        ("a", placed("Alpha", Some("b"), 0, 10)),
        ("b", trashed(placed("Beta", Some("c"), 0, 1))),
        ("c", placed("Gamma", Some("a"), 0, 20)),
        ("d", placed("Delta", Some("b"), 0, 40)),
        ("e", placed("Epsilon", None, 1, 30)),
    ];
    let bodies = [
        ("a", see("a")),
        ("c", see("a")),
        ("e", see("a")),
        ("d", see("a")),
        ("b", see("a")),
    ];
    before_and_after_rebuild(&pages, &bodies, |ws| {
        assert_eq!(
            tree_pages(ws),
            [
                ("a".into(), None),
                ("c".into(), Some("a".into())),
                ("e".into(), None)
            ]
        );
        assert_eq!(
            indexed(ws, &page("a")),
            [
                ("e".into(), vec![("e1".into(), "see Alpha".into())]),
                ("c".into(), vec![("c1".into(), "see Alpha".into())]),
                ("a".into(), vec![("a1".into(), "see Alpha".into())]),
            ]
        );
        for hidden in ["b", "d"] {
            assert_eq!(indexed(ws, &page(hidden)), Backlinks::new(), "{hidden}");
        }
        for n in ["a", "b", "c", "d", "e"] {
            assert_eq!(indexed(ws, &page(n)), from_scratch(ws, &page(n)), "{n}");
        }
    });
}

#[test]
fn a_cycle_cut_at_its_smallest_id_decides_which_pages_a_trash_hides() {
    let pages = [
        ("a", placed("Ay", Some("c"), 0, 10)),
        ("b", trashed(placed("Bee", Some("a"), 0, 20))),
        ("c", placed("Cee", Some("b"), 0, 30)),
        ("d", placed("Dee", None, 1, 40)),
    ];
    let bodies = [
        (
            "d",
            vec![
                Inline::Link(page("a")),
                Inline::Link(page("b")),
                Inline::Link(page("c")),
            ],
        ),
        ("a", see("d")),
        ("c", see("d")),
    ];
    before_and_after_rebuild(&pages, &bodies, |ws| {
        assert_eq!(tree_pages(ws), [("a".into(), None), ("d".into(), None)]);
        assert_eq!(
            indexed(ws, &page("d")),
            [("a".into(), vec![("a1".into(), "see Dee".into())])]
        );
        assert_eq!(
            indexed(ws, &page("a")),
            [("d".into(), vec![("d1".into(), "AyBeeCee".into())])]
        );
        assert_eq!(indexed(ws, &page("b")), Backlinks::new());
        assert_eq!(indexed(ws, &page("c")), Backlinks::new());
    });
}

#[test]
fn a_page_that_is_its_own_parent_is_top_level() {
    let pages = [
        ("a", placed("Ay", Some("a"), 0, 10)),
        ("b", placed("Bee", Some("a"), 0, 20)),
    ];
    before_and_after_rebuild(&pages, &[("b", see("a"))], |ws| {
        assert_eq!(
            tree_pages(ws),
            [("a".into(), None), ("b".into(), Some("a".into()))]
        );
        assert_eq!(
            indexed(ws, &page("a")),
            [("b".into(), vec![("b1".into(), "see Ay".into())])]
        );
    });
}

#[test]
fn the_oracle_imports_no_tree_or_index_code() {
    let source = include_str!("independent_oracle.rs");
    let imports: Vec<&str> = source
        .lines()
        .filter(|line| line.starts_with("use "))
        .collect();
    for forbidden in [
        "workspace::tree",
        "index::db",
        "index::query",
        "index::worker",
        "index::highlight",
        "index::tokenizer",
    ] {
        assert!(
            imports.iter().all(|line| !line.contains(forbidden)),
            "the oracle imports {forbidden}: {imports:?}"
        );
    }
    assert!(!imports.is_empty());
}
