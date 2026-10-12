use std::cell::OnceCell;
use std::io;

use super::decode;

pub(crate) const LOG_MAGIC: [u8; 8] = *b"AELOG\0\0\x02";
pub(crate) const SEGMENT_HEADER: usize = LOG_MAGIC.len() + 8;
const RECORD_HEADER: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Salt([u8; 8]);

impl Salt {
    pub fn random() -> io::Result<Self> {
        let mut salt = [0; 8];
        getrandom::fill(&mut salt)?;
        Ok(Self(salt))
    }

    fn hasher(self) -> crc32fast::Hasher {
        let mut crc = crc32fast::Hasher::new();
        crc.update(&self.0);
        crc
    }

    fn crc(self, payload: &[u8]) -> u32 {
        let mut crc = self.hasher();
        crc.update(payload);
        crc.finalize()
    }
}

pub(crate) fn segment_header(salt: Salt) -> Vec<u8> {
    [&LOG_MAGIC[..], &salt.0].concat()
}

pub(crate) fn encode_record(salt: Salt, payload: &[u8], out: &mut Vec<u8>) {
    let len = u32::try_from(payload.len()).expect("append rejects an update over 4 GiB");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&salt.crc(payload).to_le_bytes());
    out.extend_from_slice(payload);
}

#[cfg(test)]
pub(crate) fn encode_segment<P: AsRef<[u8]>>(payloads: &[P]) -> Vec<u8> {
    let salt = Salt::random().unwrap();
    let mut out = segment_header(salt);
    for payload in payloads {
        encode_record(salt, payload.as_ref(), &mut out);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogEnd {
    Clean,
    Torn,
    Corrupt,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LogContents<'a> {
    pub records: Vec<&'a [u8]>,
    pub end: LogEnd,
}

pub(crate) fn read_log(bytes: &[u8]) -> LogContents<'_> {
    read_log_decoding(bytes, bytes.len().saturating_mul(SCAN_DECODED_PER_BYTE))
}

const SCAN_DECODED_PER_BYTE: usize = 64;
const RECOVERY_WORK_PER_BYTE: usize = 64;

pub(crate) fn read_log_decoding(bytes: &[u8], mut decode_budget: usize) -> LogContents<'_> {
    let Some(header) = bytes.get(..SEGMENT_HEADER) else {
        let magic = &bytes[..bytes.len().min(LOG_MAGIC.len())];
        let end = if LOG_MAGIC.starts_with(magic) {
            LogEnd::Torn
        } else {
            LogEnd::Corrupt
        };
        return LogContents {
            records: Vec::new(),
            end,
        };
    };
    if header[..LOG_MAGIC.len()] != LOG_MAGIC {
        return LogContents {
            records: Vec::new(),
            end: LogEnd::Corrupt,
        };
    }
    let salt = Salt(header[LOG_MAGIC.len()..].try_into().unwrap());
    let mut contents = LogContents {
        records: Vec::new(),
        end: LogEnd::Clean,
    };
    let mut recovery = bytes.len().saturating_mul(RECOVERY_WORK_PER_BYTE);
    let crcs = OnceCell::new();
    let crcs = || crcs.get_or_init(|| PrefixCrcs::new(bytes, salt));
    let mut at = SEGMENT_HEADER;
    while at < bytes.len() {
        if let Some(payload) = record_at(bytes, at, salt) {
            contents.records.push(payload);
            at += RECORD_HEADER + payload.len();
            continue;
        }
        if let Some(payload) = recover_length(bytes, at, salt, &mut recovery) {
            contents.records.push(payload);
            contents.end = LogEnd::Corrupt;
            at += RECORD_HEADER + payload.len();
            continue;
        }
        let stated_end = stated_end(bytes, at, salt);
        let scan = match stated_end {
            Some(end) if decode::decodes_exactly(&bytes[at + RECORD_HEADER..end]) => Scan::NoRecord,
            Some(end) => resync(crcs().records_chaining_to(at + 1, end), &mut decode_budget),
            None => resync(crcs().records_from(at + 1), &mut decode_budget),
        };
        let torn = contents.end == LogEnd::Clean
            && a_torn_write_explains(bytes, at)
            && matches!(scan, Scan::NoRecord);
        contents.end = if torn { LogEnd::Torn } else { LogEnd::Corrupt };
        at = match (scan, stated_end) {
            (Scan::ResumeAt(next), _) | (_, Some(next)) => next,
            _ => break,
        };
    }
    contents
}

fn a_torn_write_explains(bytes: &[u8], at: usize) -> bool {
    let rest = &bytes[at..];
    let Some(header) = rest.get(..RECORD_HEADER) else {
        return true;
    };
    let len = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
    RECORD_HEADER + len >= rest.len() || rest.iter().all(|&b| b == 0)
}

#[derive(Clone, Copy)]
enum Scan {
    ResumeAt(usize),
    NoRecord,
    NoUpdate,
    OverBudget,
}

fn resync<'a>(candidates: impl Iterator<Item = (usize, &'a [u8])>, budget: &mut usize) -> Scan {
    let mut scan = Scan::NoRecord;
    for (at, payload) in candidates {
        let Some(left) = budget.checked_sub(payload.len()) else {
            scan = Scan::OverBudget;
            continue;
        };
        *budget = left;
        match decode::check(payload) {
            Err(decode::Rejected::TooDeep) => return Scan::ResumeAt(at),
            Ok(()) if decode::update(payload).is_ok() => return Scan::ResumeAt(at),
            _ => {}
        }
        scan = Scan::NoUpdate;
    }
    scan
}

fn stated_end(bytes: &[u8], at: usize, salt: Salt) -> Option<usize> {
    let (_, payload) = candidate_at(bytes, at)?;
    let end = at + RECORD_HEADER + payload.len();
    boundary_at(bytes, end, salt).then_some(end)
}

fn boundary_at(bytes: &[u8], at: usize, salt: Salt) -> bool {
    at == bytes.len()
        || record_at(bytes, at, salt).is_some_and(|record| decode::update(record).is_ok())
}

struct PrefixCrcs<'a> {
    bytes: &'a [u8],
    salt: u32,
    every_64: Vec<u32>,
}

impl<'a> PrefixCrcs<'a> {
    fn new(bytes: &'a [u8], salt: Salt) -> Self {
        let mut crc = crc32fast::Hasher::new();
        let mut every_64 = vec![crc.clone().finalize()];
        for chunk in bytes.chunks(64) {
            crc.update(chunk);
            every_64.push(crc.clone().finalize());
        }
        Self {
            bytes,
            salt: crc32fast::hash(&salt.0),
            every_64,
        }
    }

    fn records_from(&self, from: usize) -> impl Iterator<Item = (usize, &'a [u8])> {
        let end = self.bytes.len();
        (from..end).filter_map(move |at| Some((at, self.record_at(at, end)?)))
    }

    fn records_chaining_to(
        &self,
        from: usize,
        end: usize,
    ) -> impl Iterator<Item = (usize, &'a [u8])> {
        let mut chains = vec![false; end - from];
        for at in (from..end).rev() {
            chains[at - from] = self.record_at(at, end).is_some_and(|payload| {
                let next = at + RECORD_HEADER + payload.len();
                next == end || chains[next - from]
            });
        }
        let bytes = &self.bytes[..end];
        let chaining = (from..end).filter(move |&at| chains[at - from]);
        chaining.filter_map(move |at| Some((at, candidate_at(bytes, at)?.1)))
    }

    fn record_at(&self, at: usize, end: usize) -> Option<&'a [u8]> {
        let (crc, payload) = candidate_at(&self.bytes[..end], at)?;
        let start = at + RECORD_HEADER;
        (self.salted_crc(start, start + payload.len()) == crc).then_some(payload)
    }

    fn salted_crc(&self, start: usize, end: usize) -> u32 {
        let len = end - start;
        let range = self.prefix(end) ^ combine(self.prefix(start), 0, len);
        combine(self.salt, range, len)
    }

    fn prefix(&self, end: usize) -> u32 {
        let chunk = end / 64;
        let mut crc = crc32fast::Hasher::new_with_initial(self.every_64[chunk]);
        crc.update(&self.bytes[chunk * 64..end]);
        crc.finalize()
    }
}

fn combine(crc: u32, then: u32, len: usize) -> u32 {
    let mut combined = crc32fast::Hasher::new_with_initial(crc);
    combined.combine(&crc32fast::Hasher::new_with_initial_len(then, len as u64));
    combined.finalize()
}

fn recover_length<'a>(
    bytes: &'a [u8],
    at: usize,
    salt: Salt,
    budget: &mut usize,
) -> Option<&'a [u8]> {
    let header = bytes.get(at..at + RECORD_HEADER)?;
    let crc = u32::from_le_bytes(header[4..].try_into().unwrap());
    let start = at + RECORD_HEADER;
    let mut spend = |work: usize| {
        let left = budget.checked_sub(work);
        *budget = left.unwrap_or(0);
        left.map(|_| ())
    };
    let mut hasher = salt.hasher();
    for end in start + 1..=bytes.len() {
        spend(1)?;
        hasher.update(&bytes[end - 1..end]);
        if hasher.clone().finalize() != crc {
            continue;
        }
        let payload = &bytes[start..end];
        spend(payload.len())?;
        if decode::decodes_exactly(payload) && boundary_at(bytes, end, salt) {
            return Some(payload);
        }
    }
    None
}

fn record_at(bytes: &[u8], at: usize, salt: Salt) -> Option<&[u8]> {
    let (crc, payload) = candidate_at(bytes, at)?;
    (salt.crc(payload) == crc).then_some(payload)
}

/// The store never writes an empty payload, so a zero length is invalid. That keeps a
/// zero-filled tail from reading as records.
fn candidate_at(bytes: &[u8], at: usize) -> Option<(u32, &[u8])> {
    let header = bytes.get(at..at + RECORD_HEADER)?;
    let len = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(header[4..].try_into().unwrap());
    let start = at + RECORD_HEADER;
    let payload = bytes.get(start..start.checked_add(len)?)?;
    (len > 0).then_some((crc, payload))
}

#[cfg(test)]
mod tests {
    use proptest::collection::vec;
    use proptest::prelude::*;
    use proptest::sample::Index;
    use yrs::{Any, Doc, Map, Text, Transact};

    use super::*;
    use crate::storage::testkit::UpdateGen;

    const SALT: Salt = Salt(*b"testsalt");

    fn segment(payloads: &[&[u8]]) -> Vec<u8> {
        let mut out = segment_header(SALT);
        for p in payloads {
            encode_record(SALT, p, &mut out);
        }
        out
    }

    fn record(payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        encode_record(SALT, payload, &mut out);
        out
    }

    fn typed(n: usize) -> Vec<u8> {
        note(7, &"x".repeat(n))
    }

    fn note(writer: u64, text: &str) -> Vec<u8> {
        let client = Doc::with_client_id(writer);
        let t = client.get_or_insert_text("t");
        let mut txn = client.transact_mut();
        t.insert(&mut txn, 0, text);
        txn.encode_update_v1()
    }

    fn carrying(writer: u64, blob: &[u8]) -> Vec<u8> {
        let client = Doc::with_client_id(writer);
        let blobs = client.get_or_insert_map("blobs");
        let mut txn = client.transact_mut();
        blobs.insert(&mut txn, "blob", Any::Buffer(blob.into()));
        txn.encode_update_v1()
    }

    #[test]
    fn records_round_trip() {
        let bytes = segment(&[b"one", b"two two", &[0u8; 300]]);
        assert_eq!(
            read_log(&bytes),
            LogContents {
                records: vec![&b"one"[..], b"two two", &[0u8; 300]],
                end: LogEnd::Clean
            }
        );
        assert_eq!(bytes.len(), 16 + (8 + 3) + (8 + 7) + (8 + 300));
        assert_eq!(&bytes[..16], b"AELOG\0\0\x02testsalt");
        assert_eq!(&bytes[16..24], &[3, 0, 0, 0, 254, 219, 28, 208]);
    }

    #[test]
    fn classifies_damaged_segments_and_reads_every_update_around_the_damage() {
        let updates = UpdateGen::new(5, 1).updates(3);
        let (alpha, beta, gamma) = (&updates[0][..], &updates[1][..], &updates[2][..]);
        let good = segment(&[alpha, beta]);
        let two = segment(&[alpha]).len();
        let third = record(gamma);
        let not_an_update = record(b"not an update");
        let with = |tail: &[u8]| [good.as_slice(), tail].concat();
        let flip = |mut bytes: Vec<u8>, at: usize| {
            bytes[at] ^= 0x40;
            bytes
        };
        let sixty_four = (0..64).map(typed).find(|u| 8 + u.len() == 64).unwrap();
        assert_eq!(
            beta.len() & 0x40,
            0,
            "a flipped 0x40 adds 64 to beta's length"
        );
        let skips_one = flip(
            [good.as_slice(), &record(&sixty_four), &third].concat(),
            two,
        );
        let a: &[&[u8]] = &[alpha];
        let ab: &[&[u8]] = &[alpha, beta];
        let ac: &[&[u8]] = &[alpha, gamma];
        let abc: &[&[u8]] = &[alpha, beta, gamma];
        let abdc: &[&[u8]] = &[alpha, beta, &sixty_four, gamma];
        let none: &[&[u8]] = &[];
        type Case<'a> = (&'a str, Vec<u8>, &'a [&'a [u8]], LogEnd);
        let cases: Vec<Case> = vec![
            ("empty file", vec![], none, LogEnd::Torn),
            ("partial magic", LOG_MAGIC[..5].to_vec(), none, LogEnd::Torn),
            ("wrong magic", b"NOTALOG!".to_vec(), none, LogEnd::Corrupt),
            (
                "short magic, not a prefix",
                b"AEX".to_vec(),
                none,
                LogEnd::Corrupt,
            ),
            ("magic only", LOG_MAGIC.to_vec(), none, LogEnd::Torn),
            (
                "part of the salt",
                segment(&[])[..12].to_vec(),
                none,
                LogEnd::Torn,
            ),
            ("header only", segment(&[]), none, LogEnd::Clean),
            (
                "a record salted for another segment",
                [&LOG_MAGIC[..], b"othersal", &record(alpha)].concat(),
                none,
                LogEnd::Torn,
            ),
            ("short header at end", with(&third[..5]), ab, LogEnd::Torn),
            (
                "short payload at end",
                with(&third[..third.len() - 2]),
                ab,
                LogEnd::Torn,
            ),
            (
                "bad CRC in the last record",
                flip(good.clone(), good.len() - 1),
                a,
                LogEnd::Torn,
            ),
            ("zero-filled tail", with(&[0; 40]), ab, LogEnd::Torn),
            (
                "bad CRC, then a good record",
                flip(with(&third), two + 9),
                ac,
                LogEnd::Corrupt,
            ),
            (
                "absurd length, then a good record",
                flip(with(&third), two + 3),
                abc,
                LogEnd::Corrupt,
            ),
            (
                "a length that lands on the record after next",
                skips_one,
                abdc,
                LogEnd::Corrupt,
            ),
            (
                "bad CRC, then a record that is not an update, then a good record",
                flip([good.as_slice(), &not_an_update, &third].concat(), two + 9),
                ac,
                LogEnd::Corrupt,
            ),
            (
                "absurd length at end",
                flip(good.clone(), two + 3),
                ab,
                LogEnd::Corrupt,
            ),
            (
                "absurd length, then bytes that are not a record",
                flip(with(&[0xab; 20]), two + 3),
                a,
                LogEnd::Torn,
            ),
            (
                "bad CRC in the last two records",
                flip(flip(with(&third), two + 9), good.len() + 9),
                a,
                LogEnd::Corrupt,
            ),
            (
                "bad CRC, then bytes that are not zero",
                flip(with(&[0xab; 20]), two + 9),
                a,
                LogEnd::Corrupt,
            ),
            (
                "bad CRC, then zeros",
                flip(with(&[0; 20]), two + 9),
                a,
                LogEnd::Corrupt,
            ),
        ];
        for (name, bytes, records, end) in cases {
            assert_eq!(
                read_log(&bytes),
                LogContents {
                    records: records.to_vec(),
                    end
                },
                "{name}"
            );
        }
    }

    #[test]
    fn a_resync_skips_records_it_cannot_afford_to_decode_and_never_calls_that_torn() {
        let updates = UpdateGen::new(5, 1).updates(3);
        let (alpha, beta, gamma) = (&updates[0][..], &updates[1][..], &updates[2][..]);
        let two = segment(&[alpha]).len();
        let absurd_length_and_crc = |tail: &[u8]| {
            let mut bytes = [segment(&[alpha, beta]).as_slice(), tail].concat();
            bytes[two + 3] ^= 0x40;
            bytes[two + 4] ^= 1;
            bytes
        };
        let then_gamma = absurd_length_and_crc(&record(gamma));
        let then_junk = absurd_length_and_crc(&[1, 0, 0, 0, 0, 0, 0, 0, 0xab]);
        let a: &[&[u8]] = &[alpha];
        let ac: &[&[u8]] = &[alpha, gamma];
        for (name, bytes, budget, records, end) in [
            (
                "then a record",
                &then_gamma,
                usize::MAX,
                ac,
                LogEnd::Corrupt,
            ),
            (
                "then a record, no budget",
                &then_gamma,
                0,
                a,
                LogEnd::Corrupt,
            ),
            ("then junk", &then_junk, usize::MAX, a, LogEnd::Torn),
            ("then junk, no budget", &then_junk, 0, a, LogEnd::Torn),
        ] {
            assert_eq!(
                read_log_decoding(bytes, budget),
                LogContents {
                    records: records.to_vec(),
                    end
                },
                "{name}"
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            failure_persistence: None,
            ..ProptestConfig::default()
        })]

        #[test]
        fn a_salted_crc_from_prefixes_matches_hashing_the_range(
            bytes in vec(any::<u8>(), 0..600),
            start in any::<Index>(),
            end in any::<Index>(),
        ) {
            let (start, end) = (start.index(bytes.len() + 1), end.index(bytes.len() + 1));
            let (start, end) = (start.min(end), start.max(end));
            prop_assert_eq!(
                PrefixCrcs::new(&bytes, SALT).salted_crc(start, end),
                SALT.crc(&bytes[start..end]),
                "bytes {}..{} of {}", start, end, bytes.len()
            );
        }
    }

    #[test]
    fn header_like_note_text_never_hides_the_record_after_it() {
        let updates = UpdateGen::new(5, 1).updates(2);
        let (alpha, gamma) = (&updates[0][..], &updates[1][..]);
        let text = note(8, &"\0\0\u{1}\0".repeat(2048));
        let big = typed(70_000);
        let at = segment(&[alpha]).len();
        let damaged = |flips: &[(usize, u8)]| {
            let mut bytes = segment(&[alpha, &text, &big, gamma]);
            for &(offset, mask) in flips {
                bytes[at + offset] ^= mask;
            }
            bytes
        };
        for (name, flips) in [
            ("one CRC bit", &[(4, 1)][..]),
            ("a length bit and a CRC bit", &[(1, 1), (5, 0x10)]),
        ] {
            assert_eq!(
                read_log(&damaged(flips)),
                LogContents {
                    records: vec![alpha, &big, gamma],
                    end: LogEnd::Corrupt
                },
                "{name}"
            );
        }
    }

    #[test]
    fn a_frame_salted_inside_a_damaged_record_never_loads() {
        let updates = UpdateGen::new(5, 1).updates(3);
        let (alpha, ghost, gamma) = (&updates[0][..], &updates[1][..], &updates[2][..]);
        let host = carrying(8, &record(ghost));
        let at = segment(&[alpha]).len();
        assert_eq!(host[0], 1, "the host's first byte counts its one client");
        let damaged = |offset: usize, mask: u8| {
            let mut bytes = segment(&[alpha, &host, gamma]);
            bytes[at + offset] ^= mask;
            bytes
        };
        let host_lost: &[&[u8]] = &[alpha, gamma];
        let every_record: &[&[u8]] = &[alpha, &host, gamma];
        for (name, offset, mask, records) in [
            ("one CRC bit", 4, 1, host_lost),
            ("one length bit", 0, 0x40, every_record),
            ("the client count", 8, 0x40, host_lost),
        ] {
            assert_eq!(
                read_log(&damaged(offset, mask)),
                LogContents {
                    records: records.to_vec(),
                    end: LogEnd::Corrupt
                },
                "{name}"
            );
        }
    }

    #[test]
    fn a_frame_that_ends_its_host_never_loads_after_a_crc_flip() {
        let updates = UpdateGen::new(5, 1).updates(2);
        let (alpha, gamma) = (&updates[0][..], &updates[1][..]);
        let frame = record(&typed(5));
        let host = carrying(8, &frame[..frame.len() - 1]);
        assert!(
            host.ends_with(&frame),
            "the host's empty delete set ends the frame"
        );
        let mut bytes = segment(&[alpha, &host, gamma]);
        bytes[segment(&[alpha]).len() + 4] ^= 1;
        assert_eq!(
            read_log(&bytes),
            LogContents {
                records: vec![alpha, gamma],
                end: LogEnd::Corrupt
            }
        );
    }

    #[test]
    fn every_flipped_bit_in_a_record_loads_every_intact_record_and_no_planted_frame() {
        let updates = UpdateGen::new(5, 1).updates(6);
        let in_text = (1..400)
            .map(|n| record(&typed(n)))
            .find(|frame| frame.is_ascii())
            .unwrap();
        let in_text = format!("before {} after", std::str::from_utf8(&in_text).unwrap());
        let layouts = [
            (
                "salted frames in text and a buffer",
                vec![
                    updates[0].clone(),
                    note(8, &in_text),
                    updates[1].clone(),
                    carrying(9, &record(&typed(9))),
                    updates[2].clone(),
                ],
            ),
            (
                "header-like note text",
                vec![
                    updates[0].clone(),
                    note(8, &"\0\0\u{1}\0".repeat(64)),
                    note(9, &"y".repeat(300)),
                    updates[1].clone(),
                ],
            ),
            ("six plain records", updates),
        ];
        for (layout, records) in &layouts {
            let payloads: Vec<&[u8]> = records.iter().map(Vec::as_slice).collect();
            let bytes = segment(&payloads);
            let starts: Vec<usize> = (0..records.len())
                .map(|k| segment(&payloads[..k]).len())
                .collect();
            let last = records.len() - 1;
            for byte in SEGMENT_HEADER..bytes.len() {
                let damaged = starts.iter().rposition(|&s| s <= byte).unwrap();
                let in_length = byte < starts[damaged] + 4;
                let kept = payloads.iter().enumerate();
                let kept = kept.filter(|&(k, _)| in_length || k != damaged);
                let expected = LogContents {
                    records: kept.map(|(_, &p)| p).collect(),
                    end: if damaged == last && !in_length {
                        LogEnd::Torn
                    } else {
                        LogEnd::Corrupt
                    },
                };
                for bit in 0..8 {
                    let mut flipped = bytes.clone();
                    flipped[byte] ^= 1 << bit;
                    assert_eq!(
                        read_log(&flipped),
                        expected,
                        "{layout}: bit {bit} of byte {byte}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_recovered_length_never_stretches_over_an_intact_record() {
        let updates = UpdateGen::new(5, 1).updates(3);
        let (alpha, gamma) = (&updates[0][..], &updates[2][..]);
        let two = segment(&[alpha]).len();
        let mut bytes = segment(&[alpha, &updates[1], gamma]);
        let stretched_to_the_end = SALT.crc(&bytes[two + 8..]);
        bytes[two..two + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        bytes[two + 4..two + 8].copy_from_slice(&stretched_to_the_end.to_le_bytes());
        assert_eq!(
            read_log(&bytes),
            LogContents {
                records: vec![alpha, gamma],
                end: LogEnd::Corrupt
            }
        );
    }

    #[test]
    fn crafted_files_exhaust_length_recovery_and_every_intact_record_still_loads() {
        let updates = UpdateGen::new(5, 1).updates(403);
        let (alpha, beta, gamma) = (&updates[0][..], &updates[1][..], &updates[2][..]);
        let intact = &updates[3..];
        let then_a_damaged_length = |mut bytes: Vec<u8>| {
            let damaged = bytes.len();
            encode_record(SALT, beta, &mut bytes);
            bytes[damaged + 3] ^= 0x40;
            bytes
        };
        let headers_that_fit_no_record = |n: usize| {
            let mut bytes = segment_header(SALT);
            for update in &intact[..n] {
                bytes.extend([0xff, 0xff, 0xff, 0x7f, 0, 0, 0, 0]);
                encode_record(SALT, update, &mut bytes);
            }
            then_a_damaged_length(bytes)
        };
        let long = typed(4_000);
        let long_and_its_crc = [&long[..], &SALT.crc(&long).to_le_bytes()].concat();
        let crc_after_its_own_crc = SALT.crc(&long_and_its_crc).to_le_bytes();
        let mut planted = segment(&[alpha]);
        planted.extend(u32::MAX.to_le_bytes());
        planted.extend(crc_after_its_own_crc);
        planted.extend(&long_and_its_crc);
        planted.extend(crc_after_its_own_crc.repeat(2_000));
        planted.extend(record(gamma));
        let two: Vec<&[u8]> = vec![&intact[0], &intact[1], beta];
        let four_hundred: Vec<&[u8]> = intact.iter().map(Vec::as_slice).collect();
        for (name, bytes, records) in [
            (
                "after two headers that fit no record",
                headers_that_fit_no_record(2),
                two,
            ),
            (
                "after 400 headers that fit no record",
                headers_that_fit_no_record(400),
                four_hundred,
            ),
            (
                "after a payload that matches its CRC every 4 bytes",
                then_a_damaged_length(planted),
                vec![alpha, gamma],
            ),
        ] {
            assert_eq!(
                read_log(&bytes),
                LogContents {
                    records,
                    end: LogEnd::Corrupt
                },
                "{name}"
            );
        }
    }
}
