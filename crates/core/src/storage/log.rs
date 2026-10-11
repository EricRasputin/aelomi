use std::cmp::Ordering;

pub(crate) const LOG_MAGIC: [u8; 8] = *b"AELOG\0\0\x01";
const RECORD_HEADER: usize = 8;

pub(crate) fn encode_record(payload: &[u8], out: &mut Vec<u8>) {
    let len = u32::try_from(payload.len()).expect("a Yjs update is under 4 GiB");
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

pub(crate) fn read_log(bytes: &[u8]) -> LogContents<'_> {
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
    let mut records = Vec::new();
    let mut at = LOG_MAGIC.len();
    while at < bytes.len() {
        let Some(payload) = record_at(bytes, at) else {
            let end = if explained_by_torn_write(bytes, at) {
                LogEnd::Torn
            } else {
                LogEnd::Corrupt
            };
            return LogContents { records, end };
        };
        records.push(payload);
        at += RECORD_HEADER + payload.len();
    }
    LogContents {
        records,
        end: LogEnd::Clean,
    }
}

fn explained_by_torn_write(bytes: &[u8], at: usize) -> bool {
    let rest = &bytes[at..];
    let Some(header) = rest.get(..RECORD_HEADER) else {
        return true;
    };
    let len = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
    match (RECORD_HEADER + len).cmp(&rest.len()) {
        Ordering::Equal => true,
        Ordering::Greater => !(at + 1..bytes.len()).any(|q| record_at(bytes, q).is_some()),
        Ordering::Less => rest.iter().all(|&b| b == 0),
    }
}

/// The store never writes an empty payload, so a zero length is invalid. That keeps a
/// zero-filled tail from reading as records.
fn record_at(bytes: &[u8], at: usize) -> Option<&[u8]> {
    let header = bytes.get(at..at + RECORD_HEADER)?;
    let len = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(header[4..].try_into().unwrap());
    let start = at + RECORD_HEADER;
    let payload = bytes.get(start..start.checked_add(len)?)?;
    (len > 0 && crc32fast::hash(payload) == crc).then_some(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn classifies_damaged_segments() {
        let good = segment(&[b"alpha", b"beta"]);
        let two = segment(&[b"alpha"]).len();
        let third = segment(&[b"gamma"])[8..].to_vec();
        let with = |tail: &[u8]| [good.as_slice(), tail].concat();
        let flip = |mut bytes: Vec<u8>, at: usize| {
            bytes[at] ^= 0x40;
            bytes
        };
        let a: &[&[u8]] = &[b"alpha"];
        let ab: &[&[u8]] = &[b"alpha", b"beta"];
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
                a,
                LogEnd::Corrupt,
            ),
            (
                "absurd length, then a good record",
                flip(with(&third), two + 3),
                a,
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
}
