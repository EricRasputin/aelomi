use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use super::health::{FailReason, retry_delay};
use super::state::{Failure, Seq, lock_ignoring_poison};
use super::store::Shared;

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
    admissions: usize,
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

    pub fn wait(&self, seq: Seq) -> Result<(), FailReason> {
        let mut state = lock_ignoring_poison(&self.state);
        loop {
            if state.durable.is_some_and(|durable| durable >= seq) {
                return Ok(());
            }
            if state.attempted.is_some_and(|attempted| attempted >= seq)
                && let Some(failure) = &state.failure
            {
                return Err(failure.reason.clone());
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    pub fn admit(&self) {
        lock_ignoring_poison(&self.state).admissions += 1;
    }

    pub fn release(&self) {
        lock_ignoring_poison(&self.state).admissions -= 1;
        self.changed.notify_all();
    }

    pub fn is_pending(&self) -> bool {
        let state = lock_ignoring_poison(&self.state);
        state.admissions > 0 || state.pending.is_some()
    }

    pub fn failed(&self, seq: Seq, reason: FailReason, now: Duration) {
        let mut state = lock_ignoring_poison(&self.state);
        let attempts = state
            .failure
            .as_ref()
            .map_or(1, |failure| failure.attempts.saturating_add(1));
        let retry_at = if state
            .pending
            .as_ref()
            .is_some_and(|(pending, _)| *pending > seq)
        {
            Duration::ZERO
        } else {
            now + retry_delay(attempts)
        };
        state.failure = Some(Failure {
            reason,
            attempts,
            retry_at,
        });
        state.attempted = Some(seq);
        self.changed.notify_all();
    }
}

pub(crate) enum Attempt {
    Finished,
    Idle(Option<Duration>),
}

pub(crate) fn write_queued(shared: &Shared, retry_failing: bool) -> Attempt {
    let slot = &shared.settings;
    let (seq, settings) = {
        let state = lock_ignoring_poison(&slot.state);
        let Some(pending) = &state.pending else {
            return Attempt::Idle(None);
        };
        if !retry_failing
            && let Some(failure) = &state.failure
            && failure.retry_at > shared.clock.now()
        {
            return Attempt::Idle(Some(failure.retry_at));
        }
        pending.clone()
    };
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        write(&*shared.fs, &shared.layout, &settings).map_err(|error| FailReason::from(&error))
    }))
    .unwrap_or(Err(FailReason::Internal));
    match result {
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
        Err(reason) => slot.failed(seq, reason, shared.clock.now()),
    }
    Attempt::Finished
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
    let source = create_durable(fs, &temp, &bytes)?;
    fs.replace(layout.root(), RootFile::Settings, source)?;
    fs.fsync(layout.root())?;
    fs.full_sync(layout.root())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
