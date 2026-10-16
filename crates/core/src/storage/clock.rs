use std::time::{Duration, Instant};

pub(crate) trait Clock: Send + Sync {
    fn now(&self) -> Duration;
    fn unix_ms(&self) -> u64;
    #[cfg(any(test, feature = "test-support"))]
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

    fn unix_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64
    }

    #[cfg(any(test, feature = "test-support"))]
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

    fn unix_ms(&self) -> u64 {
        self.now().as_millis() as u64
    }

    fn sleep(&self, duration: Duration) {
        self.advance(duration);
    }
}
