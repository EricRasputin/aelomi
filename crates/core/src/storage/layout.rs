use std::path::{Path, PathBuf};

use super::doc_id::DocId;
use super::snapshot::SnapshotFormat;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Generation(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SegmentNo(pub u64);

impl Generation {
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl SegmentNo {
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum DocFile {
    Snapshot(Generation, SnapshotFormat),
    SnapshotTmp(Generation, SnapshotFormat),
    Log(Generation, SegmentNo),
}

impl DocFile {
    pub fn generation(self) -> Generation {
        match self {
            Self::Snapshot(g, _) | Self::SnapshotTmp(g, _) | Self::Log(g, _) => g,
        }
    }

    pub fn name(self) -> String {
        match self {
            Self::Snapshot(g, f) => format!("snapshot-{:010}.{}", g.0, f.extension()),
            Self::SnapshotTmp(g, f) => format!("snapshot-{:010}.{}.tmp", g.0, f.extension()),
            Self::Log(g, s) => format!("log-{:010}-{:010}.ylog", g.0, s.0),
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        if let Some(rest) = name.strip_prefix("snapshot-") {
            let tmp = rest.strip_suffix(".tmp");
            let (g, extension) = tmp.unwrap_or(rest).split_once('.')?;
            let (g, f) = (
                Generation(number(g)?),
                SnapshotFormat::from_extension(extension)?,
            );
            return Some(if tmp.is_some() {
                Self::SnapshotTmp(g, f)
            } else {
                Self::Snapshot(g, f)
            });
        }
        let rest = name.strip_prefix("log-")?.strip_suffix(".ylog")?;
        let (g, s) = rest.split_once('-')?;
        Some(Self::Log(Generation(number(g)?), SegmentNo(number(s)?)))
    }
}

fn number(digits: &str) -> Option<u64> {
    if digits.len() == 10 && digits.bytes().all(|b| b.is_ascii_digit()) {
        digits.parse().ok()
    } else {
        None
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Layout {
    root: PathBuf,
}

impl Layout {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn format_json(&self) -> PathBuf {
        self.root.join("format.json")
    }

    pub fn format_json_tmp(&self) -> PathBuf {
        self.root.join("format.json.tmp")
    }

    pub fn lock(&self) -> PathBuf {
        self.root.join(".lock")
    }

    pub fn docs(&self) -> PathBuf {
        self.root.join("docs")
    }

    pub fn doc_dir(&self, doc: &DocId) -> PathBuf {
        self.docs().join(doc.as_str())
    }

    pub fn doc_file(&self, doc: &DocId, file: DocFile) -> PathBuf {
        self.doc_dir(doc).join(file.name())
    }

    pub fn quarantine(&self) -> PathBuf {
        self.root.join("quarantine")
    }

    pub fn quarantine_dir(&self, doc: &DocId) -> PathBuf {
        self.quarantine().join(doc.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_round_trip_and_sort_by_number() {
        let files = [
            DocFile::Snapshot(Generation(7), SnapshotFormat::V1),
            DocFile::SnapshotTmp(Generation(7), SnapshotFormat::V1),
            DocFile::Snapshot(Generation(8), SnapshotFormat::V2),
            DocFile::SnapshotTmp(Generation(8), SnapshotFormat::V2),
            DocFile::Log(Generation(7), SegmentNo(12)),
            DocFile::Log(Generation(9_999_999_999), SegmentNo(0)),
        ];
        let names: Vec<String> = files.iter().map(|f| f.name()).collect();
        assert_eq!(
            names,
            [
                "snapshot-0000000007.ysnap",
                "snapshot-0000000007.ysnap.tmp",
                "snapshot-0000000008.ysnap2",
                "snapshot-0000000008.ysnap2.tmp",
                "log-0000000007-0000000012.ylog",
                "log-9999999999-0000000000.ylog",
            ]
        );
        for (file, name) in files.iter().zip(&names) {
            assert_eq!(DocFile::parse(name), Some(*file));
        }
        assert!("log-0000000002-0000000000.ylog" < "log-0000000010-0000000000.ylog");
    }

    #[test]
    fn foreign_names_are_not_store_files() {
        for name in [
            "snapshot-7.ysnap",
            "snapshot-000000000a.ysnap",
            "snapshot-7.ysnap2",
            "snapshot-0000000007",
            "snapshot-0000000007.",
            "snapshot-0000000007.ysnap3",
            "snapshot-0000000007.ysnap2x",
            "snapshot-0000000007.ysnap.ysnap2",
            "snapshot-0000000007.ysnap2.tmp.tmp",
            "snapshot-0000000007.tmp",
            "log-0000000001.ylog",
            "log-0000000001-0000000001.ylog.tmp",
            "notes.txt",
            ".DS_Store",
        ] {
            assert_eq!(DocFile::parse(name), None, "{name}");
        }
    }
}
