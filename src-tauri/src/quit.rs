use std::sync::{Mutex, MutexGuard, PoisonError};

use aelomi_core::storage::DocId;
use serde::Serialize;
use specta::Type;

/// The quit decision the shell holds. `pending_docs` is set while a refused quit waits for the
/// user's answer. `revision` grows with every change, so the webview can order events, snapshots,
/// and replies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct QuitStatus {
    pub revision: u32,
    pub pending_docs: Option<Vec<String>>,
}

/// Every change of the quit decision: a refused quit, Keep app open, or Quit anyway.
#[derive(Clone, Serialize, Type, tauri_specta::Event)]
pub struct QuitStatusChanged(pub QuitStatus);

/// Lives as long as the process. Each mutator returns the new status when it changed the
/// decision, for the caller to publish, and `None` when it changed nothing.
#[derive(Default)]
pub struct QuitGate(Mutex<Gate>);

#[derive(Default)]
struct Gate {
    revision: u32,
    state: State,
}

#[derive(Default)]
enum State {
    #[default]
    Open,
    Asking(Vec<String>),
    Confirmed,
}

impl Gate {
    fn status(&self) -> QuitStatus {
        QuitStatus {
            revision: self.revision,
            pending_docs: match &self.state {
                State::Asking(docs) => Some(docs.clone()),
                State::Open | State::Confirmed => None,
            },
        }
    }

    fn change(&mut self, state: State) -> QuitStatus {
        self.revision = self.revision.saturating_add(1);
        self.state = state;
        self.status()
    }
}

impl QuitGate {
    pub fn confirmed(&self) -> bool {
        matches!(self.lock().state, State::Confirmed)
    }

    /// Records a refused quit as a new question. `None` once Quit anyway is confirmed, so the
    /// quit goes on.
    pub fn refuse(&self, pending: &[DocId]) -> Option<QuitStatus> {
        let mut gate = self.lock();
        if matches!(gate.state, State::Confirmed) {
            return None;
        }
        Some(gate.change(State::Asking(
            pending.iter().map(DocId::to_string).collect(),
        )))
    }

    pub fn status(&self) -> QuitStatus {
        self.lock().status()
    }

    /// Keep app open answers the question asked at `revision`, and no newer one.
    pub fn keep_open(&self, revision: u32) -> Option<QuitStatus> {
        let mut gate = self.lock();
        let asked = matches!(gate.state, State::Asking(_)) && gate.revision == revision;
        asked.then(|| gate.change(State::Open))
    }

    /// Quit anyway. `Some` only the first time.
    pub fn confirm(&self) -> Option<QuitStatus> {
        let mut gate = self.lock();
        let first = !matches!(gate.state, State::Confirmed);
        first.then(|| gate.change(State::Confirmed))
    }

    /// Never panics, because `macos::should_terminate` is `extern "C"`.
    fn lock(&self) -> MutexGuard<'_, Gate> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docs(ids: &[&str]) -> Vec<DocId> {
        ids.iter().map(|id| DocId::new(*id).unwrap()).collect()
    }

    fn asking(revision: u32, docs: &[&str]) -> QuitStatus {
        QuitStatus {
            revision,
            pending_docs: Some(docs.iter().map(|doc| doc.to_string()).collect()),
        }
    }

    fn open(revision: u32) -> QuitStatus {
        QuitStatus {
            revision,
            pending_docs: None,
        }
    }

    #[test]
    fn each_refusal_asks_a_newer_question_with_its_own_docs() {
        let gate = QuitGate::default();
        assert_eq!(gate.status(), open(0));

        assert_eq!(gate.refuse(&docs(&["a"])), Some(asking(1, &["a"])));
        assert_eq!(
            gate.refuse(&docs(&["a", "b"])),
            Some(asking(2, &["a", "b"]))
        );
        assert_eq!(gate.status(), asking(2, &["a", "b"]));
        assert!(!gate.confirmed());
    }

    #[test]
    fn keep_open_answers_only_the_question_the_user_saw() {
        let gate = QuitGate::default();
        gate.refuse(&docs(&["a"]));
        gate.refuse(&docs(&["a"]));

        assert_eq!(gate.keep_open(1), None);
        assert_eq!(gate.status(), asking(2, &["a"]));
        assert_eq!(gate.keep_open(2), Some(open(3)));
        assert_eq!(gate.keep_open(2), None);
        assert_eq!(gate.status(), open(3));

        assert_eq!(gate.refuse(&docs(&["a"])), Some(asking(4, &["a"])));
    }

    #[test]
    fn quit_anyway_is_sticky_and_outranks_every_answer() {
        let gate = QuitGate::default();
        gate.refuse(&docs(&["a"]));

        assert_eq!(gate.confirm(), Some(open(2)));
        assert_eq!(gate.confirm(), None);
        assert_eq!(gate.keep_open(1), None);
        assert_eq!(gate.keep_open(2), None);
        assert!(gate.confirmed());
        assert_eq!(gate.refuse(&docs(&["a"])), None);
        assert_eq!(gate.status(), open(2));
    }
}
