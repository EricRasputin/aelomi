use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use super::HoldPoint;

#[derive(Default)]
pub(crate) struct Gate {
    state: Mutex<GateState>,
    changed: Condvar,
}

#[derive(Default)]
struct GateState {
    armed: Vec<HoldPoint>,
    held: Option<HoldPoint>,
    open: bool,
}

impl Gate {
    pub fn arm(&self, point: HoldPoint) {
        self.lock().armed.push(point);
    }

    pub fn wait_held(&self, point: &HoldPoint) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut state = self.lock();
        while state.held.as_ref() != Some(point) {
            let now = Instant::now();
            assert!(now < deadline, "the worker never reached {point:?}");
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    pub fn release(&self) {
        self.lock().held = None;
        self.changed.notify_all();
    }

    pub(super) fn open_all(&self) {
        let mut state = self.lock();
        state.open = true;
        state.armed.clear();
        state.held = None;
        drop(state);
        self.changed.notify_all();
    }

    pub(super) fn pass(&self, point: HoldPoint) {
        let mut state = self.lock();
        let Some(at) = state.armed.iter().position(|p| *p == point) else {
            return;
        };
        if state.open {
            return;
        }
        state.armed.remove(at);
        state.held = Some(point);
        self.changed.notify_all();
        while state.held.is_some() && !state.open {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn lock(&self) -> MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
