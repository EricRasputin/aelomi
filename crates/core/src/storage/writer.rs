//! The writer: one thread owns every log write. Each batch appends each doc's queued
//! updates to its segment with one write per file, plain-`fsync`s every touched file and
//! every directory that got a new entry, then makes the whole batch durable with one
//! `full_sync`. Only then are the batch's updates acknowledged.

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, MutexGuard};

use super::doc_id::DocId;
use super::health::FailReason;
use super::log::{LOG_MAGIC, encode_record};
use super::state::{DocDir, DocSlot, LogSize, Segment, Seq};
use super::store::Shared;

#[derive(Debug)]
pub(crate) enum Step {
    Wrote(Batch),
    /// Nothing was queued.
    Idle,
}

#[derive(Debug, Default)]
pub(crate) struct Batch {
    /// Each doc that became durable, through the `Seq` it became durable through.
    pub durable: Vec<(DocId, Seq)>,
    pub failed: Vec<(DocId, FailReason)>,
}

pub(crate) fn run(shared: &Shared) {
    while shared.wait_for_work(None) {
        while let Step::Wrote(_) = run_once(shared) {}
    }
}

pub(crate) fn run_once(shared: &Shared) -> Step {
    let mut jobs = collect(shared);
    if jobs.is_empty() {
        return Step::Idle;
    }
    let slots: Vec<Arc<DocSlot>> = jobs.iter().map(|job| job.slot.clone()).collect();
    let _io: Vec<MutexGuard<'_, ()>> = slots.iter().map(|s| s.io.lock().unwrap()).collect();

    for job in &mut jobs {
        let result = job.write(shared);
        job.fail_on(result);
    }
    for job in jobs.iter_mut().filter(|job| job.error.is_none()) {
        let result = job.sync(shared);
        job.fail_on(result);
    }
    let first_ok = jobs.iter().find(|job| job.error.is_none());
    if let Some(path) = first_ok.map(|job| job.path(shared))
        && let Err(e) = shared.fs.full_sync(&path)
    {
        for job in jobs.iter_mut().filter(|job| job.error.is_none()) {
            job.error = Some(FailReason::from(&e));
        }
    }

    let batch = commit(jobs);
    shared.batch_finished();
    Step::Wrote(batch)
}

/// One doc's part of a batch: every queued update, encoded as log records.
struct Job {
    doc: DocId,
    slot: Arc<DocSlot>,
    records: Vec<u8>,
    count: u64,
    through: Seq,
    segment: Segment,
    dir: DocDir,
    created_segment: bool,
    error: Option<FailReason>,
}

fn collect(shared: &Shared) -> Vec<Job> {
    let mut jobs = Vec::new();
    for (doc, slot) in shared.slots() {
        let state = slot.state.lock().unwrap();
        let Some(state) = state.as_ref() else {
            continue;
        };
        let Some(last) = state.queue.back() else {
            continue;
        };
        let mut records = Vec::new();
        for queued in &state.queue {
            encode_record(&queued.update, &mut records);
        }
        jobs.push(Job {
            doc,
            slot: slot.clone(),
            records,
            count: state.queue.len() as u64,
            through: last.seq,
            segment: state.log.segment,
            dir: state.log.dir,
            created_segment: false,
            error: None,
        });
    }
    jobs
}

impl Job {
    fn path(&self, shared: &Shared) -> PathBuf {
        shared.layout.doc_file(&self.doc, self.segment.file())
    }

    fn fail_on(&mut self, result: io::Result<()>) {
        if let Err(e) = result {
            self.error = Some(FailReason::from(&e));
        }
    }

    fn write(&mut self, shared: &Shared) -> io::Result<()> {
        let fs = &*shared.fs;
        if self.dir == DocDir::Missing {
            match fs.create_dir(&shared.layout.doc_dir(&self.doc)) {
                Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
                _ => self.dir = DocDir::Created,
            }
        }
        let path = self.path(shared);
        match self.segment {
            Segment::Open(..) => fs.append(&path, &self.records),
            Segment::Next(..) => {
                fs.create(&path)?;
                self.created_segment = true;
                fs.append(&path, &[&LOG_MAGIC[..], &self.records].concat())
            }
        }
    }

    fn sync(&self, shared: &Shared) -> io::Result<()> {
        let fs = &*shared.fs;
        fs.fsync(&self.path(shared))?;
        if self.created_segment {
            fs.fsync(&shared.layout.doc_dir(&self.doc))?;
        }
        if self.dir == DocDir::Created {
            fs.fsync(&shared.layout.docs())?;
        }
        Ok(())
    }

    fn size(&self) -> LogSize {
        let magic = if self.created_segment {
            LOG_MAGIC.len()
        } else {
            0
        };
        LogSize {
            bytes: (self.records.len() + magic) as u64,
            records: self.count,
        }
    }
}

fn commit(jobs: Vec<Job>) -> Batch {
    let mut batch = Batch::default();
    for job in jobs {
        let mut state = job.slot.state.lock().unwrap();
        let state = state.as_mut().expect("a doc with queued updates is loaded");
        let current = state.log.segment == job.segment;
        match job.error {
            None => {
                while state.queue.front().is_some_and(|q| q.seq <= job.through) {
                    state.queue.pop_front();
                }
                state.log.dir = DocDir::Durable;
                if current {
                    state.log.segment = job.segment.opened();
                }
                if job.segment.generation() == state.log.segment.generation() {
                    state.log.unsnapshotted += job.size();
                }
                batch.durable.push((job.doc, job.through));
            }
            Some(reason) => {
                if state.log.dir == DocDir::Missing {
                    state.log.dir = job.dir;
                }
                if current {
                    state.log.segment = job.segment.abandon();
                }
                batch.failed.push((job.doc, reason));
            }
        }
    }
    batch
}
