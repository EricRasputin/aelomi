//! Loading a doc from its files on first use.

use std::io;

use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

use super::doc_id::DocId;
use super::fs::Fs;
use super::layout::{DocFile, Generation, Layout, SegmentNo};
use super::log::{LogEnd, read_log};
use super::snapshot::read_snapshot;
use super::state::{DocDir, LogSize, LogState, Segment};

pub(crate) struct Recovered {
    pub ydoc: Doc,
    pub log: LogState,
}

/// Builds the doc from the newest valid snapshot and every log. Yjs updates are idempotent,
/// so applying a log the snapshot already covers is harmless: when unsure, apply more.
pub(crate) fn recover(fs: &dyn Fs, layout: &Layout, doc: &DocId) -> io::Result<Recovered> {
    let dir = layout.doc_dir(doc);
    let names = match fs.list(&dir) {
        Ok(names) => names,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let segment = Segment::Next(Generation(0), SegmentNo(0));
            let log = LogState {
                dir: DocDir::Missing,
                segment,
                unsnapshotted: LogSize::default(),
            };
            return Ok(Recovered {
                ydoc: Doc::new(),
                log,
            });
        }
        Err(e) => return Err(e),
    };
    let mut files: Vec<DocFile> = names.iter().filter_map(|n| DocFile::parse(n)).collect();
    files.sort();
    let newest = files
        .iter()
        .map(|f| f.generation())
        .max()
        .unwrap_or(Generation(0));

    let ydoc = Doc::new();
    let mut base = None;
    for file in files.iter().rev() {
        let DocFile::Snapshot(generation) = *file else {
            continue;
        };
        let bytes = fs.read(&layout.doc_file(doc, *file))?;
        let update = read_snapshot(&bytes).and_then(|p| Update::decode_v1(p).ok());
        if let Some(update) = update
            && ydoc.transact_mut().apply_update(update).is_ok()
        {
            base = Some(generation);
            break;
        }
    }

    let mut unsnapshotted = LogSize::default();
    let mut next = SegmentNo(0);
    for file in &files {
        let DocFile::Log(generation, number) = *file else {
            continue;
        };
        let bytes = fs.read(&layout.doc_file(doc, *file))?;
        let contents = read_log(&bytes);
        let mut txn = ydoc.transact_mut();
        for record in &contents.records {
            if let Ok(update) = Update::decode_v1(record) {
                let _ = txn.apply_update(update);
            }
        }
        if contents.end == LogEnd::Corrupt {
            eprintln!(
                "aelomi storage: {doc}/{} is corrupt after record {}",
                file.name(),
                contents.records.len()
            );
        }
        if Some(generation) >= base {
            unsnapshotted += LogSize {
                bytes: bytes.len() as u64,
                records: contents.records.len() as u64,
            };
        }
        if generation == newest {
            next = number.next();
        }
    }

    let segment = Segment::Next(newest, next);
    let log = LogState {
        dir: DocDir::Durable,
        segment,
        unsnapshotted,
    };
    Ok(Recovered { ydoc, log })
}
