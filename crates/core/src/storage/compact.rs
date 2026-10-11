use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::Path;
use std::sync::atomic::Ordering;

use yrs::updates::decoder::Decode;
use yrs::{IdSet, ReadTxn, StateVector, Transact, Update};

use super::doc_id::DocId;
use super::equality::Fingerprint;
use super::fs::create_durable;
use super::layout::{DocFile, Generation, SegmentNo};
use super::log::{LogEnd, read_log};
use super::recover::snapshot_doc;
use super::snapshot::{encode_snapshot, read_snapshot};
use super::state::{Compaction, DocSlot, LogSize, Segment, Ydoc};
use super::store::Shared;

#[derive(Debug)]
pub(crate) enum CompactError {
    Io(io::Error),
    Mismatch,
}

pub(crate) fn run(shared: &Shared) {
    while let Some(doc) = shared.wait_for_compaction() {
        if let Err(e) = compact_queued(shared, &doc) {
            eprintln!("aelomi storage: compacting {doc} failed: {e}");
        }
    }
}

pub(crate) fn compact_queued(shared: &Shared, doc: &DocId) -> Result<(), CompactError> {
    let rotated = rotate(shared, doc);
    finish(shared, doc, rotated)
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
    let tmp = shared
        .layout
        .doc_file(doc, DocFile::SnapshotTmp(rotated.generation));
    create_durable(&*shared.fs, &tmp, &encode_snapshot(&rotated.state))?;
    verify(shared, &tmp, &rotated.expected)?;
    publish(shared, doc, &tmp, rotated.generation)?;
    retire_older(shared, doc, &rotated)?;
    Ok(())
}

pub(crate) struct Rotated {
    generation: Generation,
    state: Vec<u8>,
    expected: Fingerprint,
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
    let snapshot = txn.encode_state_as_update_v1(&StateVector::default());
    if snapshot.len() > shared.limits.max_payload as usize {
        eprintln!("aelomi storage: skipped compacting {doc}, as its snapshot is too large");
        return None;
    }
    let Some(expected) = Fingerprint::of(&txn) else {
        eprintln!(
            "aelomi storage: skipped compacting {doc}, as yrs can't apply its own encoding of it"
        );
        return None;
    };
    state.compaction = Compaction::Running {
        asked_after_rotate: false,
    };
    let generation = state.log.segment.generation().next();
    state.log.segment = Segment::Next(generation, SegmentNo(0));
    Some(Rotated {
        generation,
        state: snapshot,
        expected,
    })
}

fn verify(shared: &Shared, tmp: &Path, expected: &Fingerprint) -> Result<(), CompactError> {
    let written = shared.fs.read(tmp)?;
    let read_back = read_snapshot(&written).and_then(snapshot_doc);
    match read_back {
        Some(doc) if Fingerprint::of(&doc.transact()).as_ref() == Some(expected) => Ok(()),
        _ => Err(CompactError::Mismatch),
    }
}

fn publish(shared: &Shared, doc: &DocId, tmp: &Path, generation: Generation) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    fs.rename(tmp, &layout.doc_file(doc, DocFile::Snapshot(generation)))?;
    shared.sync_quarantine_then_doc_dir(doc)?;
    fs.full_sync(&layout.doc_dir(doc))
}

fn retire_older(shared: &Shared, doc: &DocId, rotated: &Rotated) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    let slot = shared.slot(doc);
    let dir = layout.doc_dir(doc);
    let Some(held) = SnapshotIds::of(&rotated.state) else {
        eprintln!(
            "aelomi storage: kept every older file of {doc}, as yrs can't decode its snapshot"
        );
        return Ok(());
    };
    let covered: Vec<(String, bool)> = read_older(shared, &slot, &dir, rotated.generation)?
        .into_iter()
        .map(|(name, file, bytes)| {
            let covered = matches!(file, DocFile::SnapshotTmp(_)) || held.cover(file, &bytes);
            (name, covered)
        })
        .collect();

    let _files = slot.files();
    let corrupt = slot.state().as_ref().expect("still loaded").corrupt.clone();
    let mut moved = BTreeSet::new();
    for (name, covered) in covered {
        if corrupt.contains(&name) {
            move_to_quarantine_unsynced(shared, doc, &name)?;
            moved.insert(name);
        } else if covered {
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

const INCLUDE_DELETED: bool = true;

pub(crate) struct SnapshotIds {
    inserted: IdSet,
    deleted: IdSet,
}

impl SnapshotIds {
    pub fn of(payload: &[u8]) -> Option<Self> {
        let update = Update::decode_v1(payload).ok()?;
        Some(Self {
            inserted: update.insertions(INCLUDE_DELETED),
            deleted: update.delete_set().clone(),
        })
    }

    /// Compares IDs, not docs: yrs integrates pending structs differently when they arrive
    /// again, so a doc can change without gaining anything.
    pub fn cover(&self, file: DocFile, bytes: &[u8]) -> bool {
        let records = match file {
            DocFile::Snapshot(_) => read_snapshot(bytes).map(|payload| vec![payload]),
            DocFile::Log(..) => {
                let contents = read_log(bytes);
                (contents.end != LogEnd::Corrupt).then_some(contents.records)
            }
            DocFile::SnapshotTmp(_) => None,
        };
        records.is_some_and(|records| {
            records.iter().all(|record| {
                Update::decode_v1(record).is_ok_and(|update| {
                    update
                        .insertions(INCLUDE_DELETED)
                        .diff(&self.inserted)
                        .is_empty()
                        && update.delete_set().diff(&self.deleted).is_empty()
                })
            })
        })
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
        }
    }
}
