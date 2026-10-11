use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use yrs::Doc;

use super::health::{FailReason, retry_delay};
use super::layout::{DocFile, Generation, SegmentNo};

/// An update's place in the order the store received updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seq(pub u64);

pub(crate) fn lock_ignoring_poison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
pub(crate) struct DocSlot {
    state: Mutex<Option<DocState>>,
    files: Mutex<()>,
}

impl DocSlot {
    pub fn state(&self) -> MutexGuard<'_, Option<DocState>> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| self.mark_stale_after_panic(poisoned))
    }

    fn mark_stale_after_panic<'a>(
        &'a self,
        poisoned: PoisonError<MutexGuard<'a, Option<DocState>>>,
    ) -> MutexGuard<'a, Option<DocState>> {
        let mut state = poisoned.into_inner();
        if let Some(state) = state.as_mut() {
            state.ydoc = Ydoc::Stale;
        }
        self.state.clear_poison();
        state
    }

    pub fn files(&self) -> MutexGuard<'_, ()> {
        lock_ignoring_poison(&self.files)
    }
}

pub(crate) struct DocState {
    pub ydoc: Ydoc,
    pub queue: VecDeque<Queued>,
    pub log: LogState,
    pub failure: Option<Failure>,
    pub corrupt: BTreeSet<String>,
    pub compaction: Compaction,
}

impl DocState {
    pub fn fail(&mut self, reason: FailReason, now: Duration) -> Duration {
        let attempts = self.failure.as_ref().map_or(1, |f| f.attempts + 1);
        let retry_at = now + retry_delay(attempts);
        self.failure = Some(Failure {
            reason,
            attempts,
            retry_at,
        });
        retry_at
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Compaction {
    Idle,
    Queued,
    /// A doc rebuilt while compaction runs can lack what the new snapshot holds, so it goes stale
    /// when compaction ends.
    Running {
        asked_after_rotate: bool,
        rebuilt: bool,
    },
}

#[derive(Clone)]
pub(crate) enum Ydoc {
    Fresh(Doc),
    Stale,
}

pub(crate) struct Failure {
    pub reason: FailReason,
    pub attempts: u32,
    pub retry_at: Duration,
}

pub(crate) struct Queued {
    pub seq: Seq,
    pub update: Arc<[u8]>,
}

pub(crate) struct LogState {
    pub dir: DocDir,
    pub segment: Segment,
    pub unsnapshotted: LogSize,
}

impl LogState {
    pub fn leave_open_segment(&mut self) {
        if let Segment::Open(..) = self.segment {
            self.segment = self.segment.abandon();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocDir {
    Missing,
    UnsyncedThisSession,
    Durable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Segment {
    Next(Generation, SegmentNo),
    Open(Generation, SegmentNo),
}

impl Segment {
    pub fn file(self) -> DocFile {
        match self {
            Self::Next(g, n) | Self::Open(g, n) => DocFile::Log(g, n),
        }
    }

    pub fn generation(self) -> Generation {
        match self {
            Self::Next(g, _) | Self::Open(g, _) => g,
        }
    }

    pub fn opened(self) -> Self {
        match self {
            Self::Next(g, n) | Self::Open(g, n) => Self::Open(g, n),
        }
    }

    pub fn abandon(self) -> Self {
        match self {
            Self::Next(g, n) | Self::Open(g, n) => Self::Next(g, n.next()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LogSize {
    pub bytes: u64,
    pub records: u64,
}

impl LogSize {
    pub fn exceeds(self, limits: Limits) -> bool {
        self.bytes > limits.bytes || self.records > limits.records
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    pub bytes: u64,
    pub records: u64,
    pub max_payload: u32,
}

impl Limits {
    pub const DATA_6: Self = Self {
        bytes: 1_000_000,
        records: 10_000,
        max_payload: u32::MAX,
    };
}

impl std::ops::AddAssign for LogSize {
    fn add_assign(&mut self, other: Self) {
        self.bytes += other.bytes;
        self.records += other.records;
    }
}
