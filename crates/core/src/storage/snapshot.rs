use std::ops::Range;

use crc::{CRC_64_NVME, Crc, Table};

pub(crate) const SNAPSHOT_MAGIC: [u8; 8] = *b"AESNP\0\0\x01";
const HEADER: usize = SNAPSHOT_MAGIC.len() + 8;

const HEAD_MAGIC: [u8; 8] = *b"AESNP2\0A";
const TAIL_MAGIC: [u8; 8] = *b"AESNP2\0B";
const ANCHOR: usize = 32;
const BLOCK: usize = 4096;
static CRC_64: Crc<u64, Table<16>> = Crc::<u64, Table<16>>::new(&CRC_64_NVME);

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
        SnapshotFormat::V2 => read_v2(bytes),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Anchor {
    len: usize,
    crc: u64,
}

impl Anchor {
    fn parse(bytes: &[u8], magic: [u8; 8]) -> Option<Self> {
        let bytes: &[u8; ANCHOR] = bytes.try_into().ok()?;
        let field = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
        let check = u32::from_le_bytes(bytes[28..].try_into().unwrap());
        let valid = bytes[..8] == magic
            && bytes[24..28] == [0; 4]
            && crc32fast::hash(&bytes[..28]) == check;
        let len = usize::try_from(field(8)).ok().filter(|&len| len > 0)?;
        valid.then_some(Self {
            len,
            crc: field(16),
        })
    }

    fn copies(self) -> Option<[Range<usize>; 2]> {
        let a_end = ANCHOR.checked_add(self.len)?;
        let b = a_end.checked_next_multiple_of(BLOCK)?.checked_add(BLOCK)?;
        Some([ANCHOR..a_end, b..b.checked_add(self.len)?])
    }

    fn file_len(self) -> Option<usize> {
        self.copies()?[1].end.checked_add(ANCHOR)
    }

    fn tail_agreeing_with_file_len(bytes: &[u8]) -> Option<Self> {
        let tail = Self::parse(&bytes[bytes.len().checked_sub(ANCHOR)?..], TAIL_MAGIC)?;
        (tail.file_len() == Some(bytes.len())).then_some(tail)
    }
}

fn read_v2(bytes: &[u8]) -> Option<(&[u8], Condition)> {
    let head = Anchor::parse(bytes.get(..ANCHOR)?, HEAD_MAGIC);
    let tail = Anchor::tail_agreeing_with_file_len(bytes);
    let intact = |copy: usize| {
        [head, tail].into_iter().flatten().find_map(|anchor| {
            let payload = bytes.get(anchor.copies()?[copy].clone())?;
            (CRC_64.checksum(payload) == anchor.crc).then_some(payload)
        })
    };
    let (a, b) = (intact(0), intact(1));
    let zero_padding = |anchor: Anchor| {
        let [a, b] = anchor.copies()?;
        Some(bytes.get(a.end..b.start)?.iter().all(|&byte| byte == 0))
    };
    let whole =
        head == tail && a.is_some() && b.is_some() && head.and_then(zero_padding) == Some(true);
    let condition = if whole {
        Condition::Whole
    } else {
        Condition::Damaged
    };
    Some((a.or(b)?, condition))
}

#[cfg(test)]
impl Anchor {
    fn encode(self, magic: [u8; 8]) -> [u8; ANCHOR] {
        let mut out = [0; ANCHOR];
        out[..8].copy_from_slice(&magic);
        out[8..16].copy_from_slice(&(self.len as u64).to_le_bytes());
        out[16..24].copy_from_slice(&self.crc.to_le_bytes());
        let check = crc32fast::hash(&out[..28]);
        out[28..].copy_from_slice(&check.to_le_bytes());
        out
    }
}

#[cfg(test)]
pub(crate) fn encode_snapshot_v2(payload: &[u8]) -> Vec<u8> {
    let anchor = Anchor {
        len: payload.len(),
        crc: CRC_64.checksum(payload),
    };
    let [a, b] = anchor.copies().expect("a payload that fits in memory");
    let mut out = vec![0; b.end + ANCHOR];
    out[..ANCHOR].copy_from_slice(&anchor.encode(HEAD_MAGIC));
    out[a].copy_from_slice(payload);
    out[b.clone()].copy_from_slice(payload);
    out[b.end..].copy_from_slice(&anchor.encode(TAIL_MAGIC));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::testkit::SplitMix64;

    fn payload(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8 + 1).collect()
    }

    fn zeroed(bytes: &[u8], range: Range<usize>) -> Vec<u8> {
        let mut out = bytes.to_vec();
        out[range].fill(0);
        out
    }

    fn assert_loads(bytes: &[u8], original: &[u8], payload: &[u8], case: &str) {
        let condition = if bytes == original {
            Condition::Whole
        } else {
            Condition::Damaged
        };
        assert_eq!(
            read(SnapshotFormat::V2, bytes),
            Some((payload, condition)),
            "{case}"
        );
    }

    #[test]
    fn a_format_2_snapshot_puts_its_anchors_and_copies_at_fixed_offsets() {
        let head =
            b"AESNP2\0A\x09\0\0\0\0\0\0\0\x88\x98\x79\x0a\x86\x14\x8b\xae\0\0\0\0\x6f\x80\x0f\x22";
        let tail =
            b"AESNP2\0B\x09\0\0\0\0\0\0\0\x88\x98\x79\x0a\x86\x14\x8b\xae\0\0\0\0\x8c\x02\x11\x59";
        let bytes = encode_snapshot_v2(b"123456789");
        let expected = [&head[..], b"123456789", &[0; 8151], b"123456789", &tail[..]].concat();
        assert_eq!(bytes, expected);
        assert_eq!(
            read(SnapshotFormat::V2, &bytes),
            Some((&b"123456789"[..], Condition::Whole))
        );
    }

    #[test]
    fn each_format_reads_only_its_own_files() {
        let v1 = encode_snapshot(b"state");
        let v2 = encode_snapshot_v2(b"state");
        assert_eq!(read(SnapshotFormat::V1, &v2), None);
        assert_eq!(read(SnapshotFormat::V2, &v1), None);
        assert_eq!(read(SnapshotFormat::V2, &encode_snapshot_v2(b"")), None);
        assert_eq!(read(SnapshotFormat::V2, &[]), None);
        assert_eq!(read(SnapshotFormat::V2, &[0; 8225]), None);
    }

    #[test]
    fn no_aligned_4_kib_block_touches_both_copies() {
        let fixed = [
            (1, 8_225),
            (4_000, 12_224),
            (4_064, 12_288),
            (4_096, 16_416),
            (100_000, 206_528),
        ];
        let mut rng = SplitMix64::new(4096);
        let random = (0..64).map(|_| (1 + rng.below(300_000) as usize, None));
        for (len, file_len) in fixed
            .map(|(len, file)| (len, Some(file)))
            .into_iter()
            .chain(random)
        {
            let payload = payload(len);
            let bytes = encode_snapshot_v2(&payload);
            if let Some(file_len) = file_len {
                assert_eq!(bytes.len(), file_len, "len {len}");
            }
            let a_end = 32 + len;
            let b_start = bytes.len() - 32 - len;
            assert_eq!(bytes[32..a_end], payload, "len {len}: copy A");
            assert_eq!(
                bytes[b_start..bytes.len() - 32],
                payload,
                "len {len}: copy B"
            );
            let (last_a_block, first_b_block) = ((a_end - 1) / 4096, b_start / 4096);
            assert!(
                first_b_block >= last_a_block + 2,
                "len {len}: copy A ends in block {last_a_block}, copy B starts in block {first_b_block}"
            );
        }
    }

    #[test]
    fn every_single_bit_flip_leaves_the_exact_payload() {
        for len in [1, 100, 4_064, 4_096] {
            let payload = payload(len);
            let mut bytes = encode_snapshot_v2(&payload);
            for bit in 0..bytes.len() * 8 {
                bytes[bit / 8] ^= 1 << (bit % 8);
                assert_eq!(
                    read(SnapshotFormat::V2, &bytes),
                    Some((&payload[..], Condition::Damaged)),
                    "len {len}, bit {bit}"
                );
                bytes[bit / 8] ^= 1 << (bit % 8);
            }
        }
    }

    #[test]
    fn every_aligned_block_or_64_byte_zero_fill_leaves_the_exact_payload() {
        for len in [1, 100, 4_000, 4_064, 4_096, 100_000] {
            let payload = payload(len);
            let bytes = encode_snapshot_v2(&payload);
            let eof = bytes.len();
            let blocks = (0..eof).step_by(4096).map(|at| at..eof.min(at + 4096));
            let edges = [32, 32 + len, eof - 32 - len, eof - 32];
            let straddles = edges
                .into_iter()
                .flat_map(|edge| edge.saturating_sub(63)..edge)
                .map(|at| at..eof.min(at + 64));
            for range in blocks.chain(straddles) {
                let case = format!("len {len}, zeroed {range:?}");
                assert_loads(&zeroed(&bytes, range), &bytes, &payload, &case);
            }
        }
    }

    #[test]
    fn the_judges_64_zero_bytes_between_the_copies_leave_a_copy() {
        let payload = payload(4_000);
        let bytes = encode_snapshot_v2(&payload);
        for range in [4_000..4_064, 8_160..8_224, 4_032..8_192] {
            let case = format!("zeroed {range:?}");
            assert_loads(&zeroed(&bytes, range), &bytes, &payload, &case);
        }
    }

    #[test]
    fn damage_to_both_copies_or_both_anchors_reads_nothing() {
        let bytes = encode_snapshot_v2(&payload(4_000));
        let mut both_copies = bytes.clone();
        both_copies[32 + 1_000] ^= 1;
        both_copies[8_192 + 3_000] ^= 1;
        let both_anchors = zeroed(&zeroed(&bytes, 0..32), 12_192..12_224);
        let both_copies_by_one_fill_each = zeroed(&zeroed(&bytes, 0..4_096), 8_192..12_224);
        for (name, damaged) in [
            ("a bit in each copy", both_copies),
            ("both anchors zeroed", both_anchors),
            ("a block over each copy", both_copies_by_one_fill_each),
        ] {
            assert_eq!(read(SnapshotFormat::V2, &damaged), None, "{name}");
        }
    }

    #[test]
    fn a_file_whose_length_moved_loads_only_through_a_copy_its_head_still_names() {
        let payload = payload(4_000);
        let bytes = encode_snapshot_v2(&payload);
        for (name, moved, loads) in [
            ("one byte short", bytes[..12_223].to_vec(), true),
            ("one byte long", [&bytes[..], &[0]].concat(), true),
            ("cut after copy A", bytes[..4_032].to_vec(), true),
            ("cut inside copy A", bytes[..2_000].to_vec(), false),
            (
                "one byte short, head zeroed",
                zeroed(&bytes[..12_223], 0..32),
                false,
            ),
            (
                "one byte long, head zeroed",
                zeroed(&[&bytes[..], &[0]].concat(), 0..32),
                false,
            ),
        ] {
            let expected = loads.then_some((&payload[..], Condition::Damaged));
            assert_eq!(read(SnapshotFormat::V2, &moved), expected, "{name}");
        }
    }

    #[test]
    fn a_tail_inside_a_cut_payload_is_not_an_anchor() {
        let planted = b"ghost";
        let forged = Anchor {
            len: planted.len(),
            crc: CRC_64.checksum(planted),
        }
        .encode(TAIL_MAGIC);
        let note = [&planted[..], &[7; 95], &forged[..], &[7; 4_000]].concat();
        let bytes = encode_snapshot_v2(&note);
        let cut = zeroed(&bytes[..32 + 100 + 32], 0..32);
        assert_eq!(read(SnapshotFormat::V2, &cut), None);
    }

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
