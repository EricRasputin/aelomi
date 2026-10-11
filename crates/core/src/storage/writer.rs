use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::{Arc, MutexGuard};
use std::time::Duration;

use super::doc_id::DocId;
use super::health::FailReason;
use super::log::{LOG_MAGIC, encode_record};
use super::state::{DocDir, DocSlot, LogSize, Segment, Seq};
use super::store::Shared;

#[derive(Debug)]
pub(crate) enum Step {
    Wrote,
    Idle { retry_at: Option<Duration> },
}

pub(crate) fn run(shared: &Shared) {
    let mut retry_at = None;
    while shared.wait_for_work(retry_at) {
        retry_at = loop {
            match run_once(shared) {
                Step::Wrote if shared.is_shutting_down() => return,
                Step::Wrote => {}
                Step::Idle { retry_at } => break retry_at,
            }
        };
    }
}

pub(crate) fn run_once(shared: &Shared) -> Step {
    panic::catch_unwind(AssertUnwindSafe(|| write_queued(shared))).unwrap_or_else(|_| Step::Idle {
        retry_at: fail_every_queued_doc(shared),
    })
}

/// The panic may have left any doc's write partway, so each retries in a new segment.
fn fail_every_queued_doc(shared: &Shared) -> Option<Duration> {
    let now = shared.clock.now();
    let mut retry_at: Option<Duration> = None;
    for (_, slot) in shared.slots() {
        let mut state = slot.state();
        let Some(state) = state.as_mut().filter(|s| !s.queue.is_empty()) else {
            continue;
        };
        state.log.segment = state.log.segment.abandon();
        let at = state.fail(FailReason::Internal, now);
        retry_at = Some(retry_at.map_or(at, |r| r.min(at)));
    }
    shared.refresh_health(Some(FailReason::Internal));
    retry_at
}

fn write_queued(shared: &Shared) -> Step {
    let retry_failing = shared.take_retry_failing();
    let (queued, retry_at) = collect(shared, shared.clock.now(), retry_failing);
    if queued.is_empty() {
        return Step::Idle { retry_at };
    }
    let latest_failure = write(shared, queued);
    shared.refresh_health(latest_failure);
    shared.batch_finished();
    Step::Wrote
}

struct Queued {
    doc: DocId,
    slot: Arc<DocSlot>,
    records: Vec<u8>,
    count: u64,
    through: Seq,
}

fn collect(shared: &Shared, now: Duration, retry_failing: bool) -> (Vec<Queued>, Option<Duration>) {
    let mut queued = Vec::new();
    let mut retry_at: Option<Duration> = None;
    for (doc, slot) in shared.slots() {
        let state = slot.state();
        let Some(state) = state.as_ref() else {
            continue;
        };
        let Some(last) = state.queue.back() else {
            continue;
        };
        if !retry_failing
            && let Some(failure) = &state.failure
            && failure.retry_at > now
        {
            retry_at = Some(retry_at.map_or(failure.retry_at, |at| at.min(failure.retry_at)));
            continue;
        }
        let mut records = Vec::new();
        for update in &state.queue {
            encode_record(&update.update, &mut records);
        }
        queued.push(Queued {
            doc,
            slot: slot.clone(),
            records,
            count: state.queue.len() as u64,
            through: last.seq,
        });
    }
    (queued, retry_at)
}

fn write(shared: &Shared, queued: Vec<Queued>) -> Option<FailReason> {
    let slots: Vec<Arc<DocSlot>> = queued.iter().map(|q| q.slot.clone()).collect();
    let _files: Vec<MutexGuard<'_, ()>> = slots.iter().map(|s| s.files()).collect();
    let mut jobs: Vec<Job> = queued.into_iter().map(Job::new).collect();

    for job in &mut jobs {
        let result = job.write(shared);
        job.fail_on(result);
    }
    for job in jobs.iter_mut().filter(|job| job.error.is_none()) {
        let result = job.sync(shared);
        job.fail_on(result);
    }
    let new_doc_dir = |job: &Job| job.error.is_none() && job.dir == DocDir::UnsyncedThisSession;
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
    commit(shared, jobs)
}

struct Job {
    queued: Queued,
    segment: Segment,
    dir: DocDir,
    created_segment: bool,
    error: Option<FailReason>,
}

impl Job {
    /// Takes the doc's segment once its file lock is held, so compaction can't rotate it and
    /// retire the old file between this read and the write.
    fn new(queued: Queued) -> Self {
        let state = queued.slot.state();
        let log = &state
            .as_ref()
            .expect("a doc with queued updates is loaded")
            .log;
        let (segment, dir) = (log.segment, log.dir);
        drop(state);
        Self {
            queued,
            segment,
            dir,
            created_segment: false,
            error: None,
        }
    }

    fn path(&self, shared: &Shared) -> PathBuf {
        shared
            .layout
            .doc_file(&self.queued.doc, self.segment.file())
    }

    fn fail_on(&mut self, result: io::Result<()>) {
        if let Err(e) = result {
            self.error = Some(FailReason::from(&e));
        }
    }

    fn write(&mut self, shared: &Shared) -> io::Result<()> {
        let fs = &*shared.fs;
        if self.dir == DocDir::Missing {
            match fs.create_dir(&shared.layout.doc_dir(&self.queued.doc)) {
                Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
                _ => self.dir = DocDir::UnsyncedThisSession,
            }
        }
        let path = self.path(shared);
        match self.segment {
            Segment::Open(..) => fs.append(&path, &self.queued.records),
            Segment::Next(..) => {
                fs.create_new(&path)?;
                self.created_segment = true;
                fs.append(&path, &[&LOG_MAGIC[..], &self.queued.records].concat())
            }
        }
    }

    fn sync(&self, shared: &Shared) -> io::Result<()> {
        shared.fs.fsync(&self.path(shared))?;
        if self.created_segment {
            shared.sync_quarantine_then_doc_dir(&self.queued.doc)?;
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
            bytes: (self.queued.records.len() + magic) as u64,
            records: self.queued.count,
        }
    }
}

fn commit(shared: &Shared, jobs: Vec<Job>) -> Option<FailReason> {
    let now = shared.clock.now();
    let mut latest_failure = None;
    for job in jobs {
        let mut state = job.queued.slot.state();
        let state = state.as_mut().expect("a doc with queued updates is loaded");
        let current = state.log.segment == job.segment;
        match job.error {
            None => {
                while state
                    .queue
                    .front()
                    .is_some_and(|q| q.seq <= job.queued.through)
                {
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
                if state.log.unsnapshotted.exceeds(shared.limits) {
                    shared.queue_compaction(&job.queued.doc, state);
                }
            }
            Some(reason) => {
                if state.log.dir == DocDir::Missing {
                    state.log.dir = job.dir;
                }
                if current {
                    state.log.segment = job.segment.abandon();
                }
                state.fail(reason.clone(), now);
                latest_failure = Some(reason);
            }
        }
    }
    latest_failure
}
