mod body;
mod records;
mod tree;

use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

use super::records::PageId;

pub(crate) fn page(id: &str) -> PageId {
    PageId::new(id).unwrap()
}

pub(crate) fn doc_of(update: &[u8]) -> Doc {
    let doc = Doc::new();
    doc.transact_mut()
        .apply_update(Update::decode_v1(update).unwrap())
        .unwrap();
    doc
}
