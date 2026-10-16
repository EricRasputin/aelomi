use std::time::Instant;

use yrs::updates::decoder::Decode;
use yrs::{Doc, ReadTxn, Transact, Update};

use super::body::{BlockId, paragraphs_update};
use super::records::{MAX_SAFE_INTEGER, PageId, PageRecord, Placement};
use super::{PageEditError, PreparedWorkspace, write};
use crate::storage::DocId;

pub const GETTING_STARTED_LINES: [&str; 3] = [
    "⌘K to search and run commands",
    "/ to insert a block",
    "[[ to link a page",
];

impl PreparedWorkspace {
    /// Durably initializes an empty workspace that first-run seeding will skip on later launches.
    pub fn initialize_empty(&mut self, deadline: Instant) -> Result<(), PageEditError> {
        let doc = Doc::new();
        let update = doc.transact_mut().encode_update_v1();
        self.store.append(&DocId::workspace(), &update)?;
        if !self.store.flush(deadline).is_empty() {
            return Err(PageEditError::Io(std::io::Error::other(
                "could not finish creating the empty workspace",
            )));
        }
        Ok(())
    }

    /// Seeds a pristine workspace before index readers start. Failure leaves it open and unseeded.
    pub fn seed_getting_started(&mut self, deadline: Instant) {
        if let Err(error) = self.try_seed(deadline) {
            eprintln!("aelomi seed deferred: {error}");
        }
    }

    fn try_seed(&self, deadline: Instant) -> Result<(), PageEditError> {
        if !self.store.is_pristine(&DocId::workspace())? {
            return Ok(());
        }
        let page = PageId::new("getting-started").expect("a fixed page ID");
        let doc = Doc::new();
        doc.transact_mut()
            .apply_update(
                Update::decode_v1(&self.store.load(page.doc_id())?)
                    .map_err(|_| PageEditError::InvalidUpdate)?,
            )
            .map_err(|_| PageEditError::InvalidUpdate)?;
        if doc.transact().state_vector().is_empty() {
            let paragraphs: Vec<_> = GETTING_STARTED_LINES
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    (
                        BlockId::new(format!("getting-started-{i}")).expect("nonempty ID"),
                        *line,
                    )
                })
                .collect();
            self.store
                .append(page.doc_id(), &paragraphs_update(&paragraphs))?;
        }
        if !self.store.flush(deadline).is_empty() {
            return Err(PageEditError::Io(std::io::Error::other(
                "could not flush the Getting started body",
            )));
        }
        let now = self.store.unix_ms();
        if now > MAX_SAFE_INTEGER {
            return Err(PageEditError::InvalidTime);
        }
        let records = Doc::with_client_id(self.record_client.get());
        let update = {
            let mut txn = records.transact_mut();
            write::put_page(
                &mut txn,
                &page,
                None,
                &PageRecord {
                    title: "Getting started".into(),
                    placement: Placement {
                        parent: None,
                        order: super::edit::ORDER_STEP,
                    },
                    favorite: None,
                    trash: None,
                    created_at: now,
                    updated_at: now,
                },
            );
            txn.encode_update_v1()
        };
        self.store.append(&DocId::workspace(), &update)?;
        Ok(())
    }
}
