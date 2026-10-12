mod v1;
mod v2;

pub(crate) use v1::SEGMENT_HEADER;
#[cfg(test)]
pub(crate) use v1::{LOG_MAGIC, Salt, encode_record, encode_segment, segment_header};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum LogFormat {
    V1,
    V2,
}

impl LogFormat {
    pub const WRITTEN: Self = Self::V2;
    const ALL: [Self; 2] = [Self::V1, Self::V2];

    pub fn extension(self) -> &'static str {
        match self {
            Self::V1 => "ylog",
            Self::V2 => "ylog2",
        }
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.extension() == extension)
    }

    pub fn file_header_len(self) -> usize {
        match self {
            Self::V1 => SEGMENT_HEADER,
            Self::V2 => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogEnd {
    Clean,
    Torn,
    Corrupt,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LogContents {
    pub records: Vec<Vec<u8>>,
    pub end: LogEnd,
}

pub(crate) fn read(format: LogFormat, bytes: &[u8]) -> LogContents {
    match format {
        LogFormat::V1 => {
            let v1::LogContents { records, end } = v1::read_log(bytes);
            LogContents {
                records: records.into_iter().map(<[u8]>::to_vec).collect(),
                end,
            }
        }
        LogFormat::V2 => v2::read(bytes),
    }
}

pub(crate) fn encode_frame(update: &[u8], out: &mut Vec<u8>) {
    v2::encode(update, out)
}
