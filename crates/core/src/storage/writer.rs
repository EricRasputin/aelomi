use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::{Arc, MutexGuard};
use std::time::Duration;

use super::doc_id::DocId;
use super::format::Format2Ready;
use super::health::FailReason;
use super::layout::{Generation, SegmentNo};
use super::log::encode_frame;
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
    if let Some(at) = shared.settings.failed(FailReason::Internal, now) {
        retry_at = Some(retry_at.map_or(at, |r| r.min(at)));
    }
    shared.batch_finished();
    shared.refresh_health(Some(FailReason::Internal));
    retry_at
}

fn write_queued(shared: &Shared) -> Step {
    let retry_failing = shared.take_retry_failing();
    let settings = super::settings::write_queued(shared, retry_failing);
    let slots = shared.slots();
    let (queued, retry_at) = collect(&slots, shared.clock.now(), retry_failing);
    if queued.is_empty() && matches!(settings, Step::Idle { .. }) {
        let settings_retry = match settings {
            Step::Idle { retry_at } => retry_at,
            Step::Wrote => None,
        };
        return Step::Idle {
            retry_at: retry_at.into_iter().chain(settings_retry).min(),
        };
    }
    let latest_failure = write(shared, queued);
    shared.refresh_health(latest_failure);
    shared.batch_finished();
    Step::Wrote
}

struct Queued<'a> {
    doc: &'a DocId,
    slot: &'a DocSlot,
    _files: MutexGuard<'a, ()>,
    updates: Vec<Arc<[u8]>>,
    through: Seq,
}

fn collect(
    slots: &[(DocId, Arc<DocSlot>)],
    now: Duration,
    retry_failing: bool,
) -> (Vec<Queued<'_>>, Option<Duration>) {
    let mut queued = Vec::new();
    let mut retry_at: Option<Duration> = None;
    for (doc, slot) in slots {
        let mut state = slot.state();
        let Some(state) = state.as_mut() else {
            continue;
        };
        let Some(through) = state.queue.back().map(|last| last.seq) else {
            continue;
        };
        if !retry_failing
            && let Some(failure) = &state.failure
            && failure.retry_at > now
        {
            retry_at = Some(retry_at.map_or(failure.retry_at, |at| at.min(failure.retry_at)));
            continue;
        }
        let Some(files) = slot.try_files() else {
            if let Some(failure) = &mut state.failure {
                failure.retry_no_later_than(now);
            }
            continue;
        };
        queued.push(Queued {
            doc,
            slot,
            _files: files,
            updates: state.queue.iter().map(|q| q.update.clone()).collect(),
            through,
        });
    }
    (queued, retry_at)
}

fn write(shared: &Shared, queued: Vec<Queued<'_>>) -> Option<FailReason> {
    let mut jobs: Vec<Job> = queued.into_iter().map(Job::new).collect();

    let mut gate = None;
    for job in &mut jobs {
        let result = match job.segment {
            Segment::Open { .. } => job.append(shared),
            Segment::Next(generation, n) => {
                let ready = gate
                    .get_or_insert_with(|| shared.format.ensure_v2(&*shared.fs, &shared.layout));
                match ready {
                    Ok(ready) => job.start_segment(shared, ready, generation, n),
                    Err(e) => {
                        job.error = Some(FailReason::from(&*e));
                        continue;
                    }
                }
            }
        };
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

struct Job<'a> {
    queued: Queued<'a>,
    segment: Segment,
    acknowledged: u64,
    dir: DocDir,
    created_segment: bool,
    written: u64,
    error: Option<FailReason>,
}

impl<'a> Job<'a> {
    fn new(queued: Queued<'a>) -> Self {
        let state = queued.slot.state();
        let log = &state
            .as_ref()
            .expect("a doc with queued updates is loaded")
            .log;
        let (segment, dir) = (log.segment, log.dir);
        drop(state);
        let acknowledged = match segment {
            Segment::Open { written, .. } => written,
            Segment::Next(..) => 0,
        };
        Self {
            queued,
            segment,
            acknowledged,
            dir,
            created_segment: false,
            written: 0,
            error: None,
        }
    }

    fn path(&self, shared: &Shared) -> PathBuf {
        shared.layout.doc_file(self.queued.doc, self.segment.file())
    }

    fn fail_on(&mut self, result: io::Result<()>) {
        if let Err(e) = result {
            self.error = Some(FailReason::from(&e));
        }
    }

    fn start_segment(
        &mut self,
        shared: &Shared,
        ready: &Format2Ready,
        generation: Generation,
        n: SegmentNo,
    ) -> io::Result<()> {
        let fs = &*shared.fs;
        if self.dir == DocDir::Missing {
            match fs.create_dir(&shared.layout.doc_dir(self.queued.doc)) {
                Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
                _ => self.dir = DocDir::UnsyncedThisSession,
            }
        }
        ready.create_new(fs, &self.path(shared))?;
        self.segment = Segment::Open {
            generation,
            n,
            written: 0,
        };
        self.created_segment = true;
        self.append(shared)
    }

    fn append(&mut self, shared: &Shared) -> io::Result<()> {
        let mut bytes = Vec::new();
        for update in &self.queued.updates {
            encode_frame(update, &mut bytes);
        }
        self.written = bytes.len() as u64;
        shared.fs.append(&self.path(shared), &bytes)?;
        if let Segment::Open { written, .. } = &mut self.segment {
            *written += self.written;
        }
        Ok(())
    }

    fn sync(&self, shared: &Shared) -> io::Result<()> {
        shared.fs.fsync(&self.path(shared))?;
        if self.created_segment {
            shared.sync_quarantine_then_doc_dir(self.queued.doc)?;
        }
        Ok(())
    }

    fn size(&self) -> LogSize {
        LogSize {
            bytes: self.written,
            records: self.queued.updates.len() as u64,
        }
    }
}

fn commit(shared: &Shared, jobs: Vec<Job>) -> Option<FailReason> {
    let now = shared.clock.now();
    let mut latest_failure = None;
    for job in jobs {
        let mut state = job.queued.slot.state();
        let state = state.as_mut().expect("a doc with queued updates is loaded");
        let current = state.log.segment.file() == job.segment.file();
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
                    state.log.segment = job.segment;
                }
                if job.segment.generation() == state.log.segment.generation() {
                    state.log.unsnapshotted += job.size();
                }
                state.failure = None;
                if state.log.unsnapshotted.exceeds(shared.limits) {
                    shared.queue_compaction(job.queued.doc, state);
                }
            }
            Some(reason) => {
                if let Segment::Open { .. } = job.segment {
                    let file = job.segment.file().name();
                    state
                        .acknowledged_before_failure
                        .insert(file, job.acknowledged);
                }
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
