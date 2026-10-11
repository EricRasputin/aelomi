use std::collections::BTreeSet;
use std::io;

use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, Transact, Update};

use super::doc_id::DocId;
use super::equality::Ops;
use super::fs::Fs;
use super::layout::{DocFile, Generation, Layout, SegmentNo};
use super::log::{LogEnd, read_log};
use super::snapshot::read_snapshot;
use super::state::{DocDir, Limits, LogSize, LogState, Segment};

pub(crate) struct Recovered {
    pub ydoc: Doc,
    pub log: LogState,
    pub corrupt: BTreeSet<String>,
    pub needs_compaction: bool,
}

pub(crate) fn recover(
    fs: &dyn Fs,
    layout: &Layout,
    doc: &DocId,
    limits: Limits,
) -> io::Result<Recovered> {
    let Some(files) = doc_files(fs, layout, doc)? else {
        let log = LogState {
            dir: DocDir::Missing,
            segment: Segment::Next(Generation(0), SegmentNo(0)),
            unsnapshotted: LogSize::default(),
        };
        return Ok(Recovered {
            ydoc: Doc::new(),
            log,
            corrupt: BTreeSet::new(),
            needs_compaction: false,
        });
    };
    let mut corrupt = BTreeSet::new();

    let ydoc = Doc::new();
    let mut base = None;
    for file in files.iter().rev() {
        let DocFile::Snapshot(generation) = *file else {
            continue;
        };
        let bytes = fs.read(&layout.doc_file(doc, *file))?;
        if read_snapshot(&bytes).is_some_and(|payload| apply_records(&ydoc, &[payload])) {
            base.get_or_insert(generation);
        } else {
            eprintln!("aelomi storage: {doc}/{} is corrupt", file.name());
            corrupt.insert(file.name());
        }
    }

    let mut unsnapshotted = LogSize::default();
    for file in &files {
        let DocFile::Log(generation, _) = *file else {
            continue;
        };
        let bytes = fs.read(&layout.doc_file(doc, *file))?;
        let contents = read_log(&bytes);
        let applied = apply_records(&ydoc, &contents.records);
        if contents.end == LogEnd::Corrupt || !applied {
            eprintln!("aelomi storage: {doc}/{} is corrupt", file.name());
            corrupt.insert(file.name());
        }
        if Some(generation) >= base {
            let records = contents.records.len() as u64;
            unsnapshotted += LogSize {
                bytes: bytes.len() as u64,
                records,
            };
        }
    }

    let newest = files
        .iter()
        .map(|f| f.generation())
        .max()
        .unwrap_or(Generation(0));
    let next = files
        .iter()
        .filter_map(|f| match f {
            DocFile::Log(g, n) if *g == newest => Some(n.next()),
            _ => None,
        })
        .max()
        .unwrap_or(SegmentNo(0));
    let interrupted = files
        .iter()
        .any(|f| matches!(f, DocFile::SnapshotTmp(_)) || base.is_some_and(|b| f.generation() < b));
    let log = LogState {
        dir: DocDir::UnsyncedThisSession,
        segment: Segment::Next(newest, next),
        unsnapshotted,
    };
    let needs_compaction = interrupted || unsnapshotted.exceeds(limits);
    Ok(Recovered {
        ydoc,
        log,
        corrupt,
        needs_compaction,
    })
}

/// Every update recovery applies, in its order: each valid snapshot, newest first, then each
/// log's records before any damage, oldest first.
pub(crate) fn stored_updates(
    fs: &dyn Fs,
    layout: &Layout,
    doc: &DocId,
) -> io::Result<Vec<Vec<u8>>> {
    let files = doc_files(fs, layout, doc)?.unwrap_or_default();
    let mut updates = Vec::new();
    for file in files.iter().rev() {
        if let DocFile::Snapshot(_) = file
            && let Some(payload) = read_snapshot(&fs.read(&layout.doc_file(doc, *file))?)
        {
            updates.push(payload.to_vec());
        }
    }
    for file in &files {
        if let DocFile::Log(..) = file {
            let bytes = fs.read(&layout.doc_file(doc, *file))?;
            updates.extend(read_log(&bytes).records.into_iter().map(<[u8]>::to_vec));
        }
    }
    Ok(updates)
}

/// Merges `updates` into one that holds exactly their operations. Bytes that don't decode are
/// left out, as recovery can't apply them either.
pub(crate) fn merge<'a>(updates: impl IntoIterator<Item = &'a [u8]>) -> Merged {
    let mut ops = Ops::default();
    let mut level: Vec<Update> = updates
        .into_iter()
        .filter_map(|bytes| Update::decode_v1(bytes).ok())
        .inspect(|update| ops.merge(Ops::of_update(update)))
        .collect();
    while level.len() > 1 {
        let mut updates = level.into_iter().peekable();
        level = std::iter::from_fn(|| {
            updates.peek()?;
            Some(Update::merge_updates(updates.by_ref().take(MERGE_FAN_IN)))
        })
        .collect();
    }
    Merged {
        payload: level.pop().unwrap_or_default().encode_v1(),
        ops,
    }
}

/// yrs re-sorts every input at each step of a merge, so 10,000 records take about 250 ms in one
/// merge and about 10 ms merged 32 at a time.
const MERGE_FAN_IN: usize = 32;

pub(crate) struct Merged {
    pub payload: Vec<u8>,
    pub ops: Ops,
}

/// The doc's files, sorted, or `None` if it has no folder.
fn doc_files(fs: &dyn Fs, layout: &Layout, doc: &DocId) -> io::Result<Option<Vec<DocFile>>> {
    let names = match fs.list(&layout.doc_dir(doc)) {
        Ok(names) => names,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut files: Vec<DocFile> = names.iter().filter_map(|n| DocFile::parse(n)).collect();
    files.sort();
    Ok(Some(files))
}

fn apply_records(ydoc: &Doc, records: &[&[u8]]) -> bool {
    let mut txn = ydoc.transact_mut();
    let mut applied = true;
    for record in records {
        let update = Update::decode_v1(record);
        applied &= update.is_ok_and(|u| txn.apply_update(u).is_ok());
    }
    applied
}
