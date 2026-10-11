use yrs::{Doc, Text, Transact};

use super::{Env, applied, doc, ops_of_updates, record_at, soon, without};
use crate::storage::equality::Ops;

fn typed(client: u64, n: usize) -> Vec<u8> {
    let client = Doc::with_client_id(client);
    let text = client.get_or_insert_text("t");
    let mut txn = client.transact_mut();
    text.insert(&mut txn, 0, &"x".repeat(n));
    txn.encode_update_v1()
}

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

/// The last record is sized so that flipping one bit of record `i`'s length makes it end
/// exactly at EOF. Returns the five records and the mask that flips that bit.
fn five_where_a_length_flip_reaches_eof(i: usize) -> (Vec<Vec<u8>>, usize) {
    let four: Vec<Vec<u8>> = (1..=4).map(|client| typed(client, 4)).collect();
    (1..400)
        .map(|n| [&four[..], &[typed(5, n)]].concat())
        .find_map(|five| {
            let after: usize = five[i + 1..].iter().map(|u| 8 + u.len()).sum();
            (after.is_power_of_two() && five[i].len() & after == 0).then_some((five, after))
        })
        .unwrap()
}

#[test]
fn a_length_flip_that_reaches_eof_loses_only_its_own_record() {
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
    assert_eq!(held, ops_of_updates(&updates[1..]));
}

#[test]
fn a_length_flip_loses_only_its_own_record_wherever_the_new_length_ends() {
    let (middle, to_eof_from_middle) = five_where_a_length_flip_reaches_eof(2);
    let (first, to_eof_from_first) = five_where_a_length_flip_reaches_eof(0);
    let past_eof: Vec<Vec<u8>> = (1..=5).map(|client| typed(client, 4)).collect();
    for (case, five, damaged, mask) in [
        ("the middle of five, to EOF", middle, 2, to_eof_from_middle),
        ("the first of five, to EOF", first, 0, to_eof_from_first),
        ("the second of five, far past EOF", past_eof, 1, 1 << 30),
    ] {
        let held = load_after(&five, |bytes| {
            flip_length(bytes, record_at(&five, damaged), mask);
        });
        assert_eq!(held, ops_of_updates(&without(&five, &[damaged])), "{case}");
    }
}
