//! The writer: one thread owns every log write. Each batch appends each doc's queued
//! updates to its segment with one write per file, plain-`fsync`s every touched file and
//! every directory that got a new entry, then makes the whole batch durable with one
//! `full_sync`. Only then are the batch's updates acknowledged.

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, MutexGuard};
use std::time::Duration;

use super::doc_id::DocId;
use super::health::{FailReason, retry_delay};
use super::log::{LOG_MAGIC, encode_record};
use super::state::{DocDir, DocSlot, Failure, LogSize, Segment, Seq};
use super::store::Shared;

#[derive(Debug)]
pub(crate) enum Step {
    Wrote(Batch),
    /// Nothing is queued, except for failed docs waiting to retry. The earliest retry is due
    /// at `retry_at` on the store's clock.
    Idle {
        retry_at: Option<Duration>,
    },
}

#[derive(Debug, Default)]
pub(crate) struct Batch {
    /// Each doc that became durable, through the `Seq` it became durable through.
    pub durable: Vec<(DocId, Seq)>,
    pub failed: Vec<(DocId, FailReason)>,
}

pub(crate) fn run(shared: &Shared) {
    let mut retry_at = None;
    while shared.wait_for_work(retry_at) {
        retry_at = loop {
            match run_once(shared) {
                Step::Wrote(_) if shared.is_shutting_down() => return,
                Step::Wrote(_) => {}
                Step::Idle { retry_at } => break retry_at,
            }
        };
    }
}

pub(crate) fn run_once(shared: &Shared) -> Step {
    let now = shared.clock.now();
    let (mut jobs, retry_at) = collect(shared, now);
    if jobs.is_empty() {
        return Step::Idle { retry_at };
    }
    let slots: Vec<Arc<DocSlot>> = jobs.iter().map(|job| job.slot.clone()).collect();
    let _io: Vec<MutexGuard<'_, ()>> = slots.iter().map(|s| s.io.lock().unwrap()).collect();
    // A compaction may have rotated a segment since `collect`, and may have deleted the old
    // one. It can't delete anything while the io locks are held, so write where it points now.
    for job in &mut jobs {
        let state = job.slot.state.lock().unwrap();
        let log = &state
            .as_ref()
            .expect("a doc with queued updates is loaded")
            .log;
        (job.segment, job.dir) = (log.segment, log.dir);
    }

    for job in &mut jobs {
        let result = job.write(shared);
        job.fail_on(result);
    }
    for job in jobs.iter_mut().filter(|job| job.error.is_none()) {
        let result = job.sync(shared);
        job.fail_on(result);
    }
    let new_doc_dir = |job: &Job| job.error.is_none() && job.dir == DocDir::Created;
    if jobs.iter().any(new_doc_dir)
        && let Err(e) = shared.fs.fsync(&shared.layout.docs())
    {
        for job in jobs.iter_mut().filter(|job| new_doc_dir(job)) {
            job.error = Some(FailReason::from(&e));
        }
    }
    let first_ok = jobs.iter().find(|job| job.error.is_none());
    if let Some(path) = first_ok.map(|job| job.path(shared))
        && let Err(e) = shared.fs.full_sync(&path)
    {
        for job in jobs.iter_mut().filter(|job| job.error.is_none()) {
            job.error = Some(FailReason::from(&e));
        }
    }

    let batch = commit(shared, jobs);
    shared.refresh_health(batch.failed.last().map(|(_, reason)| reason.clone()));
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

/// Takes every doc with queued updates, except failed docs whose retry isn't due yet.
fn collect(shared: &Shared, now: Duration) -> (Vec<Job>, Option<Duration>) {
    let mut jobs = Vec::new();
    let mut retry_at: Option<Duration> = None;
    for (doc, slot) in shared.slots() {
        let state = slot.state.lock().unwrap();
        let Some(state) = state.as_ref() else {
            continue;
        };
        let Some(last) = state.queue.back() else {
            continue;
        };
        if let Some(failure) = &state.failure
            && failure.retry_at > now
        {
            retry_at = Some(retry_at.map_or(failure.retry_at, |at| at.min(failure.retry_at)));
            continue;
        }
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
    (jobs, retry_at)
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

    /// Fsyncs the segment, and the doc's directory if the segment is new. A new doc
    /// directory's entry in `docs/` is fsynced once for the whole batch.
    fn sync(&self, shared: &Shared) -> io::Result<()> {
        let fs = &*shared.fs;
        fs.fsync(&self.path(shared))?;
        if self.created_segment {
            fs.fsync(&shared.layout.doc_dir(&self.doc))?;
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

fn commit(shared: &Shared, jobs: Vec<Job>) -> Batch {
    let now = shared.clock.now();
    let mut batch = Batch::default();
    let mut to_compact = Vec::new();
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
                state.failure = None;
                if state.log.unsnapshotted.exceeds(shared.limits) && !state.compacting {
                    state.compacting = true;
                    to_compact.push(job.doc.clone());
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
                let attempts = state.failure.as_ref().map_or(1, |f| f.attempts + 1);
                let retry_at = now + retry_delay(attempts);
                state.failure = Some(Failure {
                    reason: reason.clone(),
                    attempts,
                    retry_at,
                });
                batch.failed.push((job.doc, reason));
            }
        }
    }
    for doc in to_compact {
        shared.schedule_compaction(doc);
    }
    batch
}
