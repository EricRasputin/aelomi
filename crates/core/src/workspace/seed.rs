use std::time::{Duration, Instant};

use yrs::updates::decoder::Decode;
use yrs::{Doc, ReadTxn, Transact, Update};

use super::body::{BlockId, paragraphs_update};
use super::records::PageId;
use super::{PageEdit, PageEditError, Workspace};
use crate::storage::DocId;

pub const GETTING_STARTED_LINES: [&str; 3] = [
    "⌘K to search and run commands",
    "/ to insert a block",
    "[[ to link a page",
];

impl Workspace {
    pub(crate) fn seed_if_pristine(&self) -> Result<(), PageEditError> {
        let _held = self
            .record_edits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !self.store.is_pristine(&DocId::workspace())? {
            return Ok(());
        }
        let page = PageId::new("getting-started").expect("a fixed page ID");
        let doc = Doc::new();
        doc.transact_mut()
            .apply_update(
                Update::decode_v1(&self.load(page.doc_id())?)
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
            self.append(page.doc_id(), &paragraphs_update(&paragraphs))?;
        }
        // Publish the page record only after its body survives a crash. A retry reuses that body.
        if !self
            .flush(Instant::now() + Duration::from_secs(2))
            .is_empty()
        {
            return Err(PageEditError::Io(std::io::Error::other(
                "could not flush the Getting started body",
            )));
        }
        self.edit_page_locked(
            PageEdit::Create {
                parent: None,
                title: "Getting started".into(),
            },
            Some(page),
        )?;
        Ok(())
    }
}
