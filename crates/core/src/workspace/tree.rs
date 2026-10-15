use std::collections::{BTreeMap, BTreeSet};

use super::records::{PageId, PageRecord};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    /// Every active page, depth-first: parents before children, siblings by order then ID.
    pub pages: Vec<TreePage>,
    /// Active favorites, by favorite order then ID.
    pub favorites: Vec<PageId>,
    /// The pages the user trashed, without the descendants trashed with them, newest first
    /// (TREE-10).
    pub trash: Vec<TrashedPage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreePage {
    pub id: PageId,
    pub parent: Option<PageId>,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashedPage {
    pub id: PageId,
    pub title: String,
    pub batch_id: String,
    pub trashed_at: u64,
}

pub(crate) struct Forest {
    pub parent: BTreeMap<PageId, Option<PageId>>,
    pub active: BTreeSet<PageId>,
}

pub(crate) fn forest(records: &BTreeMap<PageId, PageRecord>) -> Forest {
    let pages = Pages::of(records);
    Forest {
        parent: pages
            .ids
            .iter()
            .zip(&pages.parent)
            .map(|(id, parent)| ((*id).clone(), parent.map(|p| pages.ids[p].clone())))
            .collect(),
        active: pages
            .ids
            .iter()
            .zip(&pages.active)
            .filter(|(_, active)| **active)
            .map(|(id, _)| (*id).clone())
            .collect(),
    }
}

/// The records by position in page ID order, so the projection compares and looks up integers
/// instead of page IDs. Position order is page ID order, so the smallest position in a cycle is
/// its smallest page ID.
struct Pages<'a> {
    ids: Vec<&'a PageId>,
    records: Vec<&'a PageRecord>,
    /// A missing parent becomes the top level, and a parent cycle is cut at its smallest ID.
    parent: Vec<Option<usize>>,
    /// The page and every ancestor have no `trash`.
    active: Vec<bool>,
}

impl<'a> Pages<'a> {
    fn of(records: &'a BTreeMap<PageId, PageRecord>) -> Self {
        let ids: Vec<&PageId> = records.keys().collect();
        let records: Vec<&PageRecord> = records.values().collect();
        let mut parent: Vec<Option<usize>> = records
            .iter()
            .map(|record| {
                let parent = record.placement.parent.as_ref()?;
                ids.binary_search(&parent).ok()
            })
            .collect();
        cut_cycles_at_min_id(&mut parent);
        let active = mark_active(&records, &parent);
        Self {
            ids,
            records,
            parent,
            active,
        }
    }
}

fn cut_cycles_at_min_id(parent: &mut [Option<usize>]) {
    #[derive(Clone, Copy, PartialEq)]
    enum Walk {
        New,
        OnPath,
        Done,
    }
    let mut walk = vec![Walk::New; parent.len()];
    let mut path = Vec::new();
    for start in 0..parent.len() {
        let mut next = Some(start);
        while let Some(page) = next {
            match walk[page] {
                Walk::Done => break,
                Walk::OnPath => {
                    let at = path.iter().position(|p| *p == page).expect("on the path");
                    let cut = *path[at..].iter().min().expect("a cycle has pages");
                    parent[cut] = None;
                    break;
                }
                Walk::New => {}
            }
            walk[page] = Walk::OnPath;
            next = parent[page];
            path.push(page);
        }
        for page in path.drain(..) {
            walk[page] = Walk::Done;
        }
    }
}

fn mark_active(records: &[&PageRecord], parent: &[Option<usize>]) -> Vec<bool> {
    #[derive(Clone, Copy, PartialEq)]
    enum Known {
        Unknown,
        Active,
        Inactive,
    }
    let mut known = vec![Known::Unknown; records.len()];
    let mut path = Vec::new();
    for start in 0..records.len() {
        let mut next = Some(start);
        let is_active = loop {
            let Some(page) = next else { break true };
            match known[page] {
                Known::Active => break true,
                Known::Inactive => break false,
                Known::Unknown if records[page].trash.is_some() => break false,
                Known::Unknown => {}
            }
            path.push(page);
            next = parent[page];
        };
        let mark = if is_active {
            Known::Active
        } else {
            Known::Inactive
        };
        for page in path.drain(..) {
            known[page] = mark;
        }
    }
    known.into_iter().map(|k| k == Known::Active).collect()
}

pub fn tree(records: &BTreeMap<PageId, PageRecord>) -> Tree {
    let pages = Pages::of(records);
    let Pages {
        ids,
        records,
        parent,
        active,
    } = &pages;
    let mut roots = Vec::new();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); ids.len()];
    for page in (0..ids.len()).filter(|page| active[*page]) {
        match parent[page] {
            None => roots.push(page),
            Some(p) => children[p].push(page),
        }
    }
    let order = |page: &usize| (records[*page].placement.order, *page);
    roots.sort_by_key(order);
    for siblings in &mut children {
        siblings.sort_by_key(order);
    }
    let mut tree_pages = Vec::with_capacity(roots.len());
    let mut stack: Vec<usize> = roots.into_iter().rev().collect();
    while let Some(page) = stack.pop() {
        let record = records[page];
        tree_pages.push(TreePage {
            id: ids[page].clone(),
            parent: parent[page].map(|p| ids[p].clone()),
            title: record.title.clone(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        });
        stack.extend(children[page].iter().rev());
    }
    let mut favorites: Vec<(u64, usize)> = (0..ids.len())
        .filter(|page| active[*page])
        .filter_map(|page| Some((records[page].favorite.as_ref()?.order, page)))
        .collect();
    favorites.sort();
    let mut trash: Vec<TrashedPage> = (0..ids.len())
        .filter_map(|page| {
            let trash = records[page].trash.as_ref()?;
            let parent_batch = parent[page]
                .and_then(|p| records[p].trash.as_ref())
                .map(|t| &t.batch_id);
            (parent_batch != Some(&trash.batch_id)).then(|| TrashedPage {
                id: ids[page].clone(),
                title: records[page].title.clone(),
                batch_id: trash.batch_id.clone(),
                trashed_at: trash.trashed_at,
            })
        })
        .collect();
    trash.sort_by(|a, b| {
        b.trashed_at
            .cmp(&a.trashed_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    Tree {
        pages: tree_pages,
        favorites: favorites
            .into_iter()
            .map(|(_, page)| ids[page].clone())
            .collect(),
        trash,
    }
}
