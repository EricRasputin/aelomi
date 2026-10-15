//! The durable, append-only store of Y.Docs. See `ARCHITECTURE.md`, "Data model and storage".

pub(crate) mod clock;
mod compact;
mod data_path;
mod decode;
mod doc_id;
mod equality;
mod format;
pub(crate) mod fs;
mod health;
mod layout;
mod log;
mod recover;
mod snapshot;
mod state;
mod store;
mod writer;

#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) mod sim_fs;
#[cfg(feature = "test-support")]
pub mod test_support;
#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(not(test), allow(dead_code))]
mod testkit;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use {
    clock::SystemClock,
    sim_fs::{Crash, OpKind, SimFs},
    state::Limits,
    store::Driver,
};

pub use data_path::{DataPathError, check_data_path};
pub use doc_id::{DocId, InvalidDocId};
pub use health::{FailReason, Health};
pub use state::Seq;
pub use store::{AppendError, OpenError, Store};
