//! The durable, append-only store of Y.Docs. See `ARCHITECTURE.md`, "Data model and storage".

pub(crate) mod clock;
mod compact;
mod data_path;
mod decode;
mod doc_id;
mod equality;
mod format;
mod fs;
mod health;
mod layout;
mod log;
mod recover;
mod settings;
mod snapshot;
mod state;
mod store;
mod writer;

#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(not(test), allow(dead_code))]
mod sim_fs;
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
pub use store::{AppendError, OpenError, Pending, Store};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspacePresence {
    NotArrived,
    Arrived,
    Unrecognized,
}

/// Inspects the marker and document store without preparing or changing the folder.
pub fn workspace_presence(path: &std::path::Path) -> std::io::Result<WorkspacePresence> {
    let marker = path.join("format.json").try_exists()?;
    let docs = path.join("docs").try_exists()?;
    Ok(match (marker, docs && path.join("docs").is_dir()) {
        (true, true) => WorkspacePresence::Arrived,
        (false, true) => WorkspacePresence::Unrecognized,
        _ => WorkspacePresence::NotArrived,
    })
}
