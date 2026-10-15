use std::collections::{BTreeMap, BTreeSet};

use super::decode;

use yrs::branch::{Branch, BranchPtr};
use yrs::types::TypeRef;
use yrs::types::text::YChange;
use yrs::{
    Any, Array, ArrayRef, BranchID, Doc, GetString, IdSet, Map, MapRef, OffsetKind, Out, ReadTxn,
    Snapshot, StateVector, Text, TextRef, Transact, Update,
};

#[derive(Debug, PartialEq)]
pub(crate) struct Fingerprint {
    snapshot: Snapshot,
    branches: BTreeMap<BranchID, Node>,
    pending: Option<PendingStructs>,
}

/// A root decoded from an update has no type until someone asks for one, so a root records
/// none, and every branch is read as text runs, as a sequence, and as a map.
#[derive(Debug, PartialEq)]
struct Node {
    kind: Option<TypeRef>,
    content: Content,
}

#[derive(Debug, Default, PartialEq)]
struct Content {
    runs: Vec<Run>,
    sequence: Vec<Value>,
    entries: BTreeMap<String, Value>,
}

#[derive(Debug, PartialEq)]
struct Run {
    insert: Value,
    marks: BTreeMap<String, Any>,
}

#[derive(Debug, PartialEq)]
enum Value {
    Any(Any),
    Nested(BranchID),
    Doc(String),
}

#[derive(Debug, PartialEq)]
struct PendingStructs {
    missing: StateVector,
    inserts: IdSet,
    deletes: IdSet,
    pending_deletes: Option<IdSet>,
}

impl Fingerprint {
    /// `None` when yrs can't apply its own encoding of the doc's pending structs, or when a branch
    /// holds text beside values its text runs leave out. `offsets` is the doc's offset kind.
    pub fn of(txn: &impl ReadTxn, offsets: OffsetKind) -> Option<Self> {
        let pending = if txn.has_missing_updates() {
            Some(settled_pending(txn)?)
        } else {
            None
        };
        let mut reader = Reader {
            txn,
            offsets,
            found: Vec::new(),
            seen: BTreeSet::new(),
        };
        let mut branches = BTreeMap::new();
        for (_, root) in txn.root_refs() {
            if let Ok(root) = shared(root) {
                let content = reader.content(root)?;
                if content != Content::default() {
                    branches.insert(
                        root.id(),
                        Node {
                            kind: None,
                            content,
                        },
                    );
                }
            }
        }
        while let Some(branch) = reader.found.pop() {
            let kind = Some(branch.type_ref().clone());
            let content = reader.content(branch)?;
            branches.insert(branch.id(), Node { kind, content });
        }
        Some(Self {
            snapshot: txn.snapshot(),
            branches,
            pending,
        })
    }
}

/// Collects nested branches as it meets them, so a deep doc never deepens the stack.
struct Reader<'a, T> {
    txn: &'a T,
    offsets: OffsetKind,
    found: Vec<BranchPtr>,
    seen: BTreeSet<BranchID>,
}

impl<T: ReadTxn> Reader<'_, T> {
    fn content(&mut self, branch: BranchPtr) -> Option<Content> {
        let txn = self.txn;
        let text = TextRef::from(branch);
        let runs = text.diff(txn, YChange::identity).into_iter();
        let runs = runs.map(|run| Run {
            insert: self.value(run.insert),
            marks: run.attributes.map_or_else(BTreeMap::new, |marks| {
                marks.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
            }),
        });
        let runs: Vec<Run> = runs.collect();
        // yrs's sequence iterator never ends on a string with a multi-byte character.
        let string = text.get_string(txn);
        let sequence = if string.is_empty() {
            let values = ArrayRef::from(branch).iter(txn);
            values.map(|value| self.value(value)).collect()
        } else if self.runs_cover(&runs, &string, branch.content_len()) {
            Vec::new()
        } else {
            return None;
        };
        let map = MapRef::from(branch);
        let entries = map.iter(txn);
        let entries = entries.map(|(key, value)| (key.to_owned(), self.value(value)));
        Some(Content {
            runs,
            sequence,
            entries: entries.collect(),
        })
    }

    /// Text runs leave out values, binaries and subdocs, and a string embed reads like text, so
    /// the runs must spell the branch's string and count up to its length, each other run as 1.
    fn runs_cover(&self, runs: &[Run], string: &str, content_len: u32) -> bool {
        let mut spelled = String::new();
        let mut others: u64 = 0;
        for run in runs {
            match &run.insert {
                Value::Any(Any::String(s)) => spelled.push_str(s),
                _ => others += 1,
            }
        }
        let string_len = match self.offsets {
            OffsetKind::Bytes => string.len(),
            OffsetKind::Utf16 => string.encode_utf16().count(),
        };
        spelled == string && string_len as u64 + others == u64::from(content_len)
    }

    fn value(&mut self, out: Out) -> Value {
        match shared(out) {
            Ok(branch) => {
                let id = branch.id();
                if self.seen.insert(id.clone()) {
                    self.found.push(branch);
                }
                Value::Nested(id)
            }
            Err(value) => value,
        }
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

fn shared(out: Out) -> Result<BranchPtr, Value> {
    let branch: &Branch = match &out {
        Out::UndefinedRef(b) => return Ok(*b),
        Out::YText(t) => t.as_ref(),
        Out::YArray(a) => a.as_ref(),
        Out::YMap(m) => m.as_ref(),
        Out::YXmlElement(e) => e.as_ref(),
        Out::YXmlFragment(f) => f.as_ref(),
        Out::YXmlText(t) => t.as_ref(),
        Out::YDoc(doc) => return Err(Value::Doc(doc.guid().to_string())),
        Out::Any(any) => return Err(Value::Any(any.clone())),
    };
    Ok(BranchPtr::from(branch))
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
