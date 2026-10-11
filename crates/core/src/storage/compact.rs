use std::collections::BTreeSet;
use std::fmt;
use std::io;

use yrs::{ReadTxn, StateVector, Transact};

use super::doc_id::DocId;
use super::equality::Fingerprint;
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
    while let Some(doc) = shared.next_compaction() {
        let _ = compact_queued(shared, &doc);
    }
}

pub(crate) fn compact_queued(shared: &Shared, doc: &DocId) -> Result<(), CompactError> {
    let result = compact(shared, doc);
    if let Err(e) = &result {
        eprintln!("aelomi storage: compacting {doc} failed: {e}");
    }
    let slot = shared.slot(doc);
    if let Some(state) = slot.state.lock().unwrap().as_mut() {
        state.compacting = false;
    }
    result
}

fn compact(shared: &Shared, doc: &DocId) -> Result<(), CompactError> {
    let slot = shared.slot(doc);
    let (generation, state, expected, corrupt) = {
        let mut guard = slot.state.lock().unwrap();
        let state = guard.as_mut().expect("compaction runs on loaded docs");
        let generation = state.log.segment.generation().next();
        state.log.segment = Segment::Next(generation, SegmentNo(0));
        state.log.unsnapshotted = LogSize::default();
        let txn = state.ydoc.transact();
        let bytes = txn.encode_state_as_update_v1(&StateVector::default());
        (
            generation,
            bytes,
            Fingerprint::of(&txn),
            state.corrupt.clone(),
        )
    };

    let (fs, layout) = (&*shared.fs, &shared.layout);
    let dir = layout.doc_dir(doc);
    let tmp = layout.doc_file(doc, DocFile::SnapshotTmp(generation));
    fs.create(&tmp)?;
    fs.append(&tmp, &encode_snapshot(&state))?;
    fs.fsync(&tmp)?;
    fs.full_sync(&tmp)?;

    let written = fs.read(&tmp)?;
    let read_back = read_snapshot(&written).and_then(snapshot_doc);
    if read_back.is_none_or(|d| Fingerprint::of(&d.transact()) != expected) {
        return Err(CompactError::Mismatch);
    }

    fs.rename(&tmp, &layout.doc_file(doc, DocFile::Snapshot(generation)))?;
    fs.fsync(&dir)?;
    fs.full_sync(&dir)?;

    let _io = slot.io.lock().unwrap();
    let moved = retire_older(shared, doc, generation, &corrupt)?;
    let mut state = slot.state.lock().unwrap();
    let state = state.as_mut().expect("still loaded");
    state.corrupt.retain(|name| !moved.contains(name));
    Ok(())
}

fn retire_older(
    shared: &Shared,
    doc: &DocId,
    generation: Generation,
    corrupt: &BTreeSet<String>,
) -> io::Result<BTreeSet<String>> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
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
    fs.fsync(&dir)?;
    fs.full_sync(&dir)?;
    Ok(moved)
}

fn quarantine(shared: &Shared, doc: &DocId, name: &str) -> io::Result<()> {
    let (fs, layout) = (&*shared.fs, &shared.layout);
    let target_dir = layout.quarantine_dir(doc);
    for (dir, parent) in [
        (layout.quarantine(), layout.root().to_path_buf()),
        (target_dir.clone(), layout.quarantine()),
    ] {
        match fs.create_dir(&dir) {
            Ok(()) => fs.fsync(&parent)?,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    let taken = fs.list(&target_dir)?;
    let target = std::iter::once(name.to_owned())
        .chain((1..).map(|n| format!("{name}.{n}")))
        .find(|candidate| !taken.contains(candidate))
        .expect("an unused name");
    fs.rename(&layout.doc_dir(doc).join(name), &target_dir.join(target))?;
    fs.fsync(&target_dir)
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
