//! The durable, append-only store of Y.Docs. See `ARCHITECTURE.md` ("Storage").

mod doc_id;
mod format;
mod layout;
mod log;
mod snapshot;

pub use doc_id::{DocId, InvalidDocId};
