use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use aelomi_core::storage::DocId;
use serde::Serialize;
use specta::Type;

pub const QUIT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// The quit decision the shell holds. `revision` grows with every change, so the webview can
/// order events, snapshots, and replies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct QuitStatus {
    pub revision: u32,
    pub phase: QuitPhase,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum QuitPhase {
    /// No quit is in progress.
    Open,
    /// A quit waits for the webview to deliver its edits and call `complete_quit` with this
    /// revision.
    Draining,
    /// A quit was refused with these docs not yet saved, and waits for the user's answer.
    Asking { pending_docs: Vec<String> },
    /// The app is quitting.
    Confirmed,
}

/// Every change of the quit decision: a quit request, a drain result, Keep app open, or Quit
/// anyway.
#[derive(Clone, Serialize, Type, tauri_specta::Event)]
pub struct QuitStatusChanged(pub QuitStatus);

/// Lives as long as the process. Each mutator returns the new status when it changed the
/// decision, for the caller to publish, and `None` when it changed nothing.
pub struct QuitGate {
    gate: Mutex<Gate>,
    drain_timeout: Duration,
}

#[derive(Debug, PartialEq, Eq)]
pub enum QuitRequest {
    Allow,
    FlushStore,
    Drain(Option<QuitStatus>),
}

#[derive(Default)]
struct Gate {
    revision: u32,
    frontend: bool,
    phase: Phase,
}

#[derive(Default)]
enum Phase {
    #[default]
    Open,
    Draining {
        completing: bool,
    },
    Asking(Vec<String>),
    Confirmed,
}

impl Default for QuitGate {
    fn default() -> Self {
        Self::with_drain_timeout(QUIT_DRAIN_TIMEOUT)
    }
}

impl Gate {
    fn status(&self) -> QuitStatus {
        QuitStatus {
            revision: self.revision,
            phase: match &self.phase {
                Phase::Open => QuitPhase::Open,
                Phase::Draining { .. } => QuitPhase::Draining,
                Phase::Asking(docs) => QuitPhase::Asking {
                    pending_docs: docs.clone(),
                },
                Phase::Confirmed => QuitPhase::Confirmed,
            },
        }
    }

    fn change(&mut self, phase: Phase) -> QuitStatus {
        self.revision = self.revision.saturating_add(1);
        self.phase = phase;
        self.status()
    }

    fn draining_at(&self, revision: u32) -> bool {
        matches!(self.phase, Phase::Draining { .. }) && self.revision == revision
    }
}

impl QuitGate {
    pub fn with_drain_timeout(drain_timeout: Duration) -> Self {
        Self {
            gate: Mutex::default(),
            drain_timeout,
        }
    }

    pub fn drain_timeout(&self) -> Duration {
        self.drain_timeout
    }

    pub fn register_frontend(&self) {
        self.lock().frontend = true;
    }

    pub fn request(&self) -> QuitRequest {
        let mut gate = self.lock();
        match gate.phase {
            Phase::Confirmed => QuitRequest::Allow,
            _ if !gate.frontend => QuitRequest::FlushStore,
            Phase::Draining { .. } => QuitRequest::Drain(None),
            Phase::Open | Phase::Asking(_) => {
                QuitRequest::Drain(Some(gate.change(Phase::Draining { completing: false })))
            }
        }
    }

    pub fn refuse(&self, pending: &[DocId]) -> Option<QuitStatus> {
        let mut gate = self.lock();
        if matches!(gate.phase, Phase::Confirmed) {
            return None;
        }
        Some(gate.change(Phase::Asking(
            pending.iter().map(DocId::to_string).collect(),
        )))
    }

    pub fn status(&self) -> QuitStatus {
        self.lock().status()
    }

    pub fn begin_completing(&self, revision: u32) -> bool {
        let mut gate = self.lock();
        if !gate.draining_at(revision) {
            return false;
        }
        gate.phase = Phase::Draining { completing: true };
        true
    }

    pub fn finish_drain(&self, revision: u32, pending: &[DocId]) -> Option<QuitStatus> {
        let mut gate = self.lock();
        if !gate.draining_at(revision) {
            return None;
        }
        Some(if pending.is_empty() {
            gate.change(Phase::Confirmed)
        } else {
            gate.change(Phase::Asking(
                pending.iter().map(DocId::to_string).collect(),
            ))
        })
    }

    pub fn drain_failed(&self, revision: u32, docs: &[DocId]) -> Option<QuitStatus> {
        let mut gate = self.lock();
        if !gate.draining_at(revision) {
            return None;
        }
        Some(gate.change(Phase::Asking(docs.iter().map(DocId::to_string).collect())))
    }

    pub fn awaiting_drain(&self, revision: u32) -> bool {
        let gate = self.lock();
        matches!(gate.phase, Phase::Draining { completing: false }) && gate.revision == revision
    }

    pub fn drain_timed_out(&self, revision: u32, pending: &[DocId]) -> Option<QuitStatus> {
        let mut gate = self.lock();
        if !matches!(gate.phase, Phase::Draining { completing: false }) || gate.revision != revision
        {
            return None;
        }
        Some(gate.change(Phase::Asking(
            pending.iter().map(DocId::to_string).collect(),
        )))
    }

    /// Keep app open answers the question asked at `revision`, and no newer one.
    pub fn keep_open(&self, revision: u32) -> Option<QuitStatus> {
        let mut gate = self.lock();
        let asked = matches!(gate.phase, Phase::Asking(_)) && gate.revision == revision;
        asked.then(|| gate.change(Phase::Open))
    }

    /// Quit anyway. `Some` only the first time.
    pub fn confirm(&self) -> Option<QuitStatus> {
        let mut gate = self.lock();
        let first = !matches!(gate.phase, Phase::Confirmed);
        first.then(|| gate.change(Phase::Confirmed))
    }

    /// Never panics, because `macos::should_terminate` is `extern "C"`.
    fn lock(&self) -> MutexGuard<'_, Gate> {
        self.gate.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docs(ids: &[&str]) -> Vec<DocId> {
        ids.iter().map(|id| DocId::new(*id).unwrap()).collect()
    }

    fn status(revision: u32, phase: QuitPhase) -> QuitStatus {
        QuitStatus { revision, phase }
    }

    fn asking(revision: u32, docs: &[&str]) -> QuitStatus {
        status(
            revision,
            QuitPhase::Asking {
                pending_docs: docs.iter().map(|doc| doc.to_string()).collect(),
            },
        )
    }

    #[test]
    fn each_refusal_asks_a_newer_question_with_its_own_docs() {
        let gate = QuitGate::default();
        assert_eq!(gate.status(), status(0, QuitPhase::Open));
        assert_eq!(gate.request(), QuitRequest::FlushStore);

        assert_eq!(gate.refuse(&docs(&["a"])), Some(asking(1, &["a"])));
        assert_eq!(
            gate.refuse(&docs(&["a", "b"])),
            Some(asking(2, &["a", "b"]))
        );
        assert_eq!(gate.status(), asking(2, &["a", "b"]));
    }

    #[test]
    fn keep_open_answers_only_the_question_the_user_saw() {
        let gate = QuitGate::default();
        gate.refuse(&docs(&["a"]));
        gate.refuse(&docs(&["a"]));

        assert_eq!(gate.keep_open(1), None);
        assert_eq!(gate.status(), asking(2, &["a"]));
        assert_eq!(gate.keep_open(2), Some(status(3, QuitPhase::Open)));
        assert_eq!(gate.keep_open(2), None);
        assert_eq!(gate.status(), status(3, QuitPhase::Open));

        assert_eq!(gate.refuse(&docs(&["a"])), Some(asking(4, &["a"])));
    }

    #[test]
    fn quit_anyway_is_sticky_and_outranks_every_answer() {
        let gate = QuitGate::default();
        gate.refuse(&docs(&["a"]));

        assert_eq!(gate.confirm(), Some(status(2, QuitPhase::Confirmed)));
        assert_eq!(gate.confirm(), None);
        assert_eq!(gate.keep_open(1), None);
        assert_eq!(gate.keep_open(2), None);
        assert_eq!(gate.refuse(&docs(&["a"])), None);
        assert_eq!(gate.request(), QuitRequest::Allow);
        assert_eq!(gate.status(), status(2, QuitPhase::Confirmed));
    }

    #[test]
    fn a_registered_webview_drains_before_any_quit_and_repeats_keep_the_revision() {
        let gate = QuitGate::default();
        gate.register_frontend();
        gate.register_frontend();

        assert_eq!(
            gate.request(),
            QuitRequest::Drain(Some(status(1, QuitPhase::Draining)))
        );
        assert_eq!(gate.request(), QuitRequest::Drain(None));
        assert_eq!(gate.status(), status(1, QuitPhase::Draining));
        assert!(!gate.begin_completing(0));
        assert_eq!(gate.finish_drain(0, &[]), None);
        assert!(gate.begin_completing(1));
        assert_eq!(
            gate.drain_timed_out(1, &[]),
            None,
            "a completing drain has no deadline"
        );
        assert_eq!(
            gate.finish_drain(1, &[]),
            Some(status(2, QuitPhase::Confirmed))
        );
        assert_eq!(gate.request(), QuitRequest::Allow);
    }

    #[test]
    fn a_drain_that_fails_or_times_out_asks_and_a_late_reply_changes_nothing() {
        let gate = QuitGate::default();
        gate.register_frontend();
        gate.request();
        assert_eq!(gate.drain_failed(0, &docs(&["a"])), None);
        assert_eq!(gate.drain_failed(1, &docs(&["a"])), Some(asking(2, &["a"])));
        assert!(
            !gate.begin_completing(1),
            "the reply came after the question"
        );
        assert_eq!(gate.keep_open(2), Some(status(3, QuitPhase::Open)));

        assert_eq!(
            gate.request(),
            QuitRequest::Drain(Some(status(4, QuitPhase::Draining)))
        );
        assert_eq!(gate.drain_timed_out(3, &docs(&["b"])), None);
        assert_eq!(
            gate.drain_timed_out(4, &docs(&["b"])),
            Some(asking(5, &["b"]))
        );
        assert_eq!(
            gate.request(),
            QuitRequest::Drain(Some(status(6, QuitPhase::Draining))),
            "quitting again while asked drains again"
        );
        assert!(gate.begin_completing(6));
        assert_eq!(gate.finish_drain(6, &docs(&["c"])), Some(asking(7, &["c"])));
    }
}
