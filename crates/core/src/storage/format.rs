use std::io;
use std::path::Path;
use std::sync::Mutex;

use super::fs::{Fs, create_durable};
use super::layout::Layout;
use super::state::lock_ignoring_poison;

pub(crate) const FORMAT_VERSION: u64 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FolderVersion {
    Legacy1 = 1,
    Current2 = 2,
}

impl FolderVersion {
    pub fn of(found: u64) -> Option<Self> {
        match found {
            1 => Some(Self::Legacy1),
            2 => Some(Self::Current2),
            _ => None,
        }
    }
}

pub(crate) fn encode_format(version: FolderVersion) -> String {
    format!("{{\n  \"format\": {}\n}}\n", version as u64)
}

pub(crate) fn parse_format(bytes: &[u8]) -> Option<u64> {
    let json: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    json.get("format")?.as_u64()
}

pub(crate) struct FormatGate {
    state: Mutex<GateState>,
}

enum GateState {
    /// Seeing 2 in `format.json` doesn't show it survives a power loss. A rename can sit in
    /// the drive's cache after a failed sync, or after the process that made it exited.
    Unconfirmed(FolderVersion),
    Ready2,
}

pub(crate) struct Format2Ready {
    _private: (),
}

impl Format2Ready {
    pub fn create_new(&self, fs: &dyn Fs, path: &Path) -> io::Result<()> {
        fs.create_new(path)
    }
}

impl FormatGate {
    pub fn new(found: FolderVersion) -> Self {
        Self {
            state: Mutex::new(GateState::Unconfirmed(found)),
        }
    }

    pub fn ready(&self) -> Option<Format2Ready> {
        let state = lock_ignoring_poison(&self.state);
        matches!(*state, GateState::Ready2).then_some(Format2Ready { _private: () })
    }

    pub fn ensure_v2(&self, fs: &dyn Fs, layout: &Layout) -> io::Result<Format2Ready> {
        let mut state = lock_ignoring_poison(&self.state);
        match *state {
            GateState::Ready2 => {}
            GateState::Unconfirmed(FolderVersion::Legacy1) => {
                publish(fs, layout)?;
                *state = GateState::Unconfirmed(FolderVersion::Current2);
                sync_folder(fs, layout)?;
            }
            GateState::Unconfirmed(FolderVersion::Current2) => {
                fs.fsync(&layout.format_json())?;
                sync_folder(fs, layout)?;
            }
        }
        *state = GateState::Ready2;
        Ok(Format2Ready { _private: () })
    }
}

fn publish(fs: &dyn Fs, layout: &Layout) -> io::Result<()> {
    let (tmp, marker) = (
        layout.format_json_tmp(),
        encode_format(FolderVersion::Current2),
    );
    match create_durable(fs, &tmp, marker.as_bytes()) {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            fs.remove(&tmp)?;
            create_durable(fs, &tmp, marker.as_bytes())?;
        }
        result => result?,
    }
    fs.replace_format(layout.root())
}

fn sync_folder(fs: &dyn Fs, layout: &Layout) -> io::Result<()> {
    fs.fsync(layout.root())?;
    fs.full_sync(layout.root())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_json_round_trips_and_reads_only_its_format() {
        assert_eq!(
            encode_format(FolderVersion::Legacy1),
            "{\n  \"format\": 1\n}\n"
        );
        assert_eq!(
            encode_format(FolderVersion::Current2),
            "{\n  \"format\": 2\n}\n"
        );
        assert_eq!(
            parse_format(encode_format(FolderVersion::Legacy1).as_bytes()),
            Some(1)
        );
        assert_eq!(parse_format(b"{\"format\":2}"), Some(2));
        assert_eq!(parse_format(b" { \"format\" : 12 } "), Some(12));
        assert_eq!(parse_format(b"{\"format\":1,\"x\":2}"), Some(1));
        assert_eq!(
            parse_format(b"{\"format\":4294967296}"),
            Some(4_294_967_296)
        );
        assert_eq!(
            parse_format(b"{\"format\":18446744073709551615}"),
            Some(u64::MAX)
        );
        assert_eq!(
            parse_format(b"{\"x\":[1,{\"format\":3},\"}\"],\"format\":2}"),
            Some(2)
        );
        for bad in [
            &b""[..],
            b"{}",
            b"{\"x\":1}",
            b"{\"format\":\"1\"}",
            b"{\"format\":-1}",
            b"{\"format\":1.5}",
            b"{\"format\":null}",
            b"[{\"format\":1}]",
            b"{\"format\":1",
            b"\xff",
        ] {
            assert_eq!(
                parse_format(bad),
                None,
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
    }
}
