use std::ops::Range;

use crc::{CRC_64_NVME, Crc, Table};

use super::{LogContents, LogEnd};

const ANCHOR: usize = 20;
const FIELDS: usize = 17;
static CRC_64: Crc<u64, Table<16>> = Crc::<u64, Table<16>>::new(&CRC_64_NVME);

const fn body_len(n: usize) -> usize {
    n + n.div_ceil(7)
}

fn is_high(byte: u8) -> bool {
    byte >= 0x80
}

#[derive(Clone, Copy)]
enum Kind {
    Head,
    Tail,
}

impl Kind {
    fn tag(self) -> u8 {
        match self {
            Self::Head => b'H',
            Self::Tail => b'T',
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Anchor {
    len: u32,
    check: u64,
}

impl Anchor {
    #[cfg(test)]
    fn of(update: &[u8]) -> Self {
        Self {
            len: u32::try_from(update.len()).expect("append rejects an update over 4 GiB"),
            check: CRC_64.checksum(update),
        }
    }

    fn encode(self, kind: Kind) -> [u8; ANCHOR] {
        let mut fields = [0; FIELDS];
        fields[0] = kind.tag();
        fields[1..5].copy_from_slice(&self.len.to_le_bytes());
        fields[5..13].copy_from_slice(&self.check.to_le_bytes());
        let crc = crc32fast::hash(&fields[..13]);
        fields[13..].copy_from_slice(&crc.to_le_bytes());
        let mut packed = Vec::with_capacity(ANCHOR);
        pack(&fields, &mut packed);
        let mut anchor = [0; ANCHOR];
        for (byte, low) in anchor.iter_mut().zip(packed) {
            *byte = low | 0x80;
        }
        anchor
    }

    fn parse(bytes: &[u8], kind: Kind) -> Option<Self> {
        if bytes.len() != ANCHOR || !bytes.iter().all(|&byte| is_high(byte)) {
            return None;
        }
        let mut low = [0; ANCHOR];
        for (low, byte) in low.iter_mut().zip(bytes) {
            *low = byte & 0x7f;
        }
        let (fields, canonical) = unpack(&low, FIELDS);
        let len = u32::from_le_bytes(fields[1..5].try_into().unwrap());
        let crc = u32::from_le_bytes(fields[13..].try_into().unwrap());
        let valid = canonical
            && fields[0] == kind.tag()
            && len > 0
            && crc32fast::hash(&fields[..13]) == crc;
        valid.then(|| Self {
            len,
            check: u64::from_le_bytes(fields[5..13].try_into().unwrap()),
        })
    }
}

fn pack(update: &[u8], out: &mut Vec<u8>) {
    for group in update.chunks(7) {
        let mut high = 0;
        for (i, &byte) in group.iter().enumerate() {
            out.push(byte & 0x7f);
            high |= (byte >> 7) << i;
        }
        out.push(high);
    }
}

fn unpack(body: &[u8], n: usize) -> (Vec<u8>, bool) {
    debug_assert_eq!(body.len(), body_len(n));
    let mut update = Vec::with_capacity(n);
    let mut canonical = true;
    for group in body.chunks(8) {
        let (low, high) = group.split_at(group.len() - 1);
        for (i, &byte) in low.iter().enumerate() {
            update.push(byte | (((high[0] >> i) & 1) << 7));
        }
        canonical &= high[0] >> low.len() == 0;
    }
    (update, canonical)
}

#[cfg(test)]
pub(super) fn encode(update: &[u8], out: &mut Vec<u8>) {
    let anchor = Anchor::of(update);
    out.extend_from_slice(&anchor.encode(Kind::Head));
    pack(update, out);
    out.extend_from_slice(&anchor.encode(Kind::Tail));
}

struct Found {
    body: Range<usize>,
    update: Vec<u8>,
    anchor: Anchor,
    head_intact: bool,
    tail_intact: bool,
    canonical: bool,
}

pub(super) fn read(bytes: &[u8]) -> LogContents {
    read_counting(bytes).0
}

fn read_counting(bytes: &[u8]) -> (LogContents, usize) {
    let mut found = Vec::new();
    let mut hashed = 0;
    let mut i = 0;
    while i < bytes.len() {
        if is_high(bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && !is_high(bytes[i]) {
            i += 1;
        }
        let run = start..i;
        let head = run
            .start
            .checked_sub(ANCHOR)
            .and_then(|at| Anchor::parse(&bytes[at..run.start], Kind::Head));
        let tail = bytes
            .get(run.end..run.end + ANCHOR)
            .and_then(|anchor| Anchor::parse(anchor, Kind::Tail));
        let mut verify = |body: Range<usize>, anchor: Anchor| {
            hashed += body.len();
            let (update, canonical) = unpack(&bytes[body], anchor.len as usize);
            (CRC_64.checksum(&update) == anchor.check).then_some((update, canonical))
        };
        let mut from_head = None;
        if let Some(head) = head {
            let body = run.start..run.start + body_len(head.len as usize);
            if body.end <= run.end
                && let Some((update, canonical)) = verify(body.clone(), head)
            {
                from_head = Some(body.clone());
                found.push(Found {
                    tail_intact: body.end == run.end && tail == Some(head),
                    body,
                    update,
                    anchor: head,
                    head_intact: true,
                    canonical,
                });
            }
        }
        if let Some(tail) = tail {
            let len = body_len(tail.len as usize);
            if len <= run.len() {
                let body = run.end - len..run.end;
                if from_head.as_ref() != Some(&body)
                    && let Some((update, canonical)) = verify(body.clone(), tail)
                {
                    found.push(Found {
                        head_intact: body.start == run.start && head == Some(tail),
                        body,
                        update,
                        anchor: tail,
                        tail_intact: true,
                        canonical,
                    });
                }
            }
        }
    }
    let end = classify(bytes, &found);
    let records = found.into_iter().map(|found| found.update).collect();
    (LogContents { records, end }, hashed)
}

fn classify(bytes: &[u8], found: &[Found]) -> LogEnd {
    let mut damaged = false;
    let mut torn = false;
    let mut pos = 0;
    for (k, frame) in found.iter().enumerate() {
        damaged |= frame.body.start.saturating_sub(ANCHOR) != pos;
        damaged |= !frame.head_intact || !frame.canonical;
        let tail_end = frame.body.end + ANCHOR;
        if !frame.tail_intact {
            let rest = without_trailing_zeros(&bytes[frame.body.end..]);
            let only_tear = k == found.len() - 1
                && rest.len() < ANCHOR
                && *rest == frame.anchor.encode(Kind::Tail)[..rest.len()];
            if only_tear {
                torn = true;
            } else {
                damaged = true;
            }
        }
        pos = pos.max(tail_end);
    }
    if pos < bytes.len() {
        if torn_shaped(&bytes[pos..]) {
            torn = true;
        } else {
            damaged = true;
        }
    }
    if damaged {
        LogEnd::Corrupt
    } else if torn {
        LogEnd::Torn
    } else {
        LogEnd::Clean
    }
}

fn torn_shaped(rest: &[u8]) -> bool {
    let rest = without_trailing_zeros(rest);
    if rest.len() < ANCHOR {
        return rest.iter().all(|&byte| is_high(byte));
    }
    let Some(head) = Anchor::parse(&rest[..ANCHOR], Kind::Head) else {
        return false;
    };
    let len = body_len(head.len as usize);
    let after = &rest[ANCHOR..];
    if after.len() >= len + ANCHOR {
        return false;
    }
    let (body, tail) = after.split_at(after.len().min(len));
    body.iter().all(|&byte| !is_high(byte)) && tail.iter().all(|&byte| is_high(byte))
}

fn without_trailing_zeros(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|&byte| byte != 0)
        .map_or(0, |i| i + 1);
    &bytes[..end]
}

#[cfg(test)]
mod tests {
    use proptest::collection::vec;
    use proptest::prelude::*;
    use proptest::sample::Index;
    use yrs::{Any, Doc, Map, Text, Transact};

    use super::*;
    use crate::storage::testkit::{SplitMix64, UpdateGen};

    fn frame(update: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        encode(update, &mut out);
        out
    }

    struct Log {
        bytes: Vec<u8>,
        updates: Vec<Vec<u8>>,
        frames: Vec<Range<usize>>,
    }

    impl Log {
        fn head(&self, k: usize) -> Range<usize> {
            self.frames[k].start..self.frames[k].start + ANCHOR
        }

        fn tail(&self, k: usize) -> Range<usize> {
            self.frames[k].end - ANCHOR..self.frames[k].end
        }
    }

    fn log(updates: Vec<Vec<u8>>) -> Log {
        let mut bytes = Vec::new();
        let mut frames = Vec::new();
        for update in &updates {
            let start = bytes.len();
            encode(update, &mut bytes);
            frames.push(start..bytes.len());
        }
        Log {
            bytes,
            updates,
            frames,
        }
    }

    fn tear_image(img: &[u8], orig: &[u8]) -> bool {
        let zeros_from = without_trailing_zeros(img).len();
        zeros_from < orig.len() && img[..zeros_from] == orig[..zeros_from]
    }

    fn end_of(img: &[u8], orig: &[u8]) -> LogEnd {
        if img == orig {
            LogEnd::Clean
        } else if tear_image(img, orig) {
            LogEnd::Torn
        } else {
            LogEnd::Corrupt
        }
    }

    #[derive(Debug)]
    enum Damage {
        Flip(usize, u8),
        Burst(usize, Vec<u8>),
        Zero(Range<usize>),
        SetHigh(Range<usize>),
    }

    impl Damage {
        fn apply(&self, bytes: &[u8]) -> Vec<u8> {
            let mut img = bytes.to_vec();
            match self {
                Self::Flip(at, bit) => img[*at] ^= 1 << bit,
                Self::Burst(at, with) => {
                    let end = (at + with.len()).min(img.len());
                    img[*at..end].copy_from_slice(&with[..end - at]);
                }
                Self::Zero(range) => img[range.clone()].fill(0),
                Self::SetHigh(range) => img[range.clone()].iter_mut().for_each(|b| *b |= 0x80),
            }
            img
        }
    }

    fn random_bytes(rng: &mut SplitMix64, len: usize) -> Vec<u8> {
        (0..len).map(|_| rng.next_u64() as u8).collect()
    }

    fn damages(log: &Log, rng: &mut SplitMix64) -> Vec<Damage> {
        let n = log.bytes.len();
        let mut out = Vec::new();
        for at in 0..n {
            for bit in 0..8 {
                out.push(Damage::Flip(at, bit));
            }
        }
        for k in 0..log.frames.len() {
            for anchor in [log.head(k), log.tail(k)] {
                for _ in 0..8 {
                    out.push(Damage::Burst(anchor.start, random_bytes(rng, ANCHOR)));
                    let len = 2 + rng.below(ANCHOR as u64 - 2) as usize;
                    let at = anchor.start + rng.below((ANCHOR - len + 1) as u64) as usize;
                    out.push(Damage::Burst(at, random_bytes(rng, len)));
                }
                out.push(Damage::Zero(anchor));
            }
        }
        for _ in 0..600 {
            let len = 2 + rng.below(40) as usize;
            let at = rng.below((n - len) as u64) as usize;
            out.push(Damage::Burst(at, random_bytes(rng, len)));
        }
        for len in [1, 4, 8, 20, 40, 64] {
            for at in 0..=n - len {
                out.push(Damage::Zero(at..at + len));
            }
        }
        for at in (0..n).step_by(512) {
            out.push(Damage::Zero(at..(at + 512).min(n)));
        }
        out
    }

    fn within_one_anchor(log: &Log, img: &[u8], k: usize) -> bool {
        let changed: Vec<usize> = log.frames[k]
            .clone()
            .filter(|&at| img[at] != log.bytes[at])
            .collect();
        let within = |anchor: Range<usize>| changed.iter().all(|at| anchor.contains(at));
        within(log.head(k)) || within(log.tail(k))
    }

    fn assert_read_after_one_damage(log: &Log, img: &[u8], damage: &str) {
        let (contents, hashed) = read_counting(img);
        assert!(
            hashed <= 2 * img.len(),
            "{damage}: hashed {hashed} of {}",
            img.len()
        );
        let mut appended = log.updates.iter();
        assert!(
            contents
                .records
                .iter()
                .all(|record| appended.any(|update| update == record)),
            "{damage}: a record that was not appended there"
        );
        for (k, update) in log.updates.iter().enumerate() {
            if within_one_anchor(log, img, k) {
                assert!(
                    contents.records.contains(update),
                    "{damage}: lost update {k}"
                );
            }
        }
        assert_eq!(contents.end, end_of(img, &log.bytes), "{damage}");
    }

    fn text_update(client: u64, text: &str) -> Vec<u8> {
        let doc = Doc::with_client_id(client);
        let t = doc.get_or_insert_text("t");
        let mut txn = doc.transact_mut();
        t.insert(&mut txn, 0, text);
        txn.encode_update_v1()
    }

    fn buffer_update(bytes: &[u8]) -> Vec<u8> {
        let doc = Doc::with_client_id(1_618_033_988_749);
        let m = doc.get_or_insert_map("m");
        let mut txn = doc.transact_mut();
        m.insert(&mut txn, "b", Any::Buffer(bytes.into()));
        txn.encode_update_v1()
    }

    fn ghost() -> Vec<u8> {
        text_update(424_242, "planted")
    }

    fn carrier(image: &[u8]) -> Vec<u8> {
        let mut packed = vec![b'n'; 13];
        packed.extend(image.iter().map(|b| b & 0x7f));
        while !packed.len().is_multiple_of(8) || packed.len() < 13 + image.len() + 8 {
            packed.push(b'x');
        }
        let (inner, _) = unpack(&packed, packed.len() / 8 * 7);
        (0..14)
            .find_map(|lead| {
                let content = [vec![b'n'; lead], inner.clone()].concat();
                let update = buffer_update(&content);
                let at = update.windows(content.len()).position(|w| w == content)?;
                (at + lead).is_multiple_of(7).then_some(update)
            })
            .expect("a lead that starts the image at a 7-byte group")
    }

    fn planted_log() -> Log {
        let keys = UpdateGen::new(11, 1).updates(3);
        let updates = vec![
            keys[0].clone(),
            keys[1].clone(),
            carrier(&frame(&ghost())),
            text_update(3_141_592_653_589, &"lorem ipsum ".repeat(25)),
            buffer_update(&[0xff; 64]),
            keys[2].clone(),
        ];
        let log = log(updates);
        assert_eq!(
            read(&log.bytes),
            LogContents {
                records: log.updates.clone(),
                end: LogEnd::Clean,
            }
        );
        log
    }

    fn hex(bytes: &[u8]) -> String {
        let pairs: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
        pairs.join(" ")
    }

    #[test]
    fn a_frame_is_forty_bytes_plus_one_per_seven_and_keeps_its_alphabets() {
        assert_eq!(
            hex(&frame(b"one")),
            concat!(
                "c8 83 80 80 80 b3 bb a0 cb aa 8a 9e c9 b1 d0 eb 9d ac a9 81 ",
                "6f 6e 65 00 ",
                "d4 83 80 80 80 b3 bb a0 cb aa 8a 9e c9 b1 f1 eb 82 91 87 83",
            )
        );
        assert_eq!(
            hex(&frame(&[0xff; 8])),
            concat!(
                "c8 88 80 80 80 ff ff e0 ff ff ff ff ff ff cf ff f2 90 aa 86 ",
                "7f 7f 7f 7f 7f 7f 7f 7f 7f 01 ",
                "d4 88 80 80 80 ff ff e0 ff ff ff ff ff ff ee ff ed ad 84 84",
            )
        );
        assert_eq!(CRC_64.checksum(b"123456789"), 0xae8b_1486_0a79_9888);
        for n in 1..=300 {
            let framed = frame(&vec![0xff; n]);
            assert_eq!(framed.len(), 40 + n + n.div_ceil(7), "{n}");
            let (head, rest) = framed.split_at(ANCHOR);
            let (body, tail) = rest.split_at(rest.len() - ANCHOR);
            assert!(head.iter().chain(tail).all(|&b| b >= 0x80), "{n}");
            assert!(body.iter().all(|&b| b < 0x80), "{n}");
        }
        assert_eq!(
            read(&frame(b"one")),
            LogContents {
                records: vec![b"one".to_vec()],
                end: LogEnd::Clean,
            }
        );
        assert_eq!(
            read(&[]),
            LogContents {
                records: vec![],
                end: LogEnd::Clean,
            }
        );
    }

    #[test]
    fn no_single_damage_loads_a_frame_planted_in_a_payload() {
        let log = planted_log();
        let ghost = ghost();
        let mut rng = SplitMix64::new(0x9e37_79b9_7f4a_7c15);
        for damage in damages(&log, &mut rng) {
            let img = damage.apply(&log.bytes);
            let damage = format!("{damage:?}");
            assert!(!read(&img).records.contains(&ghost), "{damage}: ghost");
            assert_read_after_one_damage(&log, &img, &damage);
        }
    }

    #[test]
    fn forcing_bit_7_on_twenty_author_chosen_anchor_bytes_can_load_a_planted_frame() {
        let log = planted_log();
        let ghost = ghost();
        let image = frame(&ghost);
        let body = log.head(2).end..log.tail(2).start;
        let low: Vec<u8> = image.iter().map(|b| b & 0x7f).collect();
        let offset = log.bytes[body.clone()]
            .windows(low.len())
            .position(|w| w == low)
            .expect("the carrier's body holds the image");
        let at = body.start + offset;
        let planted = at..at + image.len();
        let [k0, k1, _, paste, buffer, k2] = log.updates.clone().try_into().unwrap();
        let with_ghost = vec![
            k0.clone(),
            k1.clone(),
            ghost,
            paste.clone(),
            buffer.clone(),
            k2.clone(),
        ];
        let head = planted.start..planted.start + ANCHOR;
        let tail = planted.end - ANCHOR..planted.end;
        for (damages, records) in [
            (&[head.clone()][..], with_ghost.clone()),
            (&[tail.clone()][..], with_ghost.clone()),
            (&[head, tail], with_ghost),
            (&[planted], vec![k0, k1, paste, buffer, k2]),
        ] {
            let mut img = log.bytes.clone();
            for range in damages {
                img = Damage::SetHigh(range.clone()).apply(&img);
            }
            assert_eq!(
                read(&img),
                LogContents {
                    records,
                    end: LogEnd::Corrupt,
                },
                "{damages:?}"
            );
        }
    }

    #[test]
    fn a_one_record_log_survives_any_damage_to_its_first_twenty_bytes() {
        let alpha = UpdateGen::new(13, 1).next_update();
        let bytes = frame(&alpha);
        let mut damaged = Vec::new();
        for at in 0..ANCHOR {
            for bit in 0..8 {
                damaged.push(Damage::Flip(at, bit).apply(&bytes));
            }
        }
        let mut rng = SplitMix64::new(0x5eed);
        for len in 1..=ANCHOR {
            for _ in 0..32 {
                damaged.push(Damage::Burst(0, random_bytes(&mut rng, len)).apply(&bytes));
            }
            damaged.push(Damage::Zero(0..len).apply(&bytes));
        }
        let expected = LogContents {
            records: vec![alpha],
            end: LogEnd::Corrupt,
        };
        for img in damaged.iter().filter(|img| **img != bytes) {
            assert_eq!(read(img), expected, "{}", hex(&img[..ANCHOR]));
        }
    }

    #[test]
    fn damage_confined_to_one_anchor_never_loses_its_record() {
        let log = log(UpdateGen::new(17, 2).updates(4));
        let mut rng = SplitMix64::new(4);
        let mut damages = Vec::new();
        for k in 0..log.frames.len() {
            for anchor in [log.head(k), log.tail(k)] {
                for at in anchor.clone() {
                    for bit in 0..8 {
                        damages.push(Damage::Flip(at, bit));
                    }
                }
                for _ in 0..8 {
                    damages.push(Damage::Burst(anchor.start, random_bytes(&mut rng, ANCHOR)));
                    let len = 2 + rng.below(ANCHOR as u64 - 1) as usize;
                    let at = anchor.start + rng.below((ANCHOR - len + 1) as u64) as usize;
                    damages.push(Damage::Burst(at, random_bytes(&mut rng, len)));
                }
                damages.push(Damage::Zero(anchor));
            }
        }
        let across = log.tail(1).start..log.head(2).end;
        damages.push(Damage::Burst(
            across.start,
            random_bytes(&mut rng, across.len()),
        ));
        for damage in damages {
            let img = damage.apply(&log.bytes);
            let read = read(&img);
            assert_eq!(read.records, log.updates, "{damage:?}");
            assert_eq!(read.end, end_of(&img, &log.bytes), "{damage:?}");
        }
    }

    #[derive(Clone, Debug)]
    enum Fault {
        Flip(Index, u8),
        Burst(Index, Vec<u8>),
        Zero(Index, usize),
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            failure_persistence: None,
            ..ProptestConfig::default()
        })]

        #[test]
        fn every_accepted_record_passes_its_crc_64(
            seed in any::<u64>(),
            count in 1..=8usize,
            fault in prop_oneof![
                (any::<Index>(), 0..8u8).prop_map(|(at, bit)| Fault::Flip(at, bit)),
                (any::<Index>(), vec(any::<u8>(), 1..=64))
                    .prop_map(|(at, with)| Fault::Burst(at, with)),
                (any::<Index>(), 1..=64usize).prop_map(|(at, len)| Fault::Zero(at, len)),
            ],
        ) {
            let log = log(UpdateGen::new(seed, 3).updates(count));
            let n = log.bytes.len();
            let damage = match fault {
                Fault::Flip(at, bit) => Damage::Flip(at.index(n), bit),
                Fault::Burst(at, with) => Damage::Burst(at.index(n), with),
                Fault::Zero(at, len) => {
                    let at = at.index(n);
                    Damage::Zero(at..(at + len).min(n))
                }
            };
            let img = damage.apply(&log.bytes);
            assert_read_after_one_damage(&log, &img, &format!("{damage:?}"));
        }
    }

    #[test]
    fn a_crafted_log_hashes_at_most_two_bytes_per_byte() {
        let mut rng = SplitMix64::new(5);
        let mut crafted = Vec::new();
        while crafted.len() < 1_000_000 {
            let run = 1 + rng.below(4000) as usize;
            let anchor = Anchor {
                len: (run * 7 / 8).max(1) as u32,
                check: rng.next_u64(),
            };
            crafted.extend(anchor.encode(Kind::Head));
            crafted.extend(std::iter::repeat_n(b'z', body_len(anchor.len as usize)));
            crafted.extend(anchor.encode(Kind::Tail));
        }
        let (contents, hashed) = read_counting(&crafted);
        let nothing = LogContents {
            records: vec![],
            end: LogEnd::Corrupt,
        };
        assert_eq!(contents, nothing);
        assert!(
            crafted.len() < hashed && hashed <= 2 * crafted.len(),
            "each body is checked from both of its anchors: {hashed} of {}",
            crafted.len()
        );

        let mut dense = vec![b'q'; 1_000_000];
        for at in (0..dense.len()).step_by(41) {
            dense[at..(at + 20).min(1_000_000)].fill(0x80);
        }
        assert_eq!(read_counting(&dense), (nothing, 0));
    }

    #[test]
    fn every_cut_of_a_log_reads_clean_at_boundaries_and_torn_inside() {
        let log = planted_log();
        for cut in 0..=log.bytes.len() {
            let whole = log.frames.iter().filter(|f| f.end <= cut).count();
            let mut records = log.updates[..whole].to_vec();
            if whole < log.frames.len() && cut >= log.tail(whole).start {
                records.push(log.updates[whole].clone());
            }
            let end = if cut == 0 || log.frames.iter().any(|f| f.end == cut) {
                LogEnd::Clean
            } else {
                LogEnd::Torn
            };
            assert_eq!(
                read(&log.bytes[..cut]),
                LogContents { records, end },
                "cut {cut}"
            );
        }
    }

    fn final_updates() -> [Vec<u8>; 5] {
        [
            b"one".to_vec(),
            vec![0xff; 8],
            (0..300u32).map(|i| (i * 37) as u8).collect(),
            b"seven!!".to_vec(),
            vec![b'a'; 14],
        ]
    }

    #[test]
    fn a_complete_final_frame_with_damage_reads_corrupt_unless_its_image_is_a_tear() {
        let mut one = frame(b"one");
        let tail = one.len() - ANCHOR;
        one[tail] ^= 1;
        assert_eq!(
            read(&one),
            LogContents {
                records: vec![b"one".to_vec()],
                end: LogEnd::Corrupt,
            }
        );

        for last in final_updates() {
            let log = log(vec![b"lead update".to_vec(), last.clone()]);
            let start = log.frames[1].start;
            let tail = log.tail(1);
            let final_byte = tail.start - 1;
            let unused_bits = match last.len() % 7 {
                0 => 0..0,
                used => used..7,
            };
            let mut damages = Vec::new();
            for at in log.frames[1].clone() {
                for bit in 0..8 {
                    damages.push(Damage::Flip(at, bit));
                }
            }
            for j in 1..=ANCHOR {
                damages.push(Damage::Zero(tail.start..tail.start + j));
            }
            for j in 0..ANCHOR {
                damages.push(Damage::Zero(tail.start + j..tail.end));
            }
            for damage in damages {
                let in_body = match &damage {
                    Damage::Flip(at, bit) => {
                        (start + ANCHOR..tail.start).contains(at)
                            && !(*at == final_byte && unused_bits.contains(&(*bit as usize)))
                    }
                    _ => false,
                };
                let records = if in_body {
                    log.updates[..1].to_vec()
                } else {
                    log.updates.clone()
                };
                let img = damage.apply(&log.bytes);
                let end = end_of(&img, &log.bytes);
                assert_eq!(
                    read(&img),
                    LogContents { records, end },
                    "{} bytes, {damage:?}",
                    last.len()
                );
            }
        }
    }

    #[test]
    fn a_strict_prefix_of_a_final_frame_then_zeros_reads_torn() {
        let one = frame(b"one");
        assert_eq!(
            read(&[&one[..one.len() - 10], &[0; 100][..]].concat()),
            LogContents {
                records: vec![b"one".to_vec()],
                end: LogEnd::Torn,
            }
        );

        for last in final_updates() {
            let log = log(vec![b"lead update".to_vec(), last.clone()]);
            let start = log.frames[1].start;
            let body_end = log.tail(1).start - start;
            let framed = &log.bytes[start..];
            for cut in 1..framed.len() {
                let bare = &log.bytes[..start + cut];
                let zero_extended = [bare, &[0; 100][..]].concat();
                let completed =
                    cut >= ANCHOR && framed[cut.min(body_end)..body_end].iter().all(|&b| b == 0);
                for (img, holds_last) in [(bare, cut >= body_end), (&zero_extended, completed)] {
                    let records = if holds_last {
                        log.updates.clone()
                    } else {
                        log.updates[..1].to_vec()
                    };
                    assert_eq!(
                        read(img),
                        LogContents {
                            records,
                            end: LogEnd::Torn,
                        },
                        "{} bytes, cut {cut}, {} zeros after",
                        last.len(),
                        img.len() - bare.len()
                    );
                }
            }
        }
    }
}
