use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::Ordering;

use yrs::updates::decoder::Decode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};

use super::doc_id::DocId;
use super::equality::{Fingerprint, Ops};
use super::fs::create_durable;
use super::layout::{DocFile, Generation, SegmentNo};
use super::log::{LogEnd, read_log};
use super::snapshot::{encode_snapshot, read_snapshot};
use super::state::{Compaction, DocSlot, LogSize, Segment, Ydoc};
use super::store::Shared;

#[derive(Debug)]
pub(crate) enum CompactError {
    Io(io::Error),
    Mismatch,
    Panicked,
}

pub(crate) fn run(shared: &Shared) {
    while let Some(doc) = shared.wait_for_compaction() {
        if let Err(e) = compact_queued(shared, &doc) {
            eprintln!("aelomi storage: compacting {doc} failed: {e}");
        }
    }
}

pub(crate) fn compact_queued(shared: &Shared, doc: &DocId) -> Result<(), CompactError> {
    panic::catch_unwind(AssertUnwindSafe(|| {
        let rotated = rotate(shared, doc);
        finish(shared, doc, rotated)
    }))
    .unwrap_or_else(|_| {
        shared.compaction_ended(doc);
        Err(CompactError::Panicked)
    })
}

pub(crate) fn finish(
    shared: &Shared,
    doc: &DocId,
    rotated: Option<Rotated>,
) -> Result<(), CompactError> {
    let result = rotated.map_or(Ok(()), |rotated| compact(shared, doc, rotated));
    shared.compaction_ended(doc);
    result
}

fn compact(shared: &Shared, doc: &DocId, rotated: Rotated) -> Result<(), CompactError> {
    let snapshot = match rotated.source {
        Source::Encoded(snapshot) => *snapshot,
        Source::MergeHeld => match merged(shared, doc)? {
            Some(snapshot) => snapshot,
            None => return Ok(()),
        },
    };
    let tmp = shared
        .layout
        .doc_file(doc, DocFile::SnapshotTmp(rotated.generation));
    create_durable(&*shared.fs, &tmp, &encode_snapshot(&snapshot.payload))?;
    if let Err(e) = verify(shared, &tmp, &snapshot.expected) {
        let _ = shared.fs.remove(&tmp);
        return Err(e);
    }
    publish(shared, doc, &tmp, rotated.generation)?;
    retire_older(shared, doc, snapshot.expected.ops(), rotated.generation)?;
    Ok(())
}

pub(crate) struct Rotated {
    generation: Generation,
    source: Source,
}

enum Source {
    Encoded(Box<Snapshot>),
    MergeHeld,
}

struct Snapshot {
    payload: Vec<u8>,
    expected: Expected,
}

enum Expected {
    Equal {
        ops: Ops,
        fingerprint: Fingerprint,
    },
    /// Loading can integrate structs yrs had left pending, and yrs deletes any that land in a
    /// deleted parent, so the loaded doc can hold more deletions.
    Merged(Ops),
}

impl Expected {
    fn of(txn: &impl ReadTxn) -> Option<Self> {
        Some(Self::Equal {
            ops: Ops::of_doc(txn)?,
            fingerprint: Fingerprint::of(txn)?,
        })
    }

    fn ops(&self) -> &Ops {
        match self {
            Self::Equal { ops, .. } | Self::Merged(ops) => ops,
        }
    }

    fn met_by(&self, held: &Ops, loaded: &impl ReadTxn) -> bool {
        let Some(loaded_ops) = Ops::of_doc(loaded) else {
            return false;
        };
        match self {
            Self::Equal { ops, fingerprint } => {
                held == ops
                    && loaded_ops == *ops
                    && Fingerprint::of(loaded).as_ref() == Some(fingerprint)
            }
            Self::Merged(ops) => held == ops && loaded_ops.contains(ops),
        }
    }
}

pub(crate) fn rotate(shared: &Shared, doc: &DocId) -> Option<Rotated> {
    let slot = shared.slot(doc);
    let mut guard = slot.state();
    let state = guard.as_mut().expect("compaction runs on loaded docs");
    let Ydoc::Fresh(ydoc) = state.ydoc.clone() else {
        return None;
    };
    let txn = ydoc.transact();
    state.log.unsnapshotted = LogSize::default();
    let source = if txn.has_missing_updates() {
        Source::MergeHeld
    } else {
        let payload = txn.encode_state_as_update_v1(&StateVector::default());
        if too_large(shared, doc, &payload) {
            return None;
        }
        let Some(expected) = Expected::of(&txn) else {
            eprintln!(
                "aelomi storage: skipped compacting {doc}, as yrs can't decode its own encoding"
            );
            return None;
        };
        Source::Encoded(Box::new(Snapshot { payload, expected }))
    };
    state.compaction = Compaction::Running {
        asked_after_rotate: false,
        stale_when_done: false,
    };
    let generation = state.log.segment.generation().next();
    state.log.segment = Segment::Next(generation, SegmentNo(0));
    Some(Rotated { generation, source })
}

fn merged(shared: &Shared, doc: &DocId) -> io::Result<Option<Snapshot>> {
    let merged = shared.merge_held(doc)?;
    if too_large(shared, doc, &merged.payload) {
        return Ok(None);
    }
    Ok(Some(Snapshot {
        payload: merged.payload,
        expected: Expected::Merged(merged.ops),
    }))
}

fn too_large(shared: &Shared, doc: &DocId, payload: &[u8]) -> bool {
    let too_large = payload.len() > shared.limits.max_payload as usize;
    if too_large {
        eprintln!("aelomi storage: skipped compacting {doc}, as its snapshot is too large");
    }
    too_large
}

fn verify(shared: &Shared, tmp: &Path, expected: &Expected) -> Result<(), CompactError> {
    let written = shared.fs.read(tmp)?;
    let verified = read_snapshot(&written)
        .and_then(|payload| Update::decode_v1(payload).ok())
        .is_some_and(|update| {
            let held = Ops::of_update(&update);
            let loaded = Doc::new();
            let applied = loaded.transact_mut().apply_update(update).is_ok();
            applied && expected.met_by(&held, &loaded.transact())
        });
    if verified {
        Ok(())
    } else {
        Err(CompactError::Mismatch)
    }
}

fn publish(shared: &Shared, doc: &DocId, tmp: &Path, generation: Generation) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    fs.rename(tmp, &layout.doc_file(doc, DocFile::Snapshot(generation)))?;
    shared.sync_quarantine_then_doc_dir(doc)?;
    fs.full_sync(&layout.doc_dir(doc))
}

fn retire_older(
    shared: &Shared,
    doc: &DocId,
    held: &Ops,
    generation: Generation,
) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    let slot = shared.slot(doc);
    let dir = layout.doc_dir(doc);
    let coverage: Vec<(String, Coverage)> = read_older(shared, &slot, &dir, generation)?
        .into_iter()
        .map(|(name, file, bytes)| {
            let coverage = match file {
                DocFile::SnapshotTmp(_) => Coverage::Whole,
                _ => coverage(held, file, &bytes),
            };
            (name, coverage)
        })
        .collect();

    let _files = slot.files();
    let corrupt = slot.state().as_ref().expect("still loaded").corrupt.clone();
    let mut moved = BTreeSet::new();
    for (name, coverage) in coverage {
        if corrupt.contains(&name) || coverage == Coverage::DamagedButHeld {
            move_to_quarantine_unsynced(shared, doc, &name)?;
            moved.insert(name);
        } else if coverage == Coverage::Whole {
            fs.remove(&dir.join(&name))?;
        } else {
            eprintln!("aelomi storage: kept {doc}/{name}, which has changes the snapshot lacks");
            slot.state().as_mut().expect("still loaded").ydoc = Ydoc::Stale;
        }
    }
    shared.sync_quarantine_then_doc_dir(doc)?;
    fs.full_sync(&dir)?;
    let mut state = slot.state();
    let state = state.as_mut().expect("still loaded");
    state.corrupt.retain(|name| !moved.contains(name));
    Ok(())
}

fn read_older(
    shared: &Shared,
    slot: &DocSlot,
    dir: &Path,
    generation: Generation,
) -> io::Result<Vec<(String, DocFile, Vec<u8>)>> {
    let fs = &*shared.fs;
    let _files = slot.files();
    let mut older = Vec::new();
    for name in fs.list(dir)? {
        let Some(file) = DocFile::parse(&name).filter(|f| f.generation() < generation) else {
            continue;
        };
        let bytes = match file {
            DocFile::SnapshotTmp(_) => Vec::new(),
            _ => fs.read(&dir.join(&name))?,
        };
        older.push((name, file, bytes));
    }
    Ok(older)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Coverage {
    Whole,
    DamagedButHeld,
    Lacking,
}

/// Compares IDs, not docs: yrs integrates pending structs differently when they arrive
/// again, so a doc can change without gaining anything.
pub(crate) fn coverage(held: &Ops, file: DocFile, bytes: &[u8]) -> Coverage {
    let (records, damaged) = match file {
        DocFile::Snapshot(_) => match read_snapshot(bytes) {
            Some(payload) => (vec![payload], false),
            None => (Vec::new(), true),
        },
        DocFile::Log(..) => {
            let contents = read_log(bytes);
            (contents.records, contents.end != LogEnd::Clean)
        }
        DocFile::SnapshotTmp(_) => return Coverage::Lacking,
    };
    let holds = records.iter().all(|record| {
        Update::decode_v1(record).is_ok_and(|update| held.contains(&Ops::of_update(&update)))
    });
    match (holds, damaged) {
        (false, _) => Coverage::Lacking,
        (true, true) => Coverage::DamagedButHeld,
        (true, false) => Coverage::Whole,
    }
}

fn move_to_quarantine_unsynced(shared: &Shared, doc: &DocId, name: &str) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    let target_dir = layout.quarantine_dir(doc);
    shared.quarantine_exists.store(true, Ordering::SeqCst);
    for dir in [layout.quarantine(), target_dir.clone()] {
        match fs.create_dir(&dir) {
            Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
            _ => {}
        }
    }
    let taken = fs.list(&target_dir)?;
    let target = std::iter::once(name.to_owned())
        .chain((1..).map(|n| format!("{name}.{n}")))
        .find(|candidate| !taken.contains(candidate))
        .expect("an unused name");
    fs.rename(&layout.doc_dir(doc).join(name), &target_dir.join(target))
}

impl From<io::Error> for CompactError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for CompactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Mismatch => f.write_str("the snapshot read back differs from the doc"),
            Self::Panicked => f.write_str("it panicked, so its files stay as they were"),
        }
    }
}

#[cfg(test)]
mod tests {
    use yrs::updates::decoder::Decode;
    use yrs::{Transact, Update};

    use super::Expected;
    use crate::storage::equality::Ops;
    use crate::storage::recover::merge;
    use crate::storage::testkit::{UpdateGen, doc_from};

    #[test]
    fn a_merge_verifies_only_when_it_holds_exactly_and_loads_every_operation_merged() {
        let updates = UpdateGen::new(9, 3).updates(21);
        let merged = |n: usize| merge(updates[..n].iter().map(Vec::as_slice));
        let all = merged(20);
        let (one_less, one_more) = (merged(19).payload, merged(21).payload);
        let expected = Expected::Merged(all.ops);
        let verifies = |held: &[u8], loaded: &[u8]| {
            let held = Ops::of_update(&Update::decode_v1(held).unwrap());
            expected.met_by(&held, &doc_from([loaded]).transact())
        };
        assert!(verifies(&all.payload, &all.payload));
        assert!(!verifies(&one_less, &all.payload), "holds one less");
        assert!(!verifies(&one_more, &one_more), "holds one more");
        assert!(!verifies(&all.payload, &one_less), "loads one less");
    }
}
