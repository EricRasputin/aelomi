use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

use super::db::{self, CacheOpen, CacheProblem, CachedPage, OrderKey, PageKey, RunError, Writer};
use super::{
    HoldPoint, IndexChange, IndexEvent, IndexStatus, PageIssue, PageProblem, Shared, Source,
    StartupReport,
};
use crate::storage::DocId;
use crate::workspace::body::{Body, BodyProblem, read_body};
use crate::workspace::records::{PageId, RecordError, read_pages};
use crate::workspace::tree::forest;

const RETRY_FIRST: Duration = Duration::from_millis(100);
const RETRY_MAX: Duration = Duration::from_secs(30);
const PAGES_PER_TRANSACTION: usize = 128;
const REPLACES_WITHOUT_READY: u32 = 2;

pub(super) fn run(shared: &Shared, source: &dyn Source, path: &Path) {
    shared.emit(&IndexEvent::Status(IndexStatus::Rebuilding));
    let mut retry = RETRY_FIRST;
    let mut opening = CacheOpen::ReuseOrCreate;
    let mut replaces = 0;
    loop {
        let mut worker = Worker {
            shared,
            source,
            path,
            committed_rows: BTreeMap::new(),
            order: BTreeMap::new(),
            reconciled: false,
            workspace_failure: None,
            page_issues: BTreeMap::new(),
            retry: &mut retry,
            ready: false,
            report: StartupReport::default(),
        };
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            worker.run(std::mem::take(&mut opening))
        }));
        let ready = worker.ready;
        let reason = match outcome {
            Ok(Ok(())) => return,
            Ok(Err(RunError::Replace(problem))) => {
                shared.stop_serving(IndexStatus::Rebuilding);
                replaces = if ready { 1 } else { replaces + 1 };
                let reason = format!("the index was replaced repeatedly: {problem}");
                opening = CacheOpen::ForceReplace(problem);
                if replaces <= REPLACES_WITHOUT_READY {
                    continue;
                }
                reason
            }
            Ok(Err(e)) => e.to_string(),
            Err(_) => "the index worker panicked".to_owned(),
        };
        shared.stop_serving(IndexStatus::Failed { reason });
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
    committed_rows: BTreeMap<PageId, CachedPage>,
    order: BTreeMap<OrderKey, PageKey>,
    reconciled: bool,
    workspace_failure: Option<String>,
    page_issues: BTreeMap<PageId, PageIssue>,
    retry: &'a mut Duration,
    ready: bool,
    report: StartupReport,
}

enum Bodies {
    Due,
    All,
}

impl Worker<'_> {
    fn run(&mut self, opening: CacheOpen) -> Result<(), RunError> {
        let started = Instant::now();
        let gate = self.shared.gate.clone();
        let hold = move |point| gate.pass(point);
        let opened = Writer::open(self.path, opening, &hold)?;
        let mut db = opened.writer;
        self.report.check = opened.timings;
        self.shared.emit(&IndexEvent::Cache(opened.state));
        self.committed_rows = opened.pages;
        self.order = self
            .committed_rows
            .iter()
            .map(|(id, page)| (page.order(id), page.key))
            .collect();
        self.sync_workspace(&mut db, true)?;
        self.sync_bodies(&mut db, Bodies::All)?;
        self.sync_workspace(&mut db, false)?;
        self.sync_bodies(&mut db, Bodies::All)?;
        if self.shared.stopping() {
            return Ok(());
        }
        let publishing = Instant::now();
        self.publish();
        self.ready = true;
        self.report.publish = publishing.elapsed();
        self.report.total = started.elapsed();
        self.shared
            .emit(&IndexEvent::Startup(Box::new(std::mem::take(
                &mut self.report,
            ))));
        *self.retry = RETRY_FIRST;
        while self.shared.wait_for_work() {
            if let Some(problem) = self.shared.replace_requested() {
                return Err(RunError::Replace(problem));
            }
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
        let issues: Arc<[PageIssue]> = self
            .page_issues
            .values()
            .filter(|issue| {
                self.committed_rows
                    .get(&issue.page_id)
                    .is_some_and(|page| page.columns.active)
            })
            .cloned()
            .collect();
        let status = match &self.workspace_failure {
            None => IndexStatus::Ready,
            Some(reason) => IndexStatus::Failed {
                reason: format!("{}: {reason}", DocId::workspace()),
            },
        };
        self.shared.lock().issues = issues;
        self.shared.publish(status);
    }

    fn sync_workspace(&mut self, db: &mut Writer, force: bool) -> Result<bool, RunError> {
        if !self.shared.take_workspace(force) {
            return Ok(false);
        }
        self.apply_workspace(db)?;
        self.shared.finish_workspace();
        Ok(true)
    }

    fn apply_workspace(&mut self, db: &mut Writer) -> Result<(), RunError> {
        let workspace = DocId::workspace();
        let started = Instant::now();
        let bytes = self
            .source
            .load(&workspace)
            .map_err(|e| RunError::Source(workspace.clone(), e))?;
        let loaded = Instant::now();
        let records = match decode(&bytes)
            .map_err(|e| e.to_string())
            .and_then(|doc| read_pages(&doc.transact()).map_err(|e: RecordError| e.to_string()))
        {
            Ok(records) => records,
            Err(reason) => {
                self.workspace_failure = Some(reason);
                return Ok(());
            }
        };
        self.workspace_failure = None;
        let decoded = Instant::now();
        let forest = forest(&records);
        let columns = db::page_columns(records, &forest);
        let erased = self
            .committed_rows
            .keys()
            .filter(|id| !columns.contains_key(*id))
            .count();
        if erased > 0 {
            return Err(RunError::Replace(CacheProblem::ErasedPages {
                count: erased,
            }));
        }
        let mut added: Vec<(OrderKey, PageId)> = Vec::new();
        for (id, page) in &columns {
            match self.committed_rows.get(id) {
                Some(cached) if cached.columns.created_at != page.created_at => {
                    return Err(RunError::Replace(CacheProblem::OrderChanged));
                }
                Some(_) => {}
                None => added.push(((page.created_at, Reverse(id.clone())), id.clone())),
            }
        }
        added.sort();
        let order_keys: Vec<OrderKey> = added.iter().map(|(order, _)| order.clone()).collect();
        let keys = db::allocate(&self.order, &order_keys)
            .ok_or(RunError::Replace(CacheProblem::KeySpace))?;
        let new_keys: BTreeMap<PageId, PageKey> =
            added.into_iter().map(|(_, id)| id).zip(keys).collect();
        let next: BTreeMap<PageId, CachedPage> = columns
            .into_iter()
            .map(|(id, columns)| {
                let key = match self.committed_rows.get(&id) {
                    Some(cached) => cached.key,
                    None => new_keys[&id],
                };
                (id, CachedPage { key, columns })
            })
            .collect();
        let forested = Instant::now();
        let tx = db.transaction()?;
        let mut relabel = BTreeSet::new();
        for (id, page) in &next {
            match self.committed_rows.get(id) {
                None => {
                    db::write_page(&tx, id, page, true)?;
                    db::write_title(&tx, page)?;
                    relabel.insert(id.clone());
                }
                Some(old) if old.columns != page.columns => {
                    db::write_page(&tx, id, page, false)?;
                    if old.columns.active != page.columns.active {
                        db::write_title(&tx, page)?;
                        db::write_block_texts(&tx, page, &next)?;
                    } else if old.columns.title != page.columns.title {
                        db::write_title(&tx, page)?;
                    }
                    if old.columns.title != page.columns.title {
                        relabel.insert(id.clone());
                    }
                }
                Some(_) => {}
            }
        }
        db::rerender_links_to(&tx, &relabel, &next)?;
        self.pass(HoldPoint::BeforeCommit);
        tx.commit()?;
        let marked: Vec<DocId> = next
            .keys()
            .filter(|id| !self.reconciled || new_keys.contains_key(*id))
            .map(|id| id.doc_id().clone())
            .collect();
        for (id, key) in new_keys {
            let created_at = next[&id].columns.created_at;
            self.order.insert((created_at, Reverse(id)), key);
        }
        self.committed_rows = next;
        self.reconciled = true;
        self.shared.mark_due_now(&marked);
        if !self.ready {
            self.report.workspace_load += loaded - started;
            self.report.workspace_decode += decoded - loaded;
            self.report.workspace_forest += forested - decoded;
            self.report.workspace_sql += forested.elapsed();
        }
        Ok(())
    }

    fn sync_bodies(&mut self, db: &mut Writer, which: Bodies) -> Result<Vec<PageId>, RunError> {
        let marked = self.shared.take_pages(matches!(which, Bodies::All));
        let mut synced = Vec::new();
        for chunk in marked.chunks(PAGES_PER_TRANSACTION) {
            let mut projected: Vec<(PageId, Body, Option<PageProblem>)> = Vec::new();
            for doc in chunk {
                if self.shared.stopping() {
                    return Ok(synced);
                }
                let Ok(page) = PageId::new(doc.as_str()) else {
                    continue;
                };
                if !self.committed_rows.contains_key(&page) {
                    continue;
                }
                self.pass(HoldPoint::BeforeLoad(doc.clone()));
                let started = Instant::now();
                let bytes = self
                    .source
                    .load(doc)
                    .map_err(|e| RunError::Source(doc.clone(), e))?;
                let loaded = Instant::now();
                let (body, problem) = match decode(&bytes) {
                    Ok(ydoc) => {
                        let decoded = Instant::now();
                        let body = read_body(&ydoc.transact());
                        if !self.ready {
                            self.report.page_decode += decoded - loaded;
                            self.report.page_project += decoded.elapsed();
                        }
                        let problem = body.problem.clone().map(|problem| match problem {
                            BodyProblem::UnrecognizedRoot(root) => {
                                PageProblem::UnrecognizedRoot(root)
                            }
                            BodyProblem::MissingBlockId => PageProblem::MissingBlockId,
                            BodyProblem::InvalidPageLink => PageProblem::InvalidPageLink,
                        });
                        (body, problem)
                    }
                    Err(_) => (Body::default(), Some(PageProblem::Undecodable)),
                };
                if !self.ready {
                    self.report.pages += 1;
                    self.report.loaded_bytes += bytes.len();
                    self.report.page_load += loaded - started;
                }
                projected.push((page, body, problem));
            }
            let started = Instant::now();
            let tx = db.transaction()?;
            for (page, body, _) in &projected {
                db::sync_body(
                    &tx,
                    &self.committed_rows[page],
                    &body.blocks,
                    &self.committed_rows,
                )?;
            }
            self.pass(HoldPoint::BeforeCommit);
            tx.commit()?;
            if !self.ready {
                self.report.page_sql += started.elapsed();
            }
            for (page, body, problem) in projected {
                match problem {
                    Some(problem) => {
                        let issue = PageIssue {
                            page_id: page.clone(),
                            problem,
                            indexed_blocks: u32::try_from(body.blocks.len()).unwrap_or(u32::MAX),
                        };
                        self.page_issues.insert(page.clone(), issue);
                    }
                    None => {
                        self.page_issues.remove(&page);
                    }
                }
                synced.push(page);
            }
        }
        synced.sort();
        synced.dedup();
        Ok(synced)
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

    fn stop_serving(&self, status: IndexStatus) {
        let mut state = self.lock();
        state.workspace_in_flight = false;
        state.replace = None;
        drop(state);
        self.publish(status);
        self.lock_reader().take();
    }

    fn replace_requested(&self) -> Option<CacheProblem> {
        self.lock().replace.clone()
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
        let delay = self.config.first_edit_delay;
        let mut state = self.lock();
        let due: Vec<DocId> = state
            .pending_bodies
            .iter()
            .filter(|(_, first)| all || now >= **first + delay)
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
            if state.workspace_dirty
                || state.replace.is_some()
                || next.is_some_and(|due| now >= due)
            {
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
