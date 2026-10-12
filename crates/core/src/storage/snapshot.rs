pub(crate) const SNAPSHOT_MAGIC: [u8; 8] = *b"AESNP\0\0\x01";
const HEADER: usize = SNAPSHOT_MAGIC.len() + 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum SnapshotFormat {
    V1,
    V2,
}

impl SnapshotFormat {
    const ALL: [Self; 2] = [Self::V1, Self::V2];

    pub fn extension(self) -> &'static str {
        match self {
            Self::V1 => "ysnap",
            Self::V2 => "ysnap2",
        }
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.extension() == extension)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Condition {
    Whole,
    Damaged,
}

pub(crate) fn read(format: SnapshotFormat, bytes: &[u8]) -> Option<(&[u8], Condition)> {
    match format {
        SnapshotFormat::V1 => read_v1(bytes).map(|payload| (payload, Condition::Whole)),
        SnapshotFormat::V2 => None,
    }
}

pub(crate) fn encode_snapshot(payload: &[u8]) -> Vec<u8> {
    let len = u32::try_from(payload.len()).expect("compaction skips a snapshot over 4 GiB");
    let mut out = Vec::with_capacity(HEADER + payload.len());
    out.extend_from_slice(&SNAPSHOT_MAGIC);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

fn read_v1(bytes: &[u8]) -> Option<&[u8]> {
    let header = bytes.get(..HEADER)?;
    let len = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(header[12..16].try_into().unwrap());
    let payload = &bytes[HEADER..];
    (header[..8] == SNAPSHOT_MAGIC
        && len > 0
        && payload.len() == len
        && crc32fast::hash(payload) == crc)
        .then_some(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_round_trips_and_rejects_any_damage() {
        let bytes = encode_snapshot(b"state");
        assert_eq!(
            read(SnapshotFormat::V1, &bytes),
            Some((&b"state"[..], Condition::Whole))
        );
        assert_eq!(bytes.len(), 16 + 5);

        let mut flipped = bytes.clone();
        flipped[18] ^= 1;
        let mut wrong_magic = bytes.clone();
        wrong_magic[0] = b'X';
        for (name, damaged) in [
            ("truncated", bytes[..bytes.len() - 1].to_vec()),
            ("trailing byte", [bytes.as_slice(), &[0]].concat()),
            ("flipped payload bit", flipped),
            ("wrong magic", wrong_magic),
            ("header only", bytes[..16].to_vec()),
            ("empty", vec![]),
            ("empty payload", encode_snapshot(b"")),
        ] {
            assert_eq!(read(SnapshotFormat::V1, &damaged), None, "{name}");
        }
    }
}
