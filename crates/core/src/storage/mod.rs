//! The durable, append-only store of Y.Docs. See `ARCHITECTURE.md` ("Storage").

mod clock;
mod doc_id;
mod format;
mod fs;
mod layout;
mod log;
mod snapshot;

#[cfg(test)]
mod sim_fs;
#[cfg(test)]
mod testkit;

pub use doc_id::{DocId, InvalidDocId};
