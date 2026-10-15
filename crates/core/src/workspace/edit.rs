use std::collections::{BTreeMap, BTreeSet};
use std::{fmt, io};

use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

use super::records::{
    MAX_SAFE_INTEGER, PageId, PageRecord, Placement, RecordError, Trash, read_pages,
};
use super::{Workspace, tree, write};
use crate::storage::{AppendError, DocId, Seq};

#[derive(Clone, Debug)]
pub enum PageEdit {
    Create {
        parent: Option<PageId>,
        title: String,
    },
    Rename {
        page: PageId,
        title: String,
    },
    Move {
        page: PageId,
        parent: Option<PageId>,
        position: usize,
    },
    Trash {
        page: PageId,
    },
    Restore {
        batch: String,
    },
}

#[derive(Clone, Debug)]
pub struct PageEditReply {
    pub change: PageChange,
    pub seq: Seq,
}

#[derive(Clone, Debug)]
pub enum PageChange {
    Created(PageId),
    Edited,
    Trashed(String),
    Restored(String),
}

impl PageEditReply {
    pub fn created_page(self) -> Option<PageId> {
        match self.change {
            PageChange::Created(page) => Some(page),
            _ => None,
        }
    }
    pub fn trash_batch(self) -> Option<String> {
        match self.change {
            PageChange::Trashed(batch) | PageChange::Restored(batch) => Some(batch),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum PageEditError {
    Io(io::Error),
    Malformed(RecordError),
    NotActive,
    MissingBatch,
    Cycle,
    OrderExhausted,
    InvalidTime,
    InvalidUpdate,
}

impl fmt::Display for PageEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Malformed(e) => e.fmt(f),
            Self::NotActive => f.write_str("the page or parent is not active"),
            Self::MissingBatch => f.write_str("the trash batch does not exist"),
            Self::Cycle => f.write_str("a page cannot move under itself or its descendants"),
            Self::OrderExhausted => f.write_str("sibling orders exceed the safe integer range"),
            Self::InvalidTime => f.write_str("the clock exceeds the safe integer range"),
            Self::InvalidUpdate => f.write_str("the workspace update cannot be applied"),
        }
    }
}
impl std::error::Error for PageEditError {}
impl From<io::Error> for PageEditError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<AppendError> for PageEditError {
    fn from(e: AppendError) -> Self {
        match e {
            AppendError::Io(e) => Self::Io(e),
            AppendError::InvalidUpdate => Self::InvalidUpdate,
        }
    }
}

pub(crate) fn random_id() -> io::Result<String> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

impl Workspace {
    pub fn edit_page(&self, edit: PageEdit) -> Result<PageEditReply, PageEditError> {
        let held = self
            .record_edits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let doc = Doc::with_client_id(self.record_client.get());
        doc.transact_mut()
            .apply_update(
                Update::decode_v1(&self.load(&DocId::workspace())?)
                    .map_err(|_| PageEditError::InvalidUpdate)?,
            )
            .map_err(|_| PageEditError::InvalidUpdate)?;
        let prior = read_pages(&doc.transact()).map_err(PageEditError::Malformed)?;
        let (records, change) = plan(edit, &prior, self.store.unix_ms())?;
        let update = {
            let mut txn = doc.transact_mut();
            for (id, record) in &records {
                if prior.get(id) != Some(record) {
                    write::put_page(&mut txn, id, prior.get(id), record);
                }
            }
            txn.encode_update_v1()
        };
        let seq = self.append_record_update(&held, &update)?;
        Ok(PageEditReply { change, seq })
    }
}

fn plan(
    edit: PageEdit,
    prior: &BTreeMap<PageId, PageRecord>,
    now: u64,
) -> Result<(BTreeMap<PageId, PageRecord>, PageChange), PageEditError> {
    let mut records = prior.clone();
    if now > MAX_SAFE_INTEGER {
        return Err(PageEditError::InvalidTime);
    }
    let forest = tree::forest(&records);
    let active = |page: &PageId| {
        if forest.active.contains(page) {
            Ok(())
        } else {
            Err(PageEditError::NotActive)
        }
    };
    let mut change = PageChange::Edited;
    match edit {
        PageEdit::Create { parent, title } => {
            if let Some(parent) = &parent {
                active(parent)?;
            }
            let page = loop {
                let candidate = PageId::new(random_id()?).expect("hex is a page ID");
                if !records.contains_key(&candidate) {
                    break candidate;
                }
            };
            let order = last_order(&records, &siblings(&records, &forest, parent.as_ref()))?;
            records.insert(
                page.clone(),
                PageRecord {
                    title,
                    placement: Placement { parent, order },
                    favorite: None,
                    trash: None,
                    created_at: now,
                    updated_at: now,
                },
            );
            change = PageChange::Created(page);
        }
        PageEdit::Rename { page, title } => {
            active(&page)?;
            let record = records.get_mut(&page).expect("active page exists");
            record.title = title;
            record.updated_at = now;
        }
        PageEdit::Move {
            page,
            parent,
            position,
        } => {
            active(&page)?;
            let mut ancestor = parent.as_ref();
            while let Some(id) = ancestor {
                if id == &page {
                    return Err(PageEditError::Cycle);
                }
                active(id)?;
                ancestor = forest.parent[id].as_ref();
            }
            let siblings = siblings(&records, &forest, parent.as_ref());
            let current = siblings.iter().position(|id| id == &page);
            let position = position.min(
                siblings
                    .len()
                    .saturating_sub(usize::from(current.is_some())),
            );
            if forest.parent[&page] != parent || current != Some(position) {
                let mut cycle = BTreeSet::new();
                let mut next = Some(&page);
                while let Some(id) = next {
                    if !cycle.insert(id.clone()) {
                        if id == &page {
                            let cut = cycle.first().expect("cycle member");
                            if cut != &page {
                                records.get_mut(cut).expect("cycle member").placement.parent = None;
                            }
                        }
                        break;
                    }
                    next = prior
                        .get(id)
                        .and_then(|record| record.placement.parent.as_ref());
                }
                let siblings: Vec<_> = siblings.into_iter().filter(|id| id != &page).collect();
                let order = insertion_order(&mut records, &siblings, position)?;
                let record = records.get_mut(&page).expect("active page exists");
                record.placement = Placement { parent, order };
                record.updated_at = now;
            }
        }
        PageEdit::Trash { page } => {
            active(&page)?;
            let batch = random_id()?;
            let mut children: BTreeMap<Option<&PageId>, Vec<&PageId>> = BTreeMap::new();
            for id in &forest.active {
                children
                    .entry(forest.parent[id].as_ref())
                    .or_default()
                    .push(id);
            }
            let mut pending = vec![&page];
            while let Some(id) = pending.pop() {
                pending.extend(children.get(&Some(id)).into_iter().flatten().copied());
                let record = records.get_mut(id).expect("active page exists");
                record.trash = Some(Trash {
                    original_parent: record.placement.parent.clone(),
                    original_order: record.placement.order,
                    batch_id: batch.clone(),
                    trashed_at: now,
                });
                record.updated_at = now;
            }
            change = PageChange::Trashed(batch);
        }
        PageEdit::Restore { batch } => {
            let restoring: BTreeSet<_> = records
                .iter()
                .filter(|(_, r)| r.trash.as_ref().is_some_and(|t| t.batch_id == batch))
                .map(|(id, _)| id.clone())
                .collect();
            if restoring.is_empty() {
                return Err(PageEditError::MissingBatch);
            }
            let mut restored = records.clone();
            for id in &restoring {
                let record = restored.get_mut(id).expect("batch member");
                let trash = record.trash.take().expect("batch member");
                record.placement = Placement {
                    parent: trash.original_parent,
                    order: trash.original_order,
                };
            }
            let planned = tree::forest(&restored);
            for id in &restoring {
                restored.get_mut(id).expect("batch member").placement.parent = planned.parent[id]
                    .clone()
                    .filter(|parent| restoring.contains(parent) || forest.active.contains(parent));
            }
            let planned = tree::forest(&restored);
            let mut active = forest.active.clone();
            let mut positions: BTreeMap<_, _> = planned
                .active
                .iter()
                .filter(|id| !restoring.contains(*id))
                .map(|id| (id.clone(), records[id].placement.order))
                .collect();
            let mut children: BTreeMap<Option<PageId>, Vec<PageId>> = BTreeMap::new();
            let mut groups: BTreeMap<Option<PageId>, Vec<PageId>> = BTreeMap::new();
            for id in planned.active.iter().filter(|id| !restoring.contains(*id)) {
                groups
                    .entry(planned.parent[id].clone())
                    .or_default()
                    .push(id.clone());
            }
            for siblings in groups.values_mut() {
                siblings.sort_by_key(|id| (records[id].placement.order, id.clone()));
            }
            for id in &restoring {
                let parent = planned.parent[id].clone().filter(|p| restoring.contains(p));
                children.entry(parent).or_default().push(id.clone());
            }
            for siblings in children.values_mut() {
                siblings.sort_by_key(|id| (restored[id].placement.order, id.clone()));
            }
            let mut pending = children.remove(&None).unwrap_or_default();
            pending.reverse();
            while let Some(id) = pending.pop() {
                if let Some(children) = children.remove(&Some(id.clone())) {
                    pending.extend(children.into_iter().rev());
                }
                let trash = records[&id].trash.clone().expect("batch member");
                let parent = planned.parent[&id].clone().filter(|p| active.contains(p));
                let cut_root = planned.parent[&id].is_none()
                    && trash
                        .original_parent
                        .as_ref()
                        .is_some_and(|p| restoring.contains(p));
                let siblings = groups.entry(parent.clone()).or_default();
                let (position, order) =
                    if parent.is_none() && trash.original_parent.is_some() && !cut_root {
                        (siblings.len(), last_order(&records, siblings)?)
                    } else {
                        let position = siblings.partition_point(|sibling| {
                            (positions[sibling], sibling) < (trash.original_order, &id)
                        });
                        let order = match position
                            .checked_sub(1)
                            .map(|at| records[&siblings[at]].placement.order)
                        {
                            Some(previous) if previous >= trash.original_order => {
                                next_order(previous, 1)?
                            }
                            _ => trash.original_order,
                        };
                        make_room(&mut records, &siblings[position..], order)?;
                        (position, order)
                    };
                let sort_order = if parent.is_none() && trash.original_parent.is_some() && !cut_root
                {
                    order
                } else {
                    trash.original_order
                };
                positions.insert(id.clone(), sort_order);
                let record = records.get_mut(&id).expect("batch member");
                record.placement = Placement { parent, order };
                record.trash = None;
                record.updated_at = now;
                siblings.insert(position, id.clone());
                active.insert(id);
            }
            change = PageChange::Restored(batch);
        }
    }
    Ok((records, change))
}

pub(crate) const ORDER_STEP: u64 = 1 << 20;

fn siblings(
    records: &BTreeMap<PageId, PageRecord>,
    forest: &tree::Forest,
    parent: Option<&PageId>,
) -> Vec<PageId> {
    let mut siblings: Vec<_> = forest
        .active
        .iter()
        .filter(|id| forest.parent[*id].as_ref() == parent)
        .cloned()
        .collect();
    siblings.sort_by_key(|id| (records[id].placement.order, id.clone()));
    siblings
}

fn next_order(order: u64, step: u64) -> Result<u64, PageEditError> {
    order
        .checked_add(step)
        .filter(|n| *n <= MAX_SAFE_INTEGER)
        .ok_or(PageEditError::OrderExhausted)
}

fn last_order(
    records: &BTreeMap<PageId, PageRecord>,
    siblings: &[PageId],
) -> Result<u64, PageEditError> {
    siblings.last().map_or(Ok(ORDER_STEP), |id| {
        next_order(records[id].placement.order, ORDER_STEP)
    })
}

fn insertion_order(
    records: &mut BTreeMap<PageId, PageRecord>,
    siblings: &[PageId],
    position: usize,
) -> Result<u64, PageEditError> {
    let Some(next) = siblings.get(position) else {
        return last_order(records, siblings);
    };
    let next = records[next].placement.order;
    let order = if position == 0 {
        next.saturating_sub(ORDER_STEP)
    } else {
        let previous = records[&siblings[position - 1]].placement.order;
        if next.saturating_sub(previous) > 1 {
            previous + (next - previous) / 2
        } else {
            next_order(previous, 1)?
        }
    };
    make_room(records, &siblings[position..], order)?;
    Ok(order)
}

fn make_room(
    records: &mut BTreeMap<PageId, PageRecord>,
    siblings: &[PageId],
    order: u64,
) -> Result<(), PageEditError> {
    let mut occupied = order;
    for id in siblings {
        let record = records.get_mut(id).expect("sibling exists");
        if record.placement.order > occupied {
            break;
        }
        occupied = next_order(occupied, 1)?;
        record.placement.order = occupied;
    }
    Ok(())
}
