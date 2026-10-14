//! The page tree as the sidebar shows it, projected from the page records.

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
    let mut parent = drop_missing_parents(records);
    cut_cycles_at_min_id(records, &mut parent);
    let active = mark_active(records, &parent);
    Forest { parent, active }
}

/// A parent with no record makes its page top level.
fn drop_missing_parents(
    records: &BTreeMap<PageId, PageRecord>,
) -> BTreeMap<PageId, Option<PageId>> {
    records
        .iter()
        .map(|(id, record)| {
            let parent = record
                .placement
                .parent
                .clone()
                .filter(|p| records.contains_key(p));
            (id.clone(), parent)
        })
        .collect()
}

/// Each parent cycle loses the parent of its smallest page ID, so no page disappears.
fn cut_cycles_at_min_id(
    records: &BTreeMap<PageId, PageRecord>,
    parent: &mut BTreeMap<PageId, Option<PageId>>,
) {
    let mut done = BTreeSet::new();
    for start in records.keys() {
        let mut path: Vec<PageId> = Vec::new();
        let mut next = Some(start.clone());
        while let Some(page) = next {
            if done.contains(&page) {
                break;
            }
            if let Some(at) = path.iter().position(|p| *p == page) {
                let cut = path[at..].iter().min().expect("a cycle has pages").clone();
                parent.insert(cut, None);
                break;
            }
            next = parent[&page].clone();
            path.push(page);
        }
        done.extend(path);
    }
}

/// A page is active when neither it nor any ancestor is trashed.
fn mark_active(
    records: &BTreeMap<PageId, PageRecord>,
    parent: &BTreeMap<PageId, Option<PageId>>,
) -> BTreeSet<PageId> {
    let mut active = BTreeSet::new();
    let mut inactive = BTreeSet::new();
    for start in records.keys() {
        let mut path = Vec::new();
        let mut next = Some(start);
        let is_active = loop {
            let Some(page) = next else { break true };
            if active.contains(page) {
                break true;
            }
            if inactive.contains(page) || records[page].trash.is_some() {
                break false;
            }
            path.push(page.clone());
            next = parent[page].as_ref();
        };
        if is_active {
            active.extend(path);
        } else {
            inactive.extend(path);
        }
    }
    active
}

pub fn tree(records: &BTreeMap<PageId, PageRecord>) -> Tree {
    let Forest { parent, active } = forest(records);
    let mut children: BTreeMap<Option<&PageId>, Vec<&PageId>> = BTreeMap::new();
    for id in &active {
        children.entry(parent[id].as_ref()).or_default().push(id);
    }
    for siblings in children.values_mut() {
        siblings.sort_by_key(|id| (records[*id].placement.order, *id));
    }
    let mut pages = Vec::with_capacity(active.len());
    let mut stack: Vec<&PageId> = children.get(&None).cloned().unwrap_or_default();
    stack.reverse();
    while let Some(id) = stack.pop() {
        let record = &records[id];
        pages.push(TreePage {
            id: id.clone(),
            parent: parent[id].clone(),
            title: record.title.clone(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        });
        if let Some(kids) = children.get(&Some(id)) {
            stack.extend(kids.iter().rev());
        }
    }
    let mut favorites: Vec<(u64, &PageId)> = active
        .iter()
        .filter_map(|id| Some((records[id].favorite.as_ref()?.order, id)))
        .collect();
    favorites.sort();
    let mut trash: Vec<TrashedPage> = records
        .iter()
        .filter_map(|(id, record)| {
            let trash = record.trash.as_ref()?;
            let parent_batch = parent[id]
                .as_ref()
                .and_then(|p| records[p].trash.as_ref())
                .map(|t| &t.batch_id);
            (parent_batch != Some(&trash.batch_id)).then(|| TrashedPage {
                id: id.clone(),
                title: record.title.clone(),
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
        pages,
        favorites: favorites.into_iter().map(|(_, id)| id.clone()).collect(),
        trash,
    }
}
