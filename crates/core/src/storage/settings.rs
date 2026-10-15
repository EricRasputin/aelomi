use std::io;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use super::health::{FailReason, retry_delay};
use super::state::{Failure, Seq, lock_ignoring_poison};
use super::store::Shared;
use super::writer::Step;

use super::fs::{Fs, create_durable};
use super::layout::{Layout, RootFile};
use crate::settings::Settings;

#[derive(Default)]
pub(crate) struct Slot {
    pub state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
pub(crate) struct State {
    pending: Option<(Seq, Settings)>,
    durable: Option<Seq>,
    attempted: Option<Seq>,
    pub failure: Option<Failure>,
}

impl Slot {
    pub fn queue(&self, seq: Seq, settings: Settings) {
        let mut state = lock_ignoring_poison(&self.state);
        state.pending = Some((seq, settings));
        if let Some(failure) = &mut state.failure {
            failure.retry_at = Duration::ZERO;
        }
    }

    pub fn wait(&self, seq: Seq) -> io::Result<()> {
        let mut state = lock_ignoring_poison(&self.state);
        loop {
            if state.durable.is_some_and(|durable| durable >= seq) {
                return Ok(());
            }
            if state.attempted.is_some_and(|attempted| attempted >= seq)
                && let Some(failure) = &state.failure
            {
                return Err(io::Error::other(format!(
                    "settings write failed: {:?}",
                    failure.reason
                )));
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    pub fn flush(&self, deadline: Instant) -> bool {
        let mut state = lock_ignoring_poison(&self.state);
        let Some(target) = state.pending.as_ref().map(|(seq, _)| *seq) else {
            return true;
        };
        loop {
            if state.durable.is_some_and(|seq| seq >= target) {
                return true;
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            state = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    pub fn failed(&self, reason: FailReason, now: Duration) -> Option<Duration> {
        let mut state = lock_ignoring_poison(&self.state);
        let seq = state.pending.as_ref()?.0;
        let attempts = state
            .failure
            .as_ref()
            .map_or(1, |failure| failure.attempts.saturating_add(1));
        let retry_at = now + retry_delay(attempts);
        state.failure = Some(Failure {
            reason,
            attempts,
            retry_at,
        });
        state.attempted = Some(seq);
        self.changed.notify_all();
        Some(retry_at)
    }
}

pub(crate) fn write_queued(shared: &Shared, retry_failing: bool) -> Step {
    let slot = &shared.settings;
    let (seq, settings) = {
        let state = lock_ignoring_poison(&slot.state);
        let Some(pending) = &state.pending else {
            return Step::Idle { retry_at: None };
        };
        if !retry_failing
            && let Some(failure) = &state.failure
            && failure.retry_at > shared.clock.now()
        {
            return Step::Idle {
                retry_at: Some(failure.retry_at),
            };
        }
        pending.clone()
    };
    match write(&*shared.fs, &shared.layout, &settings) {
        Ok(()) => {
            let mut state = lock_ignoring_poison(&slot.state);
            state.durable = Some(seq);
            state.attempted = Some(seq);
            state.failure = None;
            if state
                .pending
                .as_ref()
                .is_some_and(|(pending, _)| *pending == seq)
            {
                state.pending = None;
            }
            slot.changed.notify_all();
        }
        Err(error) => {
            slot.failed(FailReason::from(&error), shared.clock.now());
        }
    }
    Step::Wrote
}

pub(crate) fn load(fs: &dyn Fs, layout: &Layout) -> io::Result<Settings> {
    let bytes = match fs.read(&layout.file(RootFile::Settings)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(error) => {
            eprintln!("settings read failed, refusing updates until a read succeeds: {error}");
            return Err(error);
        }
    };
    Ok(Settings::from_json(&bytes).unwrap_or_else(|error| {
        eprintln!("settings parse failed, using defaults: {error}");
        Settings::default()
    }))
}

pub(crate) fn write(fs: &dyn Fs, layout: &Layout, settings: &Settings) -> io::Result<()> {
    let temp = layout.temp_file(RootFile::Settings);
    match fs.remove(&temp) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let bytes = serde_json::to_vec_pretty(&settings.to_json())?;
    create_durable(fs, &temp, &bytes)?;
    fs.replace(layout.root(), RootFile::Settings)?;
    fs.fsync(layout.root())?;
    fs.full_sync(layout.root())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
