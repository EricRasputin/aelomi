use std::collections::{BTreeMap, BTreeSet};
use std::{fmt, io};

use yrs::updates::decoder::Decode;
use yrs::{Doc, ReadTxn, Transact, Update};

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
    pub page: Option<PageId>,
    pub batch: Option<String>,
    pub seq: Seq,
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
        let _held = self
            .record_edits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.edit_page_locked(edit)
    }

    pub(crate) fn edit_page_locked(&self, edit: PageEdit) -> Result<PageEditReply, PageEditError> {
        let doc = Doc::new();
        doc.transact_mut()
            .apply_update(
                Update::decode_v1(&self.load(&DocId::workspace())?)
                    .map_err(|_| PageEditError::InvalidUpdate)?,
            )
            .map_err(|_| PageEditError::InvalidUpdate)?;
        let prior = read_pages(&doc.transact()).map_err(PageEditError::Malformed)?;
        let mut records = prior.clone();
        let now = self.store.unix_ms();
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
        let mut reply = PageEditReply {
            page: None,
            batch: None,
            seq: Seq(0),
        };
        match edit {
            PageEdit::Create { parent, title } => {
                if let Some(parent) = &parent {
                    active(parent)?;
                }
                let page = PageId::new(random_id()?).expect("hex is a page ID");
                let order = end_order(&records, parent.as_ref())?;
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
                reply.page = Some(page);
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
                let mut siblings: Vec<_> = forest
                    .active
                    .iter()
                    .filter(|id| **id != page && forest.parent[*id].as_ref() == parent.as_ref())
                    .cloned()
                    .collect();
                siblings.sort_by_key(|id| (records[id].placement.order, id.clone()));
                let order = match siblings.get(position) {
                    Some(next) => {
                        let order = records[next].placement.order;
                        make_room(
                            &mut records,
                            &forest.active,
                            parent.as_ref(),
                            order,
                            Some(&page),
                            now,
                        )?;
                        order
                    }
                    None => end_order(&records, parent.as_ref())?,
                };
                let record = records.get_mut(&page).expect("active page exists");
                record.placement = Placement { parent, order };
                record.updated_at = now;
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
                reply.batch = Some(batch);
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
                let mut active = forest.active.clone();
                let mut pending: Vec<_> = restoring.iter().cloned().collect();
                while !pending.is_empty() {
                    let at = pending
                        .iter()
                        .position(|id| {
                            let parent = &records[id]
                                .trash
                                .as_ref()
                                .expect("batch member")
                                .original_parent;
                            parent
                                .as_ref()
                                .is_none_or(|p| !restoring.contains(p) || active.contains(p))
                        })
                        .ok_or(PageEditError::Cycle)?;
                    let id = pending.remove(at);
                    let trash = records[&id].trash.clone().expect("batch member");
                    let parent = trash.original_parent.filter(|p| active.contains(p));
                    let order = if parent.is_none()
                        && records[&id]
                            .trash
                            .as_ref()
                            .unwrap()
                            .original_parent
                            .is_some()
                    {
                        end_order(&records, None)?
                    } else {
                        make_room(
                            &mut records,
                            &active,
                            parent.as_ref(),
                            trash.original_order,
                            None,
                            now,
                        )?;
                        trash.original_order
                    };
                    let record = records.get_mut(&id).expect("batch member");
                    record.placement = Placement { parent, order };
                    record.trash = None;
                    record.updated_at = now;
                    active.insert(id);
                }
                reply.batch = Some(batch);
            }
        }
        let vector = doc.transact().state_vector();
        {
            let mut txn = doc.transact_mut();
            for (id, record) in &records {
                if prior.get(id) != Some(record) {
                    write::put_page(&mut txn, id, record);
                }
            }
        }
        let update = doc.transact().encode_state_as_update_v1(&vector);
        reply.seq = self.append_record_update(&update)?;
        Ok(reply)
    }
}

const ORDER_STEP: u64 = 1 << 20;
fn end_order(
    records: &BTreeMap<PageId, PageRecord>,
    parent: Option<&PageId>,
) -> Result<u64, PageEditError> {
    records
        .values()
        .filter(|r| r.placement.parent.as_ref() == parent)
        .map(|r| r.placement.order)
        .max()
        .map_or(Ok(ORDER_STEP), |last| {
            last.checked_add(ORDER_STEP)
                .filter(|n| *n <= MAX_SAFE_INTEGER)
                .ok_or(PageEditError::OrderExhausted)
        })
}

fn make_room(
    records: &mut BTreeMap<PageId, PageRecord>,
    active: &BTreeSet<PageId>,
    parent: Option<&PageId>,
    order: u64,
    excluding: Option<&PageId>,
    now: u64,
) -> Result<(), PageEditError> {
    let mut occupied: BTreeMap<u64, PageId> = active
        .iter()
        .filter(|id| Some(*id) != excluding)
        .filter(|id| records[*id].placement.parent.as_ref() == parent)
        .map(|id| (records[id].placement.order, id.clone()))
        .collect();
    let mut free = order;
    while occupied.contains_key(&free) {
        free = free
            .checked_add(1)
            .filter(|n| *n <= MAX_SAFE_INTEGER)
            .ok_or(PageEditError::OrderExhausted)?;
    }
    while free > order {
        let id = occupied.remove(&(free - 1)).expect("occupied range");
        let record = records.get_mut(&id).expect("sibling exists");
        record.placement.order = free;
        record.updated_at = now;
        free -= 1;
    }
    Ok(())
}
