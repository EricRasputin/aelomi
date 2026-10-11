//! Shared test helpers: a seeded random number generator and random Yjs updates.

use yrs::updates::decoder::Decode;
use yrs::{
    Doc, Map, MapRef, ReadTxn, StateVector, Text, TextRef, Transact, Update, XmlElementPrelim,
    XmlFragment, XmlFragmentRef, XmlOut, XmlTextPrelim,
};

use super::equality::Fingerprint;

/// SplitMix64. Small, fast, and the same sequence on every platform.
pub(crate) struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..bound`. `bound` must be at least 1.
    pub fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

/// Random Yjs updates from several simulated clients editing a `content` XML fragment, a
/// `text` Text, and a `meta` map. The seed fixes every client's ID, so output is reproducible.
pub(crate) struct UpdateGen {
    rng: Rng,
    clients: Vec<Client>,
}

struct Client {
    doc: Doc,
    content: XmlFragmentRef,
    text: TextRef,
    meta: MapRef,
}

impl UpdateGen {
    pub fn new(seed: u64, clients: u64) -> Self {
        let first_id = (seed % 1_000_000) * 16 + 1;
        let clients = (first_id..first_id + clients)
            .map(|id| {
                let doc = Doc::with_client_id(id);
                let content = doc.get_or_insert_xml_fragment("content");
                let text = doc.get_or_insert_text("text");
                let meta = doc.get_or_insert_map("meta");
                Client {
                    doc,
                    content,
                    text,
                    meta,
                }
            })
            .collect();
        Self {
            rng: Rng::new(seed),
            clients,
        }
    }

    /// One non-empty update from a random client. Now and then that client first catches up
    /// with the others, so later edits depend on other clients' items.
    pub fn next_update(&mut self) -> Vec<u8> {
        loop {
            let at = self.rng.below(self.clients.len() as u64) as usize;
            if self.rng.below(8) == 0 {
                self.catch_up(at);
            }
            let update = self.edit(at);
            if Update::decode_v1(&update).is_ok_and(|u| !u.is_empty()) {
                return update;
            }
        }
    }

    pub fn updates(&mut self, n: usize) -> Vec<Vec<u8>> {
        (0..n).map(|_| self.next_update()).collect()
    }

    fn catch_up(&mut self, at: usize) {
        let sv = self.clients[at].doc.transact().state_vector();
        for i in 0..self.clients.len() {
            if i != at {
                let diff = self.clients[i]
                    .doc
                    .transact()
                    .encode_state_as_update_v1(&sv);
                apply(&self.clients[at].doc, &diff);
            }
        }
    }

    fn edit(&mut self, at: usize) -> Vec<u8> {
        let rng = &mut self.rng;
        let c = &self.clients[at];
        let mut txn = c.doc.transact_mut();
        match rng.below(6) {
            0 => {
                let len = c.text.len(&txn);
                let index = rng.below(len as u64 + 1) as u32;
                c.text.insert(&mut txn, index, &word(rng));
            }
            1 => {
                let len = c.text.len(&txn) as u64;
                if len > 0 {
                    let index = rng.below(len);
                    let n = 1 + rng.below((len - index).min(5));
                    c.text.remove_range(&mut txn, index as u32, n as u32);
                }
            }
            2 => {
                let index = rng.below(c.content.len(&txn) as u64 + 1) as u32;
                let paragraph = c
                    .content
                    .insert(&mut txn, index, XmlElementPrelim::empty("p"));
                paragraph.push_back(&mut txn, XmlTextPrelim::new(word(rng)));
            }
            3 => {
                let len = c.content.len(&txn) as u64;
                if len > 0 {
                    let index = rng.below(len) as u32;
                    if let Some(XmlOut::Element(p)) = c.content.get(&txn, index)
                        && let Some(XmlOut::Text(t)) = p.get(&txn, 0)
                    {
                        let len = t.len(&txn) as u64;
                        if len > 0 && rng.below(2) == 0 {
                            t.remove_range(&mut txn, rng.below(len) as u32, 1);
                        } else {
                            t.insert(&mut txn, rng.below(len + 1) as u32, &word(rng));
                        }
                    }
                }
            }
            4 => {
                let key = format!("k{}", rng.below(5));
                if rng.below(2) == 0 {
                    c.meta.insert(&mut txn, key, word(rng));
                } else {
                    c.meta.insert(&mut txn, key, rng.below(1000) as i64);
                }
            }
            _ => {
                c.meta.remove(&mut txn, &format!("k{}", rng.below(5)));
            }
        }
        txn.encode_update_v1()
    }
}

fn word(rng: &mut Rng) -> String {
    (0..1 + rng.below(6))
        .map(|_| (b'a' + rng.below(26) as u8) as char)
        .collect()
}

pub(crate) fn apply(doc: &Doc, update: &[u8]) {
    let update = Update::decode_v1(update).expect("a valid update");
    doc.transact_mut()
        .apply_update(update)
        .expect("update applies");
}

/// A doc holding every update in `updates`.
pub(crate) fn doc_from<'a>(updates: impl IntoIterator<Item = &'a [u8]>) -> Doc {
    let doc = Doc::new();
    for update in updates {
        apply(&doc, update);
    }
    doc
}

pub(crate) fn fingerprint(doc: &Doc) -> Fingerprint {
    Fingerprint::of(&doc.transact())
}

pub(crate) fn state(doc: &Doc) -> Vec<u8> {
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}
