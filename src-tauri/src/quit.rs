use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use aelomi_core::storage::{DocId, Pending};
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
    /// A quit was refused with pending documents or settings, and waits for the user's answer.
    Asking {
        pending_docs: Vec<String>,
        pending_settings: bool,
    },
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
    gate: Arc<Mutex<Gate>>,
    record_edits_done: Arc<Condvar>,
    drain_timeout: Duration,
}

pub struct RecordEdit {
    gate: Arc<Mutex<Gate>>,
    done: Arc<Condvar>,
}

impl Drop for RecordEdit {
    fn drop(&mut self) {
        let mut gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        gate.record_edits -= 1;
        self.done.notify_all();
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum QuitRequest {
    Allow,
    FlushStore,
    StartDrain(QuitStatus),
    DrainInProgress,
}

#[derive(Default)]
struct Gate {
    workspace_open: bool,
    revision: u32,
    frontend: bool,
    phase: Phase,
    record_edits: usize,
}

#[derive(Default)]
enum Phase {
    #[default]
    Open,
    Awaiting,
    Completing,
    FlushingStore,
    Asking(Pending),
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
                Phase::Awaiting | Phase::Completing | Phase::FlushingStore => QuitPhase::Draining,
                Phase::Asking(pending) => QuitPhase::Asking {
                    pending_docs: pending.docs.iter().map(ToString::to_string).collect(),
                    pending_settings: pending.settings,
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
        matches!(self.phase, Phase::Awaiting | Phase::Completing) && self.revision == revision
    }

    fn awaiting_at(&self, revision: u32) -> bool {
        matches!(self.phase, Phase::Awaiting) && self.revision == revision
    }
}

impl QuitGate {
    pub fn for_workspace(workspace_open: bool) -> Self {
        let quit = Self::default();
        quit.lock().workspace_open = workspace_open;
        quit
    }

    pub fn with_drain_timeout(drain_timeout: Duration) -> Self {
        Self {
            gate: Arc::new(Mutex::new(Gate {
                workspace_open: true,
                ..Gate::default()
            })),
            record_edits_done: Arc::default(),
            drain_timeout,
        }
    }

    pub fn workspace_opened(&self) {
        self.lock().workspace_open = true;
    }

    pub fn record_edit(&self) -> Option<RecordEdit> {
        let mut gate = self.lock();
        if matches!(
            gate.phase,
            Phase::Completing | Phase::Confirmed | Phase::FlushingStore
        ) {
            return None;
        }
        gate.record_edits += 1;
        Some(RecordEdit {
            gate: self.gate.clone(),
            done: self.record_edits_done.clone(),
        })
    }

    pub fn wait_for_record_edits(&self, deadline: Instant) -> bool {
        let mut gate = self.lock();
        let revision = gate.revision;
        while gate.record_edits != 0
            && gate.revision == revision
            && matches!(gate.phase, Phase::Completing | Phase::FlushingStore)
        {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            gate = self
                .record_edits_done
                .wait_timeout(gate, remaining)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
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
            _ if !gate.frontend => {
                if gate.workspace_open {
                    gate.phase = Phase::FlushingStore;
                }
                QuitRequest::FlushStore
            }
            Phase::Awaiting | Phase::Completing | Phase::FlushingStore => {
                QuitRequest::DrainInProgress
            }
            Phase::Open | Phase::Asking(_) => QuitRequest::StartDrain(gate.change(Phase::Awaiting)),
        }
    }

    pub fn refuse(&self, pending: &[DocId], pending_settings: bool) -> Option<QuitStatus> {
        let mut gate = self.lock();
        if matches!(gate.phase, Phase::Confirmed) {
            return None;
        }
        Some(gate.change(Phase::Asking(Pending {
            docs: pending.to_vec(),
            settings: pending_settings,
        })))
    }

    pub fn status(&self) -> QuitStatus {
        self.lock().status()
    }

    pub fn begin_completing(&self, revision: u32) -> bool {
        let mut gate = self.lock();
        if !gate.draining_at(revision) {
            return false;
        }
        gate.phase = Phase::Completing;
        true
    }

    pub fn finish_drain(
        &self,
        revision: u32,
        pending: &[DocId],
        pending_settings: bool,
    ) -> Option<QuitStatus> {
        let mut gate = self.lock();
        if !gate.draining_at(revision) {
            return None;
        }
        Some(if pending.is_empty() && !pending_settings {
            gate.change(Phase::Confirmed)
        } else {
            gate.change(Phase::Asking(Pending {
                docs: pending.to_vec(),
                settings: pending_settings,
            }))
        })
    }

    pub fn drain_failed(
        &self,
        revision: u32,
        docs: &[DocId],
        pending_settings: bool,
    ) -> Option<QuitStatus> {
        let mut gate = self.lock();
        if !gate.draining_at(revision) {
            return None;
        }
        Some(gate.change(Phase::Asking(Pending {
            docs: docs.to_vec(),
            settings: pending_settings,
        })))
    }

    pub fn awaiting_drain(&self, revision: u32) -> bool {
        self.lock().awaiting_at(revision)
    }

    pub fn drain_timed_out(
        &self,
        revision: u32,
        pending: &[DocId],
        pending_settings: bool,
    ) -> Option<QuitStatus> {
        let mut gate = self.lock();
        if !gate.awaiting_at(revision) {
            return None;
        }
        Some(gate.change(Phase::Asking(Pending {
            docs: pending.to_vec(),
            settings: pending_settings,
        })))
    }

    /// Keep app open answers the question asked at `revision`, and no newer one.
    pub fn keep_open(&self, revision: u32) -> Option<QuitStatus> {
        let mut gate = self.lock();
        let asked = matches!(gate.phase, Phase::Asking(_)) && gate.revision == revision;
        let status = asked.then(|| gate.change(Phase::Open));
        self.record_edits_done.notify_all();
        status
    }

    /// Quit anyway. `Some` only the first time.
    pub fn confirm(&self) -> Option<QuitStatus> {
        let mut gate = self.lock();
        let first = !matches!(gate.phase, Phase::Confirmed);
        let status = first.then(|| gate.change(Phase::Confirmed));
        if status.is_some() {
            self.record_edits_done.notify_all();
        }
        status
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

    #[test]
    fn a_refused_quit_then_a_second_request_admits_record_commands_during_its_drain() {
        let gate = QuitGate::default();
        gate.register_frontend();
        let QuitRequest::StartDrain(first) = gate.request() else {
            panic!()
        };
        assert!(gate.begin_completing(first.revision));
        gate.wait_for_record_edits(Instant::now() + Duration::from_secs(2));
        gate.finish_drain(first.revision, &docs(&["workspace"]), false)
            .unwrap();
        let QuitRequest::StartDrain(second) = gate.request() else {
            panic!()
        };
        assert_eq!(second.phase, QuitPhase::Draining);
        let rename = gate
            .record_edit()
            .expect("the new drain accepts its rename");
        drop(rename);
        assert!(gate.begin_completing(second.revision));
        gate.wait_for_record_edits(Instant::now() + Duration::from_secs(2));
        assert_eq!(
            gate.finish_drain(second.revision, &[], false)
                .unwrap()
                .phase,
            QuitPhase::Confirmed
        );
    }

    #[test]
    fn a_stale_timeout_resuming_after_keep_open_leaves_record_admission_open() {
        let gate = QuitGate::default();
        gate.register_frontend();
        let QuitRequest::StartDrain(first) = gate.request() else {
            panic!()
        };
        assert!(gate.awaiting_drain(first.revision));
        let asked = gate.drain_failed(first.revision, &[], false).unwrap();
        gate.keep_open(asked.revision).unwrap();
        gate.wait_for_record_edits(Instant::now() + Duration::from_secs(2));
        assert_eq!(gate.drain_timed_out(first.revision, &[], false), None);
        assert_eq!(gate.status().phase, QuitPhase::Open);
        let rename = gate
            .record_edit()
            .expect("the stale timer cannot close admission");
        drop(rename);
    }

    #[test]
    fn a_completing_record_wait_reaches_its_deadline_and_asks_without_losing_the_guard() {
        let gate = QuitGate::default();
        gate.register_frontend();
        let edit = gate.record_edit().unwrap();
        let QuitRequest::StartDrain(status) = gate.request() else {
            panic!()
        };
        assert!(gate.begin_completing(status.revision));
        assert!(!gate.wait_for_record_edits(Instant::now() + Duration::from_millis(20)));
        assert_eq!(gate.lock().record_edits, 1);
        assert!(gate.record_edit().is_none());
        let asked = gate
            .finish_drain(status.revision, &docs(&["workspace"]), false)
            .unwrap();
        assert_eq!(
            asked.phase,
            QuitPhase::Asking {
                pending_docs: vec!["workspace".into()],
                pending_settings: false,
            }
        );
        gate.keep_open(asked.revision).unwrap();
        let next = gate
            .record_edit()
            .expect("the timeout allows Keep app open");
        drop(next);
        drop(edit);
        assert_eq!(gate.lock().record_edits, 0);
    }

    fn status(revision: u32, phase: QuitPhase) -> QuitStatus {
        QuitStatus { revision, phase }
    }

    fn asking(revision: u32, docs: &[&str]) -> QuitStatus {
        status(
            revision,
            QuitPhase::Asking {
                pending_docs: docs.iter().map(|doc| doc.to_string()).collect(),
                pending_settings: false,
            },
        )
    }

    #[test]
    fn record_commands_do_not_block_a_drain_timeout_or_explicit_quit_confirmation() {
        use std::sync::mpsc;
        for confirmed in [false, true] {
            let gate = QuitGate::default();
            let edit = gate.record_edit().unwrap();
            gate.register_frontend();
            gate.request();
            if confirmed {
                gate.confirm();
            }
            let (send, receive) = mpsc::channel();
            let finished_before_release = std::thread::scope(|scope| {
                let waiting = scope.spawn(|| {
                    gate.wait_for_record_edits(Instant::now() + Duration::from_secs(2));
                    send.send(()).unwrap();
                });
                let finished = receive.recv_timeout(Duration::from_millis(100)).is_ok();
                drop(edit);
                waiting.join().unwrap();
                finished
            });
            assert!(
                finished_before_release,
                "a non-confirming timeout or explicit confirmation cannot wait on a stalled command"
            );
            assert_eq!(gate.record_edit().is_none(), confirmed);
        }
    }

    #[test]
    fn explicit_confirmation_releases_an_already_waiting_record_barrier() {
        use std::sync::mpsc;
        use std::time::Instant;
        let gate = QuitGate::default();
        let edit = gate.record_edit().unwrap();
        gate.register_frontend();
        let QuitRequest::StartDrain(status) = gate.request() else {
            panic!()
        };
        assert!(gate.begin_completing(status.revision));
        let (send, receive) = mpsc::channel();
        let finished_before_release = std::thread::scope(|scope| {
            let waiting = scope.spawn(|| {
                gate.wait_for_record_edits(Instant::now() + Duration::from_secs(2));
                send.send(()).unwrap();
            });
            let deadline = Instant::now() + Duration::from_secs(1);
            while !matches!(gate.lock().phase, Phase::Completing) {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            gate.confirm();
            let finished = receive.recv_timeout(Duration::from_millis(100)).is_ok();
            drop(edit);
            waiting.join().unwrap();
            finished
        });
        assert!(
            finished_before_release,
            "Quit anyway must release the normal drain wait"
        );
        assert_eq!(gate.status().phase, QuitPhase::Confirmed);
    }

    #[test]
    fn record_command_admission_closes_for_quit_and_keep_open_reopens_it() {
        let gate = QuitGate::default();
        let edit = gate.record_edit().expect("open command admission");
        drop(edit);
        assert_eq!(gate.request(), QuitRequest::FlushStore);
        gate.wait_for_record_edits(Instant::now() + Duration::from_secs(2));
        assert!(gate.record_edit().is_none());
        let asked = gate.refuse(&docs(&["workspace"]), false).unwrap();
        assert_eq!(
            gate.keep_open(asked.revision).unwrap().phase,
            QuitPhase::Open
        );
        let edit = gate.record_edit().expect("keep open admits commands again");
        drop(edit);
        gate.confirm();
        assert!(gate.record_edit().is_none());
    }

    #[test]
    fn each_refusal_asks_a_newer_question_with_its_own_docs() {
        let gate = QuitGate::default();
        assert_eq!(gate.status(), status(0, QuitPhase::Open));
        assert_eq!(gate.request(), QuitRequest::FlushStore);

        assert_eq!(gate.refuse(&docs(&["a"]), false), Some(asking(1, &["a"])));
        assert_eq!(
            gate.refuse(&docs(&["a", "b"]), false),
            Some(asking(2, &["a", "b"]))
        );
        assert_eq!(gate.status(), asking(2, &["a", "b"]));
    }

    #[test]
    fn keep_open_answers_only_the_question_the_user_saw() {
        let gate = QuitGate::default();
        gate.refuse(&docs(&["a"]), false);
        gate.refuse(&docs(&["a"]), false);

        assert_eq!(gate.keep_open(1), None);
        assert_eq!(gate.status(), asking(2, &["a"]));
        assert_eq!(gate.keep_open(2), Some(status(3, QuitPhase::Open)));
        assert_eq!(gate.keep_open(2), None);
        assert_eq!(gate.status(), status(3, QuitPhase::Open));

        assert_eq!(gate.refuse(&docs(&["a"]), false), Some(asking(4, &["a"])));
    }

    #[test]
    fn quit_anyway_is_sticky_and_outranks_every_answer() {
        let gate = QuitGate::default();
        gate.refuse(&docs(&["a"]), false);

        assert_eq!(gate.confirm(), Some(status(2, QuitPhase::Confirmed)));
        assert_eq!(gate.confirm(), None);
        assert_eq!(gate.keep_open(1), None);
        assert_eq!(gate.keep_open(2), None);
        assert_eq!(gate.refuse(&docs(&["a"]), false), None);
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
            QuitRequest::StartDrain(status(1, QuitPhase::Draining))
        );
        assert_eq!(gate.request(), QuitRequest::DrainInProgress);
        assert_eq!(gate.status(), status(1, QuitPhase::Draining));
        assert!(!gate.begin_completing(0));
        assert_eq!(gate.finish_drain(0, &[], false), None);
        assert!(gate.begin_completing(1));
        assert_eq!(
            gate.drain_timed_out(1, &[], false),
            None,
            "the frontend timeout does not replace completion"
        );
        assert_eq!(
            gate.finish_drain(1, &[], false),
            Some(status(2, QuitPhase::Confirmed))
        );
        assert_eq!(gate.request(), QuitRequest::Allow);
    }

    #[test]
    fn a_drain_that_fails_or_times_out_asks_and_a_late_reply_changes_nothing() {
        let gate = QuitGate::default();
        gate.register_frontend();
        gate.request();
        assert_eq!(gate.drain_failed(0, &docs(&["a"]), false), None);
        assert_eq!(
            gate.drain_failed(1, &docs(&["a"]), false),
            Some(asking(2, &["a"]))
        );
        assert!(
            !gate.begin_completing(1),
            "the reply came after the question"
        );
        assert_eq!(gate.keep_open(2), Some(status(3, QuitPhase::Open)));

        assert_eq!(
            gate.request(),
            QuitRequest::StartDrain(status(4, QuitPhase::Draining))
        );
        assert_eq!(gate.drain_timed_out(3, &docs(&["b"]), false), None);
        assert_eq!(
            gate.drain_timed_out(4, &docs(&["b"]), false),
            Some(asking(5, &["b"]))
        );
        assert_eq!(
            gate.request(),
            QuitRequest::StartDrain(status(6, QuitPhase::Draining)),
            "quitting again while asked drains again"
        );
        assert!(gate.begin_completing(6));
        assert_eq!(
            gate.finish_drain(6, &docs(&["c"]), false),
            Some(asking(7, &["c"]))
        );
    }

    #[test]
    fn a_completing_drain_refuses_repeats_and_its_deadline_but_accepts_its_results() {
        let gate = QuitGate::default();
        gate.register_frontend();
        assert_eq!(
            gate.request(),
            QuitRequest::StartDrain(status(1, QuitPhase::Draining))
        );
        assert!(gate.awaiting_drain(1));
        assert!(gate.begin_completing(1));

        assert_eq!(gate.status(), status(1, QuitPhase::Draining));
        assert_eq!(gate.request(), QuitRequest::DrainInProgress);
        assert_eq!(gate.status(), status(1, QuitPhase::Draining));
        assert!(!gate.awaiting_drain(1));
        assert_eq!(gate.drain_timed_out(1, &docs(&["a"]), false), None);
        assert!(
            gate.begin_completing(1),
            "a duplicate completion is accepted"
        );
        assert_eq!(gate.status(), status(1, QuitPhase::Draining));
        assert_eq!(
            gate.drain_failed(1, &docs(&["a"]), false),
            Some(asking(2, &["a"]))
        );
        assert_eq!(
            gate.finish_drain(1, &[], false),
            None,
            "the other completion is stale"
        );
        assert!(!gate.begin_completing(1));
        assert_eq!(gate.status(), asking(2, &["a"]));
    }
}
