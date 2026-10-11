use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use yrs::Doc;

use super::health::FailReason;
use super::layout::{DocFile, Generation, SegmentNo};

/// An update's place in the order the store received updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seq(pub u64);

#[derive(Default)]
pub(crate) struct DocSlot {
    pub state: Mutex<Option<DocState>>,
    pub files: Mutex<()>,
}

pub(crate) struct DocState {
    pub ydoc: Ydoc,
    pub queue: VecDeque<Queued>,
    pub log: LogState,
    pub failure: Option<Failure>,
    pub corrupt: BTreeSet<String>,
    pub compaction: Compaction,
}

/// A request made while `Queued` is covered by the coming rotate. One made while `Running`
/// came after the rotate took the doc's state, so it sets `again`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Compaction {
    Idle,
    Queued,
    Running { again: bool },
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
}

impl Limits {
    pub const DATA_6: Self = Self {
        bytes: 1_000_000,
        records: 10_000,
    };
}

impl std::ops::AddAssign for LogSize {
    fn add_assign(&mut self, other: Self) {
        self.bytes += other.bytes;
        self.records += other.records;
    }
}
