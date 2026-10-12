use std::collections::BTreeSet;

use proptest::prelude::*;
use proptest::sample::Index;
use yrs::{Any, Doc, Map, Text, Transact};

use super::{Env, applied, doc, ops_of_updates, record_at, soon, typed, without};
use crate::storage::equality::Ops;
use crate::storage::log::LOG_MAGIC;
use crate::storage::testkit::UpdateGen;

fn load_after(updates: &[Vec<u8>], damage: impl FnOnce(&mut [u8])) -> Ops {
    let env = Env::manual();
    let a = doc("a");
    let store = env.open();
    for update in updates {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    drop(store);
    let path = env.path("docs/a/log-0000000000-0000000000.ylog");
    let mut bytes = env.fs.files()[&path].clone();
    damage(&mut bytes);
    env.fs.put_durable(&path, &bytes);
    let (held, _) = applied(&env.open().load(&a).unwrap(), "reopened");
    held
}

fn flip_length(bytes: &mut [u8], record: usize, mask: usize) {
    let mask = u32::try_from(mask).unwrap().to_le_bytes();
    for (byte, mask) in bytes[record..record + 4].iter_mut().zip(mask) {
        *byte ^= mask;
    }
}

struct LengthFlip {
    records: Vec<Vec<u8>>,
    mask: usize,
}

fn five_where_one_length_bit_reaches_eof(damaged: usize) -> LengthFlip {
    let four: Vec<Vec<u8>> = (1..=4).map(|client| typed(client, 4)).collect();
    (1..400)
        .map(|n| [&four[..], &[typed(5, n)]].concat())
        .find_map(|records| {
            let bytes_after: usize = records[damaged + 1..].iter().map(|u| 8 + u.len()).sum();
            let flipping_reaches_eof =
                bytes_after.is_power_of_two() && records[damaged].len() & bytes_after == 0;
            flipping_reaches_eof.then_some(LengthFlip {
                records,
                mask: bytes_after,
            })
        })
        .unwrap()
}

#[test]
fn header_salt_damage_preserves_every_saved_operation() {
    let updates: Vec<Vec<u8>> = (1..=4).map(|client| typed(client, 4)).collect();
    let held = load_after(&updates, |bytes| bytes[10] ^= 1);
    assert_eq!(held, ops_of_updates(&updates));
}

#[test]
fn a_flipped_length_bit_loads_every_record() {
    let updates: Vec<Vec<u8>> = (1..=4).map(|client| typed(client, 4)).collect();
    let held = load_after(&updates, |b| flip_length(b, record_at(&updates, 2), 0x40));
    assert_eq!(held, ops_of_updates(&updates));
}

#[test]
fn a_length_flip_that_reaches_eof_loads_its_own_record() {
    let first = typed(1, 4);
    let second = (1..100)
        .map(|n| typed(2, n))
        .find(|u| 8 + u.len() == 64)
        .unwrap();
    assert_eq!(first.len() & 64, 0, "the flip adds 64 to the first length");
    let updates = [first, second];
    let held = load_after(&updates, |bytes| {
        flip_length(bytes, record_at(&updates, 0), 64);
    });
    assert_eq!(held, ops_of_updates(&updates));
}

#[test]
fn a_length_flip_loads_its_own_record_wherever_the_new_length_ends() {
    let past_eof = LengthFlip {
        records: (1..=5).map(|client| typed(client, 4)).collect(),
        mask: 1 << 30,
    };
    for (case, damaged, flip) in [
        (
            "the middle of five, to EOF",
            2,
            five_where_one_length_bit_reaches_eof(2),
        ),
        (
            "the first of five, to EOF",
            0,
            five_where_one_length_bit_reaches_eof(0),
        ),
        ("the second of five, far past EOF", 1, past_eof),
    ] {
        let held = load_after(&flip.records, |bytes| {
            flip_length(bytes, record_at(&flip.records, damaged), flip.mask);
        });
        assert_eq!(held, ops_of_updates(&flip.records), "{case}");
    }
}

fn unsalted_frame(payload: &[u8]) -> Vec<u8> {
    let len = u32::try_from(payload.len()).unwrap().to_le_bytes();
    [&len[..], &crc32fast::hash(payload).to_le_bytes(), payload].concat()
}

const NEVER_WROTE: u64 = 7;

fn note_holding_a_frame(writer: u64) -> Vec<u8> {
    let frame = (1..200)
        .map(|n| unsalted_frame(&typed(NEVER_WROTE, n)))
        .find(|frame| frame.is_ascii())
        .unwrap();
    let client = Doc::with_client_id(writer);
    let text = client.get_or_insert_text("t");
    let mut txn = client.transact_mut();
    text.insert(&mut txn, 0, std::str::from_utf8(&frame).unwrap());
    txn.encode_update_v1()
}

#[test]
fn a_record_written_inside_note_text_never_loads_after_damage() {
    let updates = [note_holding_a_frame(300), typed(400, 3)];
    let held = load_after(&updates, |bytes| bytes[record_at(&updates, 0) + 4] ^= 1);
    assert_eq!(held, ops_of_updates(&updates[1..]));
}

#[test]
fn a_frame_the_unsalted_format_accepts_never_loads_after_damage_whatever_the_salt() {
    let mut salts = BTreeSet::new();
    for writer in 0..64 {
        let updates = [
            note_holding_a_frame(1_000 + writer),
            typed(2_000 + writer, 3),
        ];
        let held = load_after(&updates, |bytes| {
            salts.insert(bytes[LOG_MAGIC.len()..record_at(&[], 0)].to_vec());
            bytes[record_at(&updates, 0) + 4] ^= 1;
        });
        assert_eq!(held, ops_of_updates(&updates[1..]), "writer {writer}");
    }
    assert_eq!(salts.len(), 64, "each segment has its own salt");
}

fn carrying(writer: u64, frame: Vec<u8>) -> Vec<u8> {
    let client = Doc::with_client_id(writer);
    let blobs = client.get_or_insert_map("blobs");
    let mut txn = client.transact_mut();
    blobs.insert(&mut txn, writer.to_string(), Any::Buffer(frame.into()));
    txn.encode_update_v1()
}

#[test]
fn a_frame_salted_by_an_author_who_read_the_segment_never_loads_after_damage() {
    let env = Env::manual();
    let a = doc("a");
    let store = env.open();
    let first = typed(1, 4);
    store.append(&a, &first).unwrap();
    assert_eq!(store.flush(soon()), []);
    let path = env.path("docs/a/log-0000000000-0000000000.ylog");
    let salt = env.fs.files()[&path][LOG_MAGIC.len()..record_at(&[], 0)].to_vec();
    let ghost = typed(NEVER_WROTE, 3);
    let len = u32::try_from(ghost.len()).unwrap().to_le_bytes();
    let crc = crc32fast::hash(&[&salt[..], &ghost].concat()).to_le_bytes();
    let updates = [
        first,
        carrying(300, [&len[..], &crc, &ghost].concat()),
        typed(400, 3),
    ];
    for update in &updates[1..] {
        store.append(&a, update).unwrap();
    }
    assert_eq!(store.flush(soon()), []);
    drop(store);
    env.fs.flip_bit(&path, record_at(&updates, 1) + 4);
    let (held, _) = applied(&env.open().load(&a).unwrap(), "reopened");
    assert_eq!(held, ops_of_updates(&without(&updates, &[1])));
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn a_flipped_bit_anywhere_in_a_segment_loads_nothing_that_was_never_appended(
        seed in 0..1_000_000u64,
        at in any::<Index>(),
        bit in 0..8u8,
    ) {
        let ghosts = UpdateGen::new(seed, 2).updates(4);
        let updates: Vec<Vec<u8>> = (0..4)
            .map(|k| carrying(900_000_000 + k as u64, unsalted_frame(&ghosts[k])))
            .collect();
        let mut flipped = 0;
        let held = load_after(&updates, |bytes| {
            flipped = at.index(bytes.len());
            bytes[flipped] ^= 1 << bit;
        });
        let damaged = (0..updates.len())
            .find(|&k| (record_at(&updates, k)..record_at(&updates, k + 1)).contains(&flipped));
        let in_length = |k| (record_at(&updates, k)..record_at(&updates, k) + 4).contains(&flipped);
        let expected = match damaged {
            Some(k) if in_length(k) => ops_of_updates(&updates),
            Some(k) => ops_of_updates(&without(&updates, &[k])),
            None => Ops::default(),
        };
        prop_assert_eq!(held, expected, "bit {} of byte {}", bit, flipped);
    }
}
