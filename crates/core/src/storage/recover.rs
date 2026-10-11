use std::collections::BTreeSet;
use std::io;

use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

use super::doc_id::DocId;
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
    let names = match fs.list(&layout.doc_dir(doc)) {
        Ok(names) => names,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let log = LogState {
                dir: DocDir::Missing,
                segment: Segment::Next(Generation(0), SegmentNo(0)),
                unsnapshotted: LogSize::default(),
            };
            let corrupt = BTreeSet::new();
            return Ok(Recovered {
                ydoc: Doc::new(),
                log,
                corrupt,
                needs_compaction: false,
            });
        }
        Err(e) => return Err(e),
    };
    let mut files: Vec<DocFile> = names.iter().filter_map(|n| DocFile::parse(n)).collect();
    files.sort();
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

/// Applies every record it can, and returns whether all of them applied.
fn apply_records(ydoc: &Doc, records: &[&[u8]]) -> bool {
    let mut txn = ydoc.transact_mut();
    records.iter().fold(true, |applied, record| {
        let update = Update::decode_v1(record);
        applied & update.is_ok_and(|u| txn.apply_update(u).is_ok())
    })
}

pub(crate) fn snapshot_doc(payload: &[u8]) -> Option<Doc> {
    let update = Update::decode_v1(payload).ok()?;
    let doc = Doc::new();
    doc.transact_mut().apply_update(update).ok()?;
    Some(doc)
}
