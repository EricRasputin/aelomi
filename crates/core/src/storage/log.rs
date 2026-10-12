use std::io;

use yrs::Update;
use yrs::encoding::read::Read;
use yrs::updates::decoder::{Decode, DecoderV1};

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
    read_log_hashing(bytes, bytes.len().saturating_mul(RESYNC_HASHED_PER_BYTE))
}

const RESYNC_HASHED_PER_BYTE: usize = 1024;
const RECOVERY_WORK_PER_BYTE: usize = 64;

pub(crate) fn read_log_hashing(bytes: &[u8], mut budget: usize) -> LogContents<'_> {
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
        let scan = resync(bytes, salt, at + 1, &mut budget);
        let torn = contents.end == LogEnd::Clean
            && a_torn_write_explains(bytes, at)
            && matches!(scan, Scan::NoRecord);
        contents.end = if torn { LogEnd::Torn } else { LogEnd::Corrupt };
        let Scan::ResumeAt(next) = scan else {
            break;
        };
        at = next;
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

fn resync(bytes: &[u8], salt: Salt, from: usize, budget: &mut usize) -> Scan {
    let mut scan = Scan::NoRecord;
    for at in from..bytes.len() {
        let Some((crc, payload)) = candidate_at(bytes, at) else {
            continue;
        };
        let Some(left) = budget.checked_sub(payload.len()) else {
            scan = Scan::OverBudget;
            continue;
        };
        *budget = left;
        if salt.crc(payload) != crc {
            continue;
        }
        if Update::decode_v1(payload).is_ok() {
            return Scan::ResumeAt(at);
        }
        scan = Scan::NoUpdate;
    }
    scan
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
        let next_passes =
            || record_at(bytes, end, salt).is_some_and(|next| Update::decode_v1(next).is_ok());
        if decodes_exactly(payload) && (end == bytes.len() || next_passes()) {
            return Some(payload);
        }
    }
    None
}

/// yrs stops reading at the end of an update, so a payload stretched over the records after it
/// still decodes.
fn decodes_exactly(payload: &[u8]) -> bool {
    let mut decoder = DecoderV1::from(payload);
    Update::decode(&mut decoder).is_ok() && decoder.read_u8().is_err()
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
    use yrs::{Doc, Text, Transact};

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
        let client = Doc::with_client_id(7);
        let text = client.get_or_insert_text("t");
        let mut txn = client.transact_mut();
        text.insert(&mut txn, 0, &"x".repeat(n));
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
    fn a_resync_skips_records_too_long_for_its_budget_and_never_calls_that_torn() {
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
            ("then junk, no budget", &then_junk, 0, a, LogEnd::Corrupt),
        ] {
            assert_eq!(
                read_log_hashing(bytes, budget),
                LogContents {
                    records: records.to_vec(),
                    end
                },
                "{name}"
            );
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
