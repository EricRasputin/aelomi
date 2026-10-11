use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::Path;

use yrs::{ReadTxn, StateVector, Transact};

use super::doc_id::DocId;
use super::equality::Fingerprint;
use super::fs::create_durable;
use super::layout::{DocFile, Generation, SegmentNo};
use super::recover::snapshot_doc;
use super::snapshot::{encode_snapshot, read_snapshot};
use super::state::{LogSize, Segment};
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
    let result = compact(shared, doc);
    if let Some(state) = shared.slot(doc).state.lock().unwrap().as_mut() {
        state.compacting = false;
    }
    result
}

fn compact(shared: &Shared, doc: &DocId) -> Result<(), CompactError> {
    let rotated = rotate(shared, doc);
    let tmp = shared
        .layout
        .doc_file(doc, DocFile::SnapshotTmp(rotated.generation));
    create_durable(&*shared.fs, &tmp, &encode_snapshot(&rotated.state))?;
    verify(shared, &tmp, &rotated.expected)?;
    publish(shared, doc, &tmp, rotated.generation)?;
    retire_older(shared, doc, rotated.generation, &rotated.corrupt)?;
    Ok(())
}

struct Rotated {
    generation: Generation,
    state: Vec<u8>,
    expected: Fingerprint,
    corrupt: BTreeSet<String>,
}

fn rotate(shared: &Shared, doc: &DocId) -> Rotated {
    let slot = shared.slot(doc);
    let mut guard = slot.state.lock().unwrap();
    let state = guard.as_mut().expect("compaction runs on loaded docs");
    let generation = state.log.segment.generation().next();
    state.log.segment = Segment::Next(generation, SegmentNo(0));
    state.log.unsnapshotted = LogSize::default();
    let txn = state.ydoc.transact();
    Rotated {
        generation,
        state: txn.encode_state_as_update_v1(&StateVector::default()),
        expected: Fingerprint::of(&txn),
        corrupt: state.corrupt.clone(),
    }
}

fn verify(shared: &Shared, tmp: &Path, expected: &Fingerprint) -> Result<(), CompactError> {
    let written = shared.fs.read(tmp)?;
    let read_back = read_snapshot(&written).and_then(snapshot_doc);
    match read_back {
        Some(doc) if Fingerprint::of(&doc.transact()) == *expected => Ok(()),
        _ => Err(CompactError::Mismatch),
    }
}

fn publish(shared: &Shared, doc: &DocId, tmp: &Path, generation: Generation) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    fs.rename(tmp, &layout.doc_file(doc, DocFile::Snapshot(generation)))?;
    shared.sync_doc_dir(doc)?;
    fs.full_sync(&layout.doc_dir(doc))
}

fn retire_older(
    shared: &Shared,
    doc: &DocId,
    generation: Generation,
    corrupt: &BTreeSet<String>,
) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    let slot = shared.slot(doc);
    let _files = slot.files.lock().unwrap();
    let dir = layout.doc_dir(doc);
    let mut moved = BTreeSet::new();
    for name in fs.list(&dir)? {
        if DocFile::parse(&name).is_none_or(|f| f.generation() >= generation) {
            continue;
        }
        if corrupt.contains(&name) {
            quarantine(shared, doc, &name)?;
            moved.insert(name);
        } else {
            fs.remove(&dir.join(&name))?;
        }
    }
    shared.sync_doc_dir(doc)?;
    fs.full_sync(&dir)?;
    let mut state = slot.state.lock().unwrap();
    let state = state.as_mut().expect("still loaded");
    state.corrupt.retain(|name| !moved.contains(name));
    Ok(())
}

/// Moves the file without syncing: the caller's [`Shared::sync_doc_dir`] syncs where it went
/// before where it was.
fn quarantine(shared: &Shared, doc: &DocId, name: &str) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    let target_dir = layout.quarantine_dir(doc);
    shared.quarantine_created();
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
