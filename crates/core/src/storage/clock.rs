use std::time::{Duration, Instant};

pub(crate) trait Clock: Send + Sync {
    fn now(&self) -> Duration;
    #[cfg(test)]
    fn sleep(&self, duration: Duration);
}

pub(crate) struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }

    #[cfg(test)]
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct ManualClock {
    now: std::sync::Mutex<Duration>,
}

#[cfg(test)]
impl ManualClock {
    pub fn advance(&self, by: Duration) {
        *self.now.lock().unwrap() += by;
    }

    pub fn advance_to(&self, at: Duration) {
        let mut now = self.now.lock().unwrap();
        *now = (*now).max(at);
    }
}

#[cfg(test)]
impl Clock for ManualClock {
    fn now(&self) -> Duration {
        *self.now.lock().unwrap()
    }

    fn sleep(&self, duration: Duration) {
        self.advance(duration);
    }
}
