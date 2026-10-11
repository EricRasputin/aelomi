use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::Path;

pub(crate) trait Fs: Send + Sync {
    fn create_dir(&self, path: &Path) -> io::Result<()>;
    fn create_new(&self, path: &Path) -> io::Result<()>;
    fn append(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Plain `fsync` of a file or directory: data reaches the drive, maybe only its cache.
    fn fsync(&self, path: &Path) -> io::Result<()>;
    /// `F_FULLFSYNC`: `fsync`, then the drive flushes its whole cache to permanent storage.
    fn full_sync(&self, path: &Path) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove(&self, path: &Path) -> io::Result<()>;
    fn list(&self, dir: &Path) -> io::Result<Vec<String>>;
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn lock(&self, path: &Path) -> Result<FsLock, LockError>;
}

/// Writes `path` and makes it durable, so a rename can publish it without exposing a name
/// that points at bytes a crash could lose.
pub(crate) fn create_durable(fs: &dyn Fs, path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs.create_new(path)?;
    fs.append(path, bytes)?;
    fs.fsync(path)?;
    fs.full_sync(path)
}

pub(crate) struct FsLock {
    _guard: Box<dyn Send + Sync>,
}

impl FsLock {
    pub fn new(guard: impl Send + Sync + 'static) -> Self {
        Self {
            _guard: Box::new(guard),
        }
    }
}

#[derive(Debug)]
pub(crate) enum LockError {
    Held,
    Io(io::Error),
}

pub(crate) struct OsFs;

impl Fs for OsFs {
    fn create_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir(path)
    }

    fn create_new(&self, path: &Path) -> io::Result<()> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(drop)
    }

    fn append(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        OpenOptions::new().append(true).open(path)?.write_all(bytes)
    }

    fn fsync(&self, path: &Path) -> io::Result<()> {
        let file = File::open(path)?;
        // std's `sync_all` is `F_FULLFSYNC` on Apple targets, so plain `fsync` comes from libc.
        if unsafe { libc::fsync(file.as_raw_fd()) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    // On Apple targets `File::sync_all` calls `fcntl(fd, F_FULLFSYNC)` (Rust 1.98.1,
    // `library/std/src/sys/fs/unix.rs`, `os_fsync`). Apple's fcntl(2): F_FULLFSYNC "does the
    // same thing as fsync(2) then asks the drive to flush all buffered data to the permanent
    // storage device ... data that had been fsync'd on the same device before is guaranteed to
    // be persisted when this call returns." So one call after plain `fsync` of every file and
    // directory in a batch makes the whole batch durable.
    fn full_sync(&self, path: &Path) -> io::Result<()> {
        File::open(path)?.sync_all()
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<String>> {
        let mut names = std::fs::read_dir(dir)?
            .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
            .collect::<io::Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn lock(&self, path: &Path) -> Result<FsLock, LockError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(LockError::Io)?;
        match file.try_lock() {
            Ok(()) => Ok(FsLock::new(file)),
            Err(TryLockError::WouldBlock) => Err(LockError::Held),
            Err(TryLockError::Error(e)) => Err(LockError::Io(e)),
        }
    }
}
