use std::io;
use std::time::Duration;

use super::doc_id::DocId;

/// Why writes are failing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailReason {
    DiskFull,
    PermissionDenied,
    ReadOnlyVolume,
    Io(String),
    /// A bug in the store stopped a write. It is retried like any other failure.
    Internal,
}

impl From<&io::Error> for FailReason {
    fn from(error: &io::Error) -> Self {
        match error.raw_os_error() {
            Some(libc::ENOSPC | libc::EDQUOT) => Self::DiskFull,
            Some(libc::EACCES | libc::EPERM) => Self::PermissionDenied,
            Some(libc::EROFS) => Self::ReadOnlyVolume,
            _ => Self::Io(error.to_string()),
        }
    }
}

/// Whether every write is succeeding. Failing writes are retried; see [`retry_delay`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Failing {
        reason: FailReason,
        /// When writes started failing, on the store's monotonic clock.
        since: Duration,
        /// Docs whose updates are waiting on a failed write.
        pending_docs: Vec<DocId>,
        pending_settings: bool,
    },
}

pub(crate) fn retry_delay(attempt: u32) -> Duration {
    let first = Duration::from_millis(100);
    let cap = Duration::from_secs(5);
    first
        .saturating_mul(1 << attempt.saturating_sub(1).min(16))
        .min(cap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_errors_map_to_reasons() {
        let reason = |errno| FailReason::from(&io::Error::from_raw_os_error(errno));
        assert_eq!(reason(libc::ENOSPC), FailReason::DiskFull);
        assert_eq!(reason(libc::EDQUOT), FailReason::DiskFull);
        assert_eq!(reason(libc::EACCES), FailReason::PermissionDenied);
        assert_eq!(reason(libc::EPERM), FailReason::PermissionDenied);
        assert_eq!(reason(libc::EROFS), FailReason::ReadOnlyVolume);
        assert_eq!(
            reason(libc::EIO),
            FailReason::Io(io::Error::from_raw_os_error(libc::EIO).to_string())
        );
    }

    #[test]
    fn retries_back_off_from_100_ms_doubling_to_5_s() {
        let delays: Vec<u128> = (1..=9).map(|a| retry_delay(a).as_millis()).collect();
        assert_eq!(delays, [100, 200, 400, 800, 1600, 3200, 5000, 5000, 5000]);
        assert_eq!(retry_delay(u32::MAX), Duration::from_secs(5));
    }
}
