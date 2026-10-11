use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use yrs::Doc;

use super::health::FailReason;
use super::layout::{DocFile, Generation, SegmentNo};

/// An update's place in the order the store received updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seq(pub u64);

/// A doc's entry in the store. `state` is `None` until the doc's first load.
#[derive(Default)]
pub(crate) struct DocSlot {
    pub state: Mutex<Option<DocState>>,
    /// Held by the writer while it writes and syncs this doc's batch, and by compaction while
    /// it deletes old files, so compaction never deletes a segment the writer is writing.
    pub io: Mutex<()>,
}

/// A loaded doc: its full state in memory, and the updates not yet durable.
pub(crate) struct DocState {
    /// Every update appended or loaded, durable or not.
    pub ydoc: Doc,
    /// Appended updates that aren't durable yet, in `Seq` order. An update leaves the queue
    /// only once a full sync covers it, so a failed batch is retried from here.
    pub queue: VecDeque<Queued>,
    pub log: LogState,
    /// Set while the doc's last batch failed. Its queue waits until `retry_at`.
    pub failure: Option<Failure>,
    /// Names of files that failed validation in a way a torn write can't explain.
    /// Compaction moves them to `quarantine/` instead of deleting them.
    pub corrupt: BTreeSet<String>,
    /// Queued for compaction, or being compacted.
    pub compacting: bool,
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

/// The writer's view of a doc's files.
pub(crate) struct LogState {
    pub dir: DocDir,
    pub segment: Segment,
    /// Log files that no snapshot covers yet. Compaction starts when they grow too large.
    pub unsnapshotted: LogSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocDir {
    Missing,
    /// Created, but its directory entry may not be durable yet.
    Created,
    Durable,
}

/// The segment the doc's next write goes to. A segment is only ever `Open` if this session
/// created it and every write to it succeeded, so the writer can't append to a segment from
/// an earlier session or after a write error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Segment {
    /// The next write creates this segment.
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

    /// A failed write or sync leaves unknown bytes in the file, so writing moves to a new one.
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

/// When a doc's unsnapshotted logs get compacted (DATA-6).
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
