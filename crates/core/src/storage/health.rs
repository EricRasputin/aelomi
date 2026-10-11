use std::io;

/// Why writes are failing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailReason {
    DiskFull,
    PermissionDenied,
    ReadOnlyVolume,
    Io(String),
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
