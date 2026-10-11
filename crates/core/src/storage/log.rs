use std::cmp::Ordering;

use yrs::Update;
use yrs::updates::decoder::Decode;

pub(crate) const LOG_MAGIC: [u8; 8] = *b"AELOG\0\0\x01";
const RECORD_HEADER: usize = 8;

pub(crate) fn encode_record(payload: &[u8], out: &mut Vec<u8>) {
    let len = u32::try_from(payload.len()).expect("append rejects an update over 4 GiB");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
    out.extend_from_slice(payload);
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

/// Reads every record it can. After a bad record it resumes at the next record whose CRC matches
/// and whose payload decodes as an update. Only the first bad record decides torn or corrupt.
pub(crate) fn read_log(bytes: &[u8]) -> LogContents<'_> {
    read_log_hashing(bytes, bytes.len().saturating_mul(RESYNC_HASHED_PER_BYTE))
}

/// Resyncing hashes at most this many bytes per byte of the file, so a file where most offsets
/// read as long records still takes linear time. `measure_resync` times it.
const RESYNC_HASHED_PER_BYTE: usize = 1024;

pub(crate) fn read_log_hashing(bytes: &[u8], mut budget: usize) -> LogContents<'_> {
    if bytes.len() < LOG_MAGIC.len() {
        let end = if LOG_MAGIC.starts_with(bytes) {
            LogEnd::Torn
        } else {
            LogEnd::Corrupt
        };
        return LogContents {
            records: Vec::new(),
            end,
        };
    }
    if bytes[..LOG_MAGIC.len()] != LOG_MAGIC {
        return LogContents {
            records: Vec::new(),
            end: LogEnd::Corrupt,
        };
    }
    let mut contents = LogContents {
        records: Vec::new(),
        end: LogEnd::Clean,
    };
    let mut at = LOG_MAGIC.len();
    while at < bytes.len() {
        if let Some(payload) = record_at(bytes, at) {
            contents.records.push(payload);
            at += RECORD_HEADER + payload.len();
            continue;
        }
        let damage = match contents.end {
            LogEnd::Clean => first_damage(bytes, at),
            LogEnd::Torn | LogEnd::Corrupt => Damage::Corrupt,
        };
        if damage == Damage::Torn {
            contents.end = LogEnd::Torn;
            break;
        }
        let scan = past_own_length(bytes, at, &mut budget)
            .map_or_else(|| resync(bytes, at + 1, &mut budget), Scan::ResumeAt);
        contents.end = match (damage, scan) {
            (Damage::TornUnlessARecordFollows, Scan::NoRecord) => LogEnd::Torn,
            _ => LogEnd::Corrupt,
        };
        let Scan::ResumeAt(next) = scan else {
            break;
        };
        at = next;
    }
    contents
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Damage {
    Torn,
    TornUnlessARecordFollows,
    Corrupt,
}

fn first_damage(bytes: &[u8], at: usize) -> Damage {
    let rest = &bytes[at..];
    let Some(header) = rest.get(..RECORD_HEADER) else {
        return Damage::Torn;
    };
    let len = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
    match (RECORD_HEADER + len).cmp(&rest.len()) {
        Ordering::Equal => Damage::Torn,
        Ordering::Greater => Damage::TornUnlessARecordFollows,
        Ordering::Less if rest.iter().all(|&b| b == 0) => Damage::Torn,
        Ordering::Less => Damage::Corrupt,
    }
}

#[derive(Clone, Copy)]
enum Scan {
    ResumeAt(usize),
    NoRecord,
    /// Records follow but none decodes as an update, or some were too long to hash.
    NoUpdate,
}

/// Damage inside a payload leaves its length intact, so the next record starts right after it.
fn past_own_length(bytes: &[u8], at: usize, budget: &mut usize) -> Option<usize> {
    let (_, damaged) = candidate_at(bytes, at)?;
    let next = at + RECORD_HEADER + damaged.len();
    let (crc, payload) = candidate_at(bytes, next)?;
    *budget = budget.checked_sub(payload.len())?;
    (crc32fast::hash(payload) == crc && Update::decode_v1(payload).is_ok()).then_some(next)
}

fn resync(bytes: &[u8], from: usize, budget: &mut usize) -> Scan {
    let mut scan = Scan::NoRecord;
    for at in from..bytes.len() {
        let Some((crc, payload)) = candidate_at(bytes, at) else {
            continue;
        };
        let Some(left) = budget.checked_sub(payload.len()) else {
            scan = Scan::NoUpdate;
            continue;
        };
        *budget = left;
        if crc32fast::hash(payload) != crc {
            continue;
        }
        if Update::decode_v1(payload).is_ok() {
            return Scan::ResumeAt(at);
        }
        scan = Scan::NoUpdate;
    }
    scan
}

fn record_at(bytes: &[u8], at: usize) -> Option<&[u8]> {
    let (crc, payload) = candidate_at(bytes, at)?;
    (crc32fast::hash(payload) == crc).then_some(payload)
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

    fn segment(payloads: &[&[u8]]) -> Vec<u8> {
        let mut out = LOG_MAGIC.to_vec();
        for p in payloads {
            encode_record(p, &mut out);
        }
        out
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
        assert_eq!(bytes.len(), 8 + (8 + 3) + (8 + 7) + (8 + 300));
        assert_eq!(&bytes[8..16], &[3, 0, 0, 0, 241, 134, 108, 122]);
    }

    #[test]
    fn classifies_damaged_segments_and_reads_every_update_around_the_damage() {
        let updates = UpdateGen::new(5, 1).updates(3);
        let (alpha, beta, gamma) = (&updates[0][..], &updates[1][..], &updates[2][..]);
        let good = segment(&[alpha, beta]);
        let two = segment(&[alpha]).len();
        let third = segment(&[gamma])[8..].to_vec();
        let not_an_update = segment(&[b"not an update"])[8..].to_vec();
        let with = |tail: &[u8]| [good.as_slice(), tail].concat();
        let flip = |mut bytes: Vec<u8>, at: usize| {
            bytes[at] ^= 0x40;
            bytes
        };
        let typed = |n: usize| {
            let client = Doc::with_client_id(7);
            let text = client.get_or_insert_text("t");
            let mut txn = client.transact_mut();
            text.insert(&mut txn, 0, &"x".repeat(n));
            txn.encode_update_v1()
        };
        let sixty_four = (0..64).map(typed).find(|u| 8 + u.len() == 64).unwrap();
        assert_eq!(
            beta.len() & 0x40,
            0,
            "a flipped 0x40 adds 64 to beta's length"
        );
        let skips_one = flip(
            [good.as_slice(), &segment(&[&sixty_four])[8..], &third].concat(),
            two,
        );
        let a: &[&[u8]] = &[alpha];
        let ab: &[&[u8]] = &[alpha, beta];
        let ac: &[&[u8]] = &[alpha, gamma];
        let adc: &[&[u8]] = &[alpha, &sixty_four, gamma];
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
            ("magic only", LOG_MAGIC.to_vec(), none, LogEnd::Clean),
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
                ac,
                LogEnd::Corrupt,
            ),
            (
                "a length that lands on the record after next",
                skips_one,
                adc,
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
        let absurd_length = |tail: &[u8]| {
            let mut bytes = [segment(&[alpha, beta]).as_slice(), tail].concat();
            bytes[two + 3] ^= 0x40;
            bytes
        };
        let then_gamma = absurd_length(&segment(&[gamma])[8..]);
        let then_junk = absurd_length(&[1, 0, 0, 0, 0, 0, 0, 0, 0xab]);
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
}
