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

fn is_active<'a>(records: &'a BTreeMap<PageId, PageRecord>, mut id: &'a PageId) -> bool {
    loop {
        let Some(record) = records.get(id) else {
            return false;
        };
        if record.trash.is_some() {
            return false;
        }
        match record
            .placement
            .parent
            .as_ref()
            .filter(|p| records.contains_key(*p))
        {
            None => return true,
            Some(parent) => id = parent,
        }
    }
}

fn from_scratch(workspace: &Workspace, target: &PageId) -> Backlinks {
    let ws = doc_of(&workspace.load(&DocId::workspace()).unwrap());
    let records = read_pages(&ws.transact()).unwrap();
    let active = |id: &PageId| is_active(&records, id);
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
            .unwrap()
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
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0..PAGES, 0..PAGES + 1, any::<bool>())
            .prop_map(|(source, target, first)| Op::Link { source, target, first }),
        2 => (0..PAGES, any::<usize>()).prop_map(|(source, which)| Op::Unlink { source, which }),
        2 => (0..PAGES, 0..TITLES.len()).prop_map(|(page, title)| Op::Rename { page, title }),
        1 => (0..PAGES).prop_map(|page| Op::Trash { page }),
        1 => (0..PAGES).prop_map(|page| Op::Restore { page }),
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
            coalesce: Duration::ZERO,
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
        }
    }
}
