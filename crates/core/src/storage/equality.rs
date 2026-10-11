use std::collections::BTreeMap;

use yrs::branch::{Branch, BranchPtr};
use yrs::types::ToJson;
use yrs::updates::decoder::Decode;
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
    let update = Update::decode_v1(&state).ok()?;
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
