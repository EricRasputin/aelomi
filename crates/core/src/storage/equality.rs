use std::collections::BTreeMap;

use super::decode;

use yrs::branch::{Branch, BranchPtr};
use yrs::types::ToJson;
use yrs::{
    Any, ArrayRef, Doc, IdSet, MapRef, Out, ReadTxn, Snapshot, StateVector, Transact, Update,
};

#[derive(Debug, PartialEq)]
pub(crate) struct Fingerprint {
    snapshot: Snapshot,
    roots: BTreeMap<String, Root>,
    pending: Option<PendingStructs>,
}

/// A root decoded from an update has no type until someone asks for one, so it is read both
/// as a sequence and as a map.
#[derive(Debug, PartialEq)]
struct Root {
    sequence: Any,
    entries: Any,
}

#[derive(Debug, PartialEq)]
struct PendingStructs {
    missing: StateVector,
    inserts: IdSet,
    deletes: IdSet,
    pending_deletes: Option<IdSet>,
}

impl Fingerprint {
    /// `None` when yrs can't apply its own encoding of the doc's pending structs.
    pub fn of(txn: &impl ReadTxn) -> Option<Self> {
        let pending = if txn.has_missing_updates() {
            Some(settled_pending(txn)?)
        } else {
            None
        };
        let roots = txn
            .root_refs()
            .filter_map(|(name, root)| {
                let branch = branch(&root)?;
                let sequence = ArrayRef::from(branch).to_json(txn);
                let entries = MapRef::from(branch).to_json(txn);
                let empty = matches!((&sequence, &entries), (Any::Array(s), Any::Map(m)) if s.is_empty() && m.is_empty());
                (!empty).then(|| (name.to_owned(), Root { sequence, entries }))
            })
            .collect();
        Some(Self {
            snapshot: txn.snapshot(),
            roots,
            pending,
        })
    }
}

/// How yrs records pending structs depends on the order and grouping they arrived in, but one
/// encode and decode settles it, and another changes nothing. So they're compared settled.
fn settled_pending(txn: &impl ReadTxn) -> Option<PendingStructs> {
    let state = txn.encode_state_as_update_v1(&StateVector::default());
    let settled = Doc::new();
    let update = decode::update(&state).ok()?;
    settled.transact_mut().apply_update(update).ok()?;
    let txn = settled.transact();
    let store = txn.store();
    let (missing, inserts, deletes) = match store.pending_update() {
        Some(p) => (
            p.missing.clone(),
            p.update.insertions(true),
            p.update.delete_set().clone(),
        ),
        None => Default::default(),
    };
    Some(PendingStructs {
        missing,
        inserts,
        deletes,
        pending_deletes: store.pending_ds().cloned(),
    })
}

fn branch(root: &Out) -> Option<BranchPtr> {
    let branch: &Branch = match root {
        Out::UndefinedRef(b) => return Some(*b),
        Out::YText(t) => t.as_ref(),
        Out::YArray(a) => a.as_ref(),
        Out::YMap(m) => m.as_ref(),
        Out::YXmlElement(e) => e.as_ref(),
        Out::YXmlFragment(f) => f.as_ref(),
        Out::YXmlText(t) => t.as_ref(),
        Out::Any(_) | Out::YDoc(_) => return None,
    };
    Some(BranchPtr::from(branch))
}

const INCLUDE_DELETED: bool = true;

#[derive(Debug, Default)]
pub(crate) struct Ops {
    inserted: IdSet,
    deleted: IdSet,
}

impl Ops {
    pub fn of_update(update: &Update) -> Self {
        Self {
            inserted: update.insertions(INCLUDE_DELETED),
            deleted: update.delete_set().clone(),
        }
    }

    /// Takes integrated blocks from yrs's plain encoding, since a client's blocks can start past
    /// its state vector, and pending structs from the pending update itself, since yrs's
    /// full-state encoding can renumber them.
    pub fn of_doc(txn: &impl ReadTxn) -> Option<Self> {
        let integrated = decode::update(&txn.encode_diff_v1(&StateVector::default())).ok()?;
        let mut ops = Self::of_update(&integrated);
        let store = txn.store();
        if let Some(pending) = store.pending_update() {
            ops.inserted
                .merge_with(pending.update.insertions(INCLUDE_DELETED));
            ops.deleted.merge_with(pending.update.delete_set().clone());
        }
        if let Some(pending) = store.pending_ds() {
            ops.deleted.merge_with(pending.clone());
        }
        Some(ops)
    }

    pub fn merge(&mut self, other: Self) {
        self.inserted.merge_with(other.inserted);
        self.deleted.merge_with(other.deleted);
    }

    pub fn contains(&self, other: &Self) -> bool {
        other.inserted.diff(&self.inserted).is_empty()
            && other.deleted.diff(&self.deleted).is_empty()
    }

    /// A doc can delete more than its updates do, since yrs deletes what it integrates into a
    /// deleted parent, but only blocks they insert or delete.
    #[cfg(test)]
    pub fn only_from(&self, updates: &Self) -> bool {
        let mut touched = updates.inserted.clone();
        touched.merge_with(updates.deleted.clone());
        self.inserted.diff(&updates.inserted).is_empty() && self.deleted.diff(&touched).is_empty()
    }
}

impl PartialEq for Ops {
    fn eq(&self, other: &Self) -> bool {
        self.contains(other) && other.contains(self)
    }
}
