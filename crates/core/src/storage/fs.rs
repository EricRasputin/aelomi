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
    #[allow(
        dead_code,
        reason = "the format-2 publication gate lands in a later PR"
    )]
    fn replace_format(&self, root: &Path) -> io::Result<()>;
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

pub(crate) fn validate_format_marker(bytes: &[u8]) -> io::Result<()> {
    if super::format::parse_format(bytes).is_some_and(|version| version > 0) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid format marker",
        ))
    }
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
        // Store's folder lock serializes publishers. A dangling symlink is also an entry.
        match std::fs::symlink_metadata(to) {
            Ok(_) => return Err(io::ErrorKind::AlreadyExists.into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        std::fs::rename(from, to)
    }

    fn replace_format(&self, root: &Path) -> io::Result<()> {
        let from = root.join("format.json.tmp");
        validate_format_marker(&self.read(&from)?)?;
        // The OS cannot report prior durability, so establish it before publishing the name.
        self.full_sync(&from)?;
        std::fs::rename(from, root.join("format.json"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_publication_os_replace_publishes_a_complete_marker() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("format.json"), br#"{"format":1}"#).unwrap();
        create_durable(&OsFs, &root.join("format.json.tmp"), br#"{"format":2}"#).unwrap();
        OsFs.replace_format(root).unwrap();
        OsFs.fsync(root).unwrap();
        OsFs.full_sync(root).unwrap();
        assert_eq!(
            OsFs.read(&root.join("format.json")).unwrap(),
            br#"{"format":2}"#
        );
        assert_eq!(OsFs.list(root).unwrap(), ["format.json"]);
    }

    #[test]
    fn format_publication_os_replace_preserves_the_old_marker_on_invalid_source() {
        for bytes in [&b"{\"format\":2"[..], br#"{"format":0}"#, b"{}"] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            std::fs::write(root.join("format.json"), br#"{"format":1}"#).unwrap();
            create_durable(&OsFs, &root.join("format.json.tmp"), bytes).unwrap();
            assert_eq!(
                OsFs.replace_format(root).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
            assert_eq!(
                OsFs.read(&root.join("format.json")).unwrap(),
                br#"{"format":1}"#
            );
            assert_eq!(OsFs.read(&root.join("format.json.tmp")).unwrap(), bytes);
        }
    }

    #[test]
    fn format_publication_os_plain_rename_preserves_existing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("format.json.tmp");
        let to = dir.path().join("format.json");
        std::fs::write(&from, br#"{"format":2}"#).unwrap();
        std::fs::write(&to, br#"{"format":1}"#).unwrap();
        let error = OsFs.rename(&from, &to).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&from).unwrap(), br#"{"format":2}"#);
        assert_eq!(std::fs::read(&to).unwrap(), br#"{"format":1}"#);
    }
}
