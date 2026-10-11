//! The durable, append-only store of Y.Docs. See `ARCHITECTURE.md` ("Storage").

mod clock;
mod doc_id;
mod equality;
mod format;
mod fs;
mod health;
mod layout;
mod log;
mod recover;
mod snapshot;
mod state;
mod store;
mod writer;

#[cfg(test)]
mod sim_fs;
#[cfg(test)]
mod testkit;
#[cfg(test)]
mod tests;

pub use doc_id::{DocId, InvalidDocId};
pub use health::FailReason;
pub use state::Seq;
pub use store::{AppendError, OpenError, Store};
