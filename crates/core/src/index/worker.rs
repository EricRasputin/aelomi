use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::PoisonError;
use std::time::{Duration, Instant};

use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

use super::db::{self, PageRow, RunError, Writer};
use super::{HoldPoint, IndexChange, IndexEvent, IndexStatus, Shared, Source};
use crate::storage::DocId;
use crate::workspace::body::{Block, read_body};
use crate::workspace::records::{PageId, RecordError, read_pages};
use crate::workspace::tree::forest;

const RETRY_FIRST: Duration = Duration::from_millis(100);
const RETRY_MAX: Duration = Duration::from_secs(30);
const PAGES_PER_TRANSACTION: usize = 128;

pub(super) fn run(shared: &Shared, source: &dyn Source, path: &Path) {
    shared.emit(&IndexEvent::Status(IndexStatus::Rebuilding));
    let mut retry = RETRY_FIRST;
    loop {
        let mut worker = Worker {
            shared,
            source,
            path,
            committed_rows: BTreeMap::new(),
            reconciled: false,
            failed: BTreeMap::new(),
            retry: &mut retry,
        };
        let reason = match panic::catch_unwind(AssertUnwindSafe(|| worker.run())) {
            Ok(Ok(())) => return,
            Ok(Err(fatal)) => fatal.to_string(),
            Err(_) => "the index worker panicked".to_owned(),
        };
        shared.lock_reader().take();
        shared.lock().workspace_in_flight = false;
        shared.publish(IndexStatus::Failed { reason });
        if !shared.sleep(retry) {
            return;
        }
        retry = (retry * 2).min(RETRY_MAX);
    }
}

struct Worker<'a> {
    shared: &'a Shared,
    source: &'a dyn Source,
    path: &'a Path,
    committed_rows: BTreeMap<PageId, PageRow>,
    reconciled: bool,
    failed: BTreeMap<DocId, String>,
    retry: &'a mut Duration,
}

enum Bodies {
    Due,
    All,
}

impl Worker<'_> {
    fn run(&mut self) -> Result<(), RunError> {
        let (mut db, cache) = Writer::open(self.path)?;
        self.shared.emit(&IndexEvent::Cache(cache));
        self.sync_workspace(&mut db, true)?;
        self.sync_bodies(&mut db, Bodies::All)?;
        self.sync_workspace(&mut db, false)?;
        self.sync_bodies(&mut db, Bodies::All)?;
        if self.shared.stopping() {
            return Ok(());
        }
        *self.shared.lock_reader() = Some(db::open_reader(self.path)?);
        self.publish();
        *self.retry = RETRY_FIRST;
        while self.shared.wait_for_work() {
            let workspace = self.sync_workspace(&mut db, false)?;
            let pages = self.sync_bodies(&mut db, Bodies::Due)?;
            self.publish();
            if workspace || !pages.is_empty() {
                self.shared
                    .emit(&IndexEvent::Changed(IndexChange { workspace, pages }));
            }
        }
        Ok(())
    }

    fn publish(&self) {
        self.shared.publish(match self.failed.iter().next() {
            None => IndexStatus::Ready,
            Some((doc, reason)) => IndexStatus::Failed {
                reason: format!("{doc}: {reason}"),
            },
        });
    }

    fn sync_workspace(&mut self, db: &mut Writer, force: bool) -> Result<bool, RunError> {
        if !self.shared.take_workspace(force) {
            return Ok(false);
        }
        let result = self.apply_workspace(db);
        self.shared.finish_workspace();
        result.map(|()| true)
    }

    fn apply_workspace(&mut self, db: &mut Writer) -> Result<(), RunError> {
        let workspace = DocId::workspace();
        let bytes = self
            .source
            .load(&workspace)
            .map_err(|e| RunError::Source(workspace.clone(), e))?;
        let records = match decode(&bytes)
            .map_err(|e| e.to_string())
            .and_then(|doc| read_pages(&doc.transact()).map_err(|e: RecordError| e.to_string()))
        {
            Ok(records) => records,
            Err(reason) => {
                self.failed.insert(workspace, reason);
                return Ok(());
            }
        };
        self.failed.remove(&workspace);
        let forest = forest(&records);
        let rows = db::page_rows(records, &forest);
        let reconcile = !self.reconciled;
        let old = if reconcile {
            BTreeMap::new()
        } else {
            std::mem::take(&mut self.committed_rows)
        };
        let tx = db.transaction()?;
        if reconcile {
            db::retain_pages(&tx, &rows.keys().cloned().collect())?;
        }
        let mut relabel = BTreeSet::new();
        for (id, row) in &rows {
            let previous = old.get(id);
            if previous != Some(row) {
                db::upsert_page(&tx, id, row)?;
            }
            if previous.map(|p| &p.record.title) != Some(&row.record.title) {
                relabel.insert(id.clone());
            }
        }
        for id in old.keys().filter(|id| !rows.contains_key(*id)) {
            db::delete_page(&tx, id.as_str())?;
            relabel.insert(id.clone());
        }
        if !reconcile {
            db::rerender_links_to(&tx, &relabel, &rows)?;
        }
        self.pass(HoldPoint::BeforeCommit);
        tx.commit()?;
        for id in old.keys().filter(|id| !rows.contains_key(*id)) {
            self.failed.remove(id.doc_id());
        }
        let added: Vec<DocId> = rows
            .keys()
            .filter(|id| reconcile || !old.contains_key(*id))
            .map(|id| id.doc_id().clone())
            .collect();
        self.committed_rows = rows;
        self.reconciled = true;
        self.shared.mark_due_now(&added);
        Ok(())
    }

    fn sync_bodies(&mut self, db: &mut Writer, which: Bodies) -> Result<Vec<PageId>, RunError> {
        let marked = self.shared.take_pages(matches!(which, Bodies::All));
        let mut replaced = Vec::new();
        for chunk in marked.chunks(PAGES_PER_TRANSACTION) {
            let mut projected: Vec<(PageId, Option<Vec<Block>>)> = Vec::new();
            for doc in chunk {
                if self.shared.stopping() {
                    return Ok(replaced);
                }
                let Ok(page) = PageId::new(doc.as_str()) else {
                    continue;
                };
                if !self.committed_rows.contains_key(&page) {
                    projected.push((page, None));
                    continue;
                }
                self.pass(HoldPoint::BeforeLoad(doc.clone()));
                let bytes = self
                    .source
                    .load(doc)
                    .map_err(|e| RunError::Source(doc.clone(), e))?;
                match decode(&bytes)
                    .map_err(|e| e.to_string())
                    .and_then(|ydoc| read_body(&ydoc.transact()).map_err(|e| e.to_string()))
                {
                    Ok(blocks) => {
                        self.failed.remove(doc);
                        projected.push((page, Some(blocks)));
                    }
                    Err(reason) => {
                        self.failed.insert(doc.clone(), reason);
                        projected.push((page, None));
                    }
                }
            }
            let tx = db.transaction()?;
            for (page, blocks) in &projected {
                match blocks {
                    Some(blocks) => db::sync_body(&tx, page, blocks, &self.committed_rows)?,
                    None if self.committed_rows.contains_key(page) => {
                        db::sync_body(&tx, page, &[], &self.committed_rows)?
                    }
                    None => db::delete_page(&tx, page.as_str())?,
                }
            }
            self.pass(HoldPoint::BeforeCommit);
            tx.commit()?;
            replaced.extend(projected.into_iter().map(|(page, _)| page));
        }
        replaced.sort();
        replaced.dedup();
        Ok(replaced)
    }

    fn pass(&self, point: HoldPoint) {
        self.shared.gate.pass(point);
    }
}

fn decode(bytes: &[u8]) -> Result<Doc, yrs::encoding::read::Error> {
    let doc = Doc::new();
    let update = Update::decode_v1(bytes)?;
    doc.transact_mut()
        .apply_update(update)
        .map_err(|_| yrs::encoding::read::Error::Custom("the update does not apply".into()))?;
    Ok(doc)
}

impl Shared {
    fn stopping(&self) -> bool {
        self.lock().shutdown
    }

    fn take_workspace(&self, force: bool) -> bool {
        let mut state = self.lock();
        let taken = std::mem::take(&mut state.workspace_dirty) || force;
        state.workspace_in_flight = taken;
        taken
    }

    fn finish_workspace(&self) {
        self.lock().workspace_in_flight = false;
        self.changed.notify_all();
    }

    fn mark_due_now(&self, docs: &[DocId]) {
        let now = Instant::now();
        let due = now.checked_sub(self.config.first_edit_delay).unwrap_or(now);
        let mut state = self.lock();
        for doc in docs {
            state.pending_bodies.insert(doc.clone(), due);
        }
    }

    fn take_pages(&self, all: bool) -> Vec<DocId> {
        let now = Instant::now();
        let first_edit_delay = self.config.first_edit_delay;
        let mut state = self.lock();
        let due: Vec<DocId> = state
            .pending_bodies
            .iter()
            .filter(|(_, first)| all || now >= **first + first_edit_delay)
            .map(|(doc, _)| doc.clone())
            .collect();
        for doc in &due {
            state.pending_bodies.remove(doc);
        }
        due
    }

    fn wait_for_work(&self) -> bool {
        let mut state = self.lock();
        state.busy = false;
        self.changed.notify_all();
        loop {
            if state.shutdown {
                return false;
            }
            let now = Instant::now();
            let next = state
                .pending_bodies
                .values()
                .min()
                .map(|first| *first + self.config.first_edit_delay);
            if state.workspace_dirty || next.is_some_and(|due| now >= due) {
                state.busy = true;
                return true;
            }
            state = match next {
                Some(due) => {
                    self.changed
                        .wait_timeout(state, due - now)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
                None => self
                    .changed
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner),
            };
        }
    }

    fn sleep(&self, delay: Duration) -> bool {
        let deadline = Instant::now() + delay;
        let mut state = self.lock();
        state.busy = false;
        self.changed.notify_all();
        loop {
            if state.shutdown {
                return false;
            }
            let now = Instant::now();
            if now >= deadline {
                state.busy = true;
                return true;
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}
