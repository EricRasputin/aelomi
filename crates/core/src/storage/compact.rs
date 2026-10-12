use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::Ordering;

use yrs::{Doc, ReadTxn, StateVector, Transact};

use super::decode;
use super::doc_id::DocId;
use super::equality::{Fingerprint, Ops};
use super::fs::create_durable;
use super::layout::{DocFile, Generation, SegmentNo};
use super::log::{LogEnd, SEGMENT_HEADER, read_log};
use super::snapshot::{self, Condition, SnapshotFormat, encode_snapshot};
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
    let format = SnapshotFormat::V1;
    let tmp = shared
        .layout
        .doc_file(doc, DocFile::SnapshotTmp(rotated.generation, format));
    create_durable(&*shared.fs, &tmp, &encode_snapshot(&snapshot.payload))?;
    if let Err(e) = verify(shared, format, &tmp, &snapshot.expected) {
        let _ = shared.fs.remove(&tmp);
        return Err(e);
    }
    publish(
        shared,
        doc,
        &tmp,
        DocFile::Snapshot(rotated.generation, format),
    )?;
    retire_older(shared, doc, &snapshot, rotated.generation)?;
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

fn verify(
    shared: &Shared,
    format: SnapshotFormat,
    tmp: &Path,
    expected: &Expected,
) -> Result<(), CompactError> {
    let written = shared.fs.read(tmp)?;
    let verified = snapshot::read(format, &written)
        .filter(|(_, condition)| *condition == Condition::Whole)
        .and_then(|(payload, _)| decode::update(payload).ok())
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

fn publish(shared: &Shared, doc: &DocId, tmp: &Path, published: DocFile) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    fs.rename(tmp, &layout.doc_file(doc, published))?;
    shared.sync_quarantine_then_doc_dir(doc)?;
    fs.full_sync(&layout.doc_dir(doc))
}

fn retire_older(
    shared: &Shared,
    doc: &DocId,
    snapshot: &Snapshot,
    generation: Generation,
) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    let slot = shared.slot(doc);
    let dir = layout.doc_dir(doc);
    let older = read_older(shared, &slot, &dir, generation)?;
    let coverage: Vec<Coverage> = older
        .iter()
        .map(|(_, file, bytes)| match file {
            DocFile::SnapshotTmp(..) => Coverage::Full { damaged: false },
            _ => coverage(snapshot.expected.ops(), *file, bytes),
        })
        .collect();
    let lacking: Vec<&[u8]> = older
        .iter()
        .zip(&coverage)
        .filter(|(_, coverage)| **coverage == Coverage::Lacking)
        .flat_map(|((_, file, bytes), _)| records(*file, bytes).0)
        .collect();
    let kept_files_add = !lacking.is_empty() && adds_to(&snapshot.payload, lacking);

    let _files = slot.files_waking_writer(shared);
    let (corrupt, acknowledged_before_failure) = {
        let state = slot.state();
        let state = state.as_ref().expect("still loaded");
        (
            state.corrupt.clone(),
            state.acknowledged_before_failure.clone(),
        )
    };
    let mut retired = BTreeSet::new();
    for ((name, _, bytes), coverage) in older.iter().zip(coverage) {
        let torn_by_a_failed_write = || {
            acknowledged_before_failure
                .get(name)
                .is_some_and(|&acknowledged| torn_after(acknowledged, bytes))
        };
        match coverage {
            Coverage::Lacking => {
                eprintln!(
                    "aelomi storage: kept {doc}/{name}, which has changes the snapshot lacks"
                );
                if kept_files_add {
                    slot.state().as_mut().expect("still loaded").ydoc = Ydoc::Stale;
                }
                continue;
            }
            Coverage::Full { damaged }
                if !corrupt.contains(name) && (!damaged || torn_by_a_failed_write()) =>
            {
                fs.remove(&dir.join(name))?;
            }
            Coverage::Full { .. } => move_to_quarantine_unsynced(shared, doc, name)?,
        }
        retired.insert(name.clone());
    }
    shared.sync_quarantine_then_doc_dir(doc)?;
    fs.full_sync(&dir)?;
    let mut state = slot.state();
    let state = state.as_mut().expect("still loaded");
    state.corrupt.retain(|name| !retired.contains(name));
    state
        .acknowledged_before_failure
        .retain(|name, _| !retired.contains(name));
    Ok(())
}

fn read_older(
    shared: &Shared,
    slot: &DocSlot,
    dir: &Path,
    generation: Generation,
) -> io::Result<Vec<(String, DocFile, Vec<u8>)>> {
    let fs = &*shared.fs;
    let _files = slot.files_waking_writer(shared);
    let mut older = Vec::new();
    for name in fs.list(dir)? {
        let Some(file) = DocFile::parse(&name).filter(|f| f.generation() < generation) else {
            continue;
        };
        let bytes = match file {
            DocFile::SnapshotTmp(..) => Vec::new(),
            _ => fs.read(&dir.join(&name))?,
        };
        older.push((name, file, bytes));
    }
    Ok(older)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Coverage {
    Full { damaged: bool },
    Lacking,
}

/// Compares IDs, not docs: yrs integrates pending structs differently when they arrive
/// again, so a doc can change without gaining anything.
pub(crate) fn coverage(held: &Ops, file: DocFile, bytes: &[u8]) -> Coverage {
    if let DocFile::SnapshotTmp(..) = file {
        return Coverage::Lacking;
    }
    let (records, damaged) = records(file, bytes);
    let holds = records.iter().all(|record| {
        decode::update(record).is_ok_and(|update| held.contains(&Ops::of_update(&update)))
    });
    if holds {
        Coverage::Full { damaged }
    } else {
        Coverage::Lacking
    }
}

pub(crate) fn torn_after(acknowledged: u64, bytes: &[u8]) -> bool {
    let acknowledged = acknowledged as usize;
    let before = bytes.get(..acknowledged).map(read_log);
    read_log(bytes).end == LogEnd::Torn
        && (acknowledged == 0 || before.is_some_and(|before| before.end == LogEnd::Clean))
}

fn records(file: DocFile, bytes: &[u8]) -> (Vec<&[u8]>, bool) {
    match file {
        DocFile::Snapshot(_, format) => match snapshot::read(format, bytes) {
            Some((payload, condition)) => (vec![payload], condition == Condition::Damaged),
            None => (Vec::new(), true),
        },
        DocFile::Log(..) => {
            let contents = read_log(bytes);
            let holds_record_bytes = bytes.len() > SEGMENT_HEADER;
            let damaged = match contents.end {
                LogEnd::Clean => false,
                LogEnd::Torn => holds_record_bytes,
                LogEnd::Corrupt => true,
            };
            (contents.records, damaged)
        }
        DocFile::SnapshotTmp(..) => (Vec::new(), false),
    }
}

fn adds_to<'a>(snapshot: &[u8], records: impl IntoIterator<Item = &'a [u8]>) -> bool {
    let doc = Doc::new();
    let mut txn = doc.transact_mut();
    if let Ok(update) = decode::update(snapshot) {
        let _ = txn.apply_update(update);
    }
    let before = Ops::of_doc(&txn);
    for record in records {
        if let Ok(update) = decode::update(record) {
            let _ = txn.apply_update(update);
        }
    }
    before.is_none() || before != Ops::of_doc(&txn)
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
    use yrs::Transact;

    use super::{Expected, decode};
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
            let held = Ops::of_update(&decode::update(held).unwrap());
            expected.met_by(&held, &doc_from([loaded]).transact())
        };
        assert!(verifies(&all.payload, &all.payload));
        assert!(!verifies(&one_less, &all.payload), "holds one less");
        assert!(!verifies(&one_more, &one_more), "holds one more");
        assert!(!verifies(&all.payload, &one_less), "loads one less");
    }
}
