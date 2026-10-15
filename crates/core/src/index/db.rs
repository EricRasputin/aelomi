use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};

use super::HoldPoint;
use super::tokenizer::with_tokenizer_spec;
use crate::workspace::body::{Block, BlockId, Segment, render};
use crate::workspace::records::{MAX_SAFE_INTEGER, PageId, PageRecord};
use crate::workspace::tree::Forest;

/// The index's file name, directly in the data folder.
pub const FILE: &str = "index.sqlite";
const SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

pub(crate) const SCHEMA_VERSION: i64 = 2;

pub(crate) const SLOT_BITS: u32 = 20;
pub(crate) const SLOTS: i64 = 1 << SLOT_BITS;
pub(crate) const PAGE_KEYS: i64 = 1 << (63 - SLOT_BITS);
pub(crate) const KEY_SPACING: i64 = 1 << 16;

macro_rules! define_schema {
    (
        $name:literal, $option:literal, $value:literal,
        $before_title:literal, $between_tables:literal, $after_block:literal
    ) => {
        pub(crate) const SCHEMA: &str = concat!(
            $before_title,
            concat!($name, " ", $option, " ", $value),
            $between_tables,
            concat!($name, " ", $option, " ", $value),
            $after_block,
        );
    };
}

with_tokenizer_spec!(
    define_schema,
    r#"
CREATE TABLE pages (
  rowid INTEGER PRIMARY KEY,
  page_id TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  parent_id TEXT,
  sibling_order INTEGER NOT NULL,
  favorite_order INTEGER,
  trash_batch_id TEXT,
  trashed_at INTEGER,
  trash_original_parent_id TEXT,
  trash_original_order INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  active INTEGER NOT NULL
);
CREATE TABLE title_text (
  rowid INTEGER PRIMARY KEY,
  title TEXT NOT NULL
);
CREATE TABLE blocks (
  rowid INTEGER PRIMARY KEY,
  block_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  segments TEXT NOT NULL
);
CREATE TABLE block_text (
  rowid INTEGER PRIMARY KEY,
  text TEXT NOT NULL
);
CREATE TABLE links (
  target_id TEXT NOT NULL,
  block_rowid INTEGER NOT NULL,
  PRIMARY KEY (target_id, block_rowid)
) WITHOUT ROWID;
CREATE INDEX links_by_block ON links (block_rowid);
CREATE VIRTUAL TABLE title_fts USING fts5 (
  title, content = 'title_text', content_rowid = 'rowid',
  tokenize = '"#,
    r#"', prefix = '1 2 3'
);
CREATE VIRTUAL TABLE block_fts USING fts5 (
  text, content = 'block_text', content_rowid = 'rowid',
  tokenize = '"#,
    r#"', prefix = '1 2 3'
);
CREATE TRIGGER title_text_insert AFTER INSERT ON title_text BEGIN
  INSERT INTO title_fts (rowid, title) VALUES (new.rowid, new.title);
END;
CREATE TRIGGER title_text_delete AFTER DELETE ON title_text BEGIN
  INSERT INTO title_fts (title_fts, rowid, title) VALUES ('delete', old.rowid, old.title);
END;
CREATE TRIGGER title_text_update AFTER UPDATE OF title ON title_text
WHEN old.title IS NOT new.title BEGIN
  INSERT INTO title_fts (title_fts, rowid, title) VALUES ('delete', old.rowid, old.title);
  INSERT INTO title_fts (rowid, title) VALUES (new.rowid, new.title);
END;
CREATE TRIGGER block_text_insert AFTER INSERT ON block_text BEGIN
  INSERT INTO block_fts (rowid, text) VALUES (new.rowid, new.text);
END;
CREATE TRIGGER block_text_delete AFTER DELETE ON block_text BEGIN
  INSERT INTO block_fts (block_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
END;
CREATE TRIGGER block_text_update AFTER UPDATE OF text ON block_text
WHEN old.text IS NOT new.text BEGIN
  INSERT INTO block_fts (block_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
  INSERT INTO block_fts (rowid, text) VALUES (new.rowid, new.text);
END;
"#
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheState {
    /// There was no index file.
    Created,
    /// The file passed every check.
    Reused,
    /// The file and its sidecars were deleted and a new index was created.
    Replaced(CacheProblem),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheProblem {
    /// The file or a sidecar is a symlink or a special file. Only the entry was removed.
    NotAFile,
    NotADatabase(String),
    /// An older or a newer build wrote the file.
    SchemaVersion {
        found: i64,
        expected: i64,
    },
    SchemaMismatch,
    IntegrityCheck(String),
    FtsIntegrityCheck(String),
    /// Pages the cache holds have no record any more. Replacing the whole file leaves none of
    /// their bytes behind (TREE-14).
    ErasedPages {
        count: usize,
    },
    /// A stored value is invalid although SQLite's checks pass.
    Corrupt(Corrupt),
    /// No page key or block slot is free where the order needs one.
    KeySpace,
    /// A page's creation time changed, so its key no longer orders it.
    OrderChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corrupt {
    PageId,
    BlockId,
    Ordinal,
    Segments,
    Key,
    MissingRow,
    Field,
}

impl Corrupt {
    pub(crate) fn from_row_error(error: &rusqlite::Error) -> Option<Self> {
        match error {
            rusqlite::Error::InvalidColumnType(..)
            | rusqlite::Error::FromSqlConversionFailure(..)
            | rusqlite::Error::IntegralValueOutOfRange(..)
            | rusqlite::Error::Utf8Error(..) => Some(Self::Field),
            _ => None,
        }
    }
}

impl fmt::Display for CacheState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => f.write_str("created"),
            Self::Reused => f.write_str("reused"),
            Self::Replaced(problem) => write!(f, "replaced reason={problem}"),
        }
    }
}

impl fmt::Display for CacheProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAFile => f.write_str("not-a-file"),
            Self::NotADatabase(e) => write!(f, "not-a-database ({e})"),
            Self::SchemaVersion { found, expected } => {
                write!(f, "schema-version (found {found}, expected {expected})")
            }
            Self::SchemaMismatch => f.write_str("schema-mismatch"),
            Self::IntegrityCheck(e) => write!(f, "integrity-check ({e})"),
            Self::FtsIntegrityCheck(e) => write!(f, "fts-integrity-check ({e})"),
            Self::ErasedPages { count } => write!(f, "erased-pages (count {count})"),
            Self::Corrupt(what) => write!(f, "corrupt ({what})"),
            Self::KeySpace => f.write_str("key-space"),
            Self::OrderChanged => f.write_str("order-changed"),
        }
    }
}

impl fmt::Display for Corrupt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PageId => "page-id",
            Self::BlockId => "block-id",
            Self::Ordinal => "ordinal",
            Self::Segments => "segments",
            Self::Key => "key",
            Self::MissingRow => "missing-row",
            Self::Field => "field",
        })
    }
}

#[derive(Debug)]
pub(crate) enum RunError {
    Sql(rusqlite::Error),
    Io(io::Error),
    Source(crate::storage::DocId, io::Error),
    Folder(String),
    Replace(CacheProblem),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(e) => write!(f, "index database error: {e}"),
            Self::Io(e) => write!(f, "index file error: {e}"),
            Self::Source(doc, e) => write!(f, "could not load doc {doc}: {e}"),
            Self::Folder(name) => write!(f, "{name} is a folder, so the index cannot be created"),
            Self::Replace(problem) => write!(f, "the index must be replaced: {problem}"),
        }
    }
}

impl From<rusqlite::Error> for RunError {
    fn from(e: rusqlite::Error) -> Self {
        match Corrupt::from_row_error(&e) {
            Some(what) => what.into(),
            None => Self::Sql(e),
        }
    }
}

impl From<io::Error> for RunError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<Corrupt> for RunError {
    fn from(what: Corrupt) -> Self {
        Self::Replace(CacheProblem::Corrupt(what))
    }
}

/// How long each part of the startup check took. A check that did not run is `None`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckTimings {
    pub open: Duration,
    pub schema: Option<Duration>,
    pub integrity: Option<Duration>,
    pub title_fts: Option<Duration>,
    pub block_fts: Option<Duration>,
}

#[derive(Default)]
pub(crate) enum CacheOpen {
    #[default]
    ReuseOrCreate,
    ForceReplace(CacheProblem),
}

pub(crate) struct Writer(Connection);

pub(crate) struct Opened {
    pub writer: Writer,
    pub state: CacheState,
    pub timings: CheckTimings,
    pub pages: BTreeMap<PageId, CachedPage>,
}

fn connect(path: &Path) -> Result<Connection, RunError> {
    let conn = Connection::open_with_flags(path, write_flags())?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(conn)
}

impl Writer {
    pub fn open(
        path: &Path,
        opening: CacheOpen,
        hold: &dyn Fn(HoldPoint),
    ) -> Result<Opened, RunError> {
        let started = Instant::now();
        let files = CacheFiles::new(path);
        let mut timings = CheckTimings::default();
        let mut state = match (opening, files.entries()?) {
            (_, Entries::NotFiles) => {
                files.remove(hold)?;
                CacheState::Replaced(CacheProblem::NotAFile)
            }
            (CacheOpen::ForceReplace(problem), _) => {
                files.remove(hold)?;
                CacheState::Replaced(problem)
            }
            (CacheOpen::ReuseOrCreate, Entries::Missing) => {
                files.remove(hold)?;
                CacheState::Created
            }
            (CacheOpen::ReuseOrCreate, Entries::Files) => match check(path, &mut timings) {
                Ok(()) => CacheState::Reused,
                Err(problem) => {
                    files.remove(hold)?;
                    CacheState::Replaced(problem)
                }
            },
        };
        let mut writer = Self(connect(path)?);
        let mut pages = BTreeMap::new();
        if state == CacheState::Reused {
            match writer.cached_pages() {
                Ok(cached) => pages = cached,
                Err(RunError::Replace(problem)) => {
                    drop(writer);
                    files.remove(hold)?;
                    writer = Self(connect(path)?);
                    state = CacheState::Replaced(problem);
                }
                Err(e) => return Err(e),
            }
        }
        if state != CacheState::Reused {
            let tx = writer.0.unchecked_transaction()?;
            tx.execute_batch(SCHEMA)?;
            tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            tx.commit()?;
        }
        timings.open = started.elapsed();
        Ok(Opened {
            writer,
            state,
            timings,
            pages,
        })
    }

    pub fn transaction(&mut self) -> rusqlite::Result<Transaction<'_>> {
        self.0.transaction()
    }

    fn cached_pages(&self) -> Result<BTreeMap<PageId, CachedPage>, RunError> {
        let mut statement = self.0.prepare(
            "SELECT rowid, page_id, title, parent_id, sibling_order, favorite_order,
               trash_batch_id, trashed_at, trash_original_parent_id, trash_original_order,
               created_at, updated_at, active
             FROM pages ORDER BY rowid",
        )?;
        let mut rows = statement.query([])?;
        let mut pages = BTreeMap::new();
        let mut previous: Option<OrderKey> = None;
        while let Some(row) = rows.next()? {
            let key = PageKey::from_stored(row.get(0)?)?;
            let id = stored_page_id(row.get::<_, String>(1)?)?;
            let trash = match (
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<i64>>(9)?,
            ) {
                (None, None, None, None) => None,
                (Some(batch_id), Some(at), parent, Some(order)) if !batch_id.is_empty() => {
                    Some(TrashColumns {
                        batch_id,
                        trashed_at: stored_integer(at)?,
                        original_parent: parent.map(stored_page_id).transpose()?,
                        original_order: stored_integer(order)?,
                    })
                }
                _ => return Err(Corrupt::Field.into()),
            };
            let columns = PageColumns {
                title: row.get(2)?,
                normalized_parent: row
                    .get::<_, Option<String>>(3)?
                    .map(stored_page_id)
                    .transpose()?,
                sibling_order: stored_integer(row.get(4)?)?,
                favorite_order: row
                    .get::<_, Option<i64>>(5)?
                    .map(stored_integer)
                    .transpose()?,
                trash,
                created_at: stored_integer(row.get(10)?)?,
                updated_at: stored_integer(row.get(11)?)?,
                active: match row.get::<_, i64>(12)? {
                    0 => false,
                    1 => true,
                    _ => return Err(Corrupt::Field.into()),
                },
            };
            let order = (columns.created_at, Reverse(id.clone()));
            if previous.as_ref().is_some_and(|p| *p >= order) {
                return Err(Corrupt::Key.into());
            }
            previous = Some(order);
            pages.insert(id, CachedPage { key, columns });
        }
        Ok(pages)
    }
}

pub(crate) fn open_reader(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_millis(250))?;
    Ok(conn)
}

fn write_flags() -> OpenFlags {
    OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NOFOLLOW
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
}

struct CacheFiles<'path> {
    main: &'path Path,
    sidecars: [PathBuf; 3],
}

enum Entries {
    Missing,
    Files,
    NotFiles,
}

enum Entry {
    Missing,
    File,
    Other,
}

impl<'path> CacheFiles<'path> {
    fn new(main: &'path Path) -> Self {
        Self {
            main,
            sidecars: SIDECARS.map(|suffix| {
                let mut name = main.as_os_str().to_owned();
                name.push(suffix);
                PathBuf::from(name)
            }),
        }
    }

    fn entries(&self) -> Result<Entries, RunError> {
        let main = match entry_kind(self.main)? {
            Entry::Missing => Entries::Missing,
            Entry::File => Entries::Files,
            Entry::Other => return Ok(Entries::NotFiles),
        };
        for sidecar in &self.sidecars {
            if matches!(entry_kind(sidecar)?, Entry::Other) {
                return Ok(Entries::NotFiles);
            }
        }
        Ok(main)
    }

    fn remove(&self, hold: &dyn Fn(HoldPoint)) -> Result<(), RunError> {
        hold(HoldPoint::BeforeRemove);
        unlink(self.main)?;
        hold(HoldPoint::AfterRemovingMain);
        for sidecar in &self.sidecars {
            unlink(sidecar)?;
        }
        Ok(())
    }
}

fn entry_kind(path: &Path) -> Result<Entry, RunError> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Entry::Missing),
        Err(e) => Err(e.into()),
        Ok(meta) if meta.is_dir() => Err(RunError::Folder(file_name(path))),
        Ok(meta) if meta.is_file() => Ok(Entry::File),
        Ok(_) => Ok(Entry::Other),
    }
}

fn unlink(path: &Path) -> Result<(), RunError> {
    match entry_kind(path)? {
        Entry::Missing => Ok(()),
        Entry::File | Entry::Other => Ok(std::fs::remove_file(path)?),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

fn check(path: &Path, timings: &mut CheckTimings) -> Result<(), CacheProblem> {
    let not_db = |e: rusqlite::Error| CacheProblem::NotADatabase(e.to_string());
    let conn = Connection::open_with_flags(path, write_flags()).map_err(not_db)?;
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(not_db)?;
    if version != SCHEMA_VERSION {
        return Err(CacheProblem::SchemaVersion {
            found: version,
            expected: SCHEMA_VERSION,
        });
    }
    let started = Instant::now();
    if schema(&conn).map_err(not_db)? != expected_schema().map_err(not_db)? {
        return Err(CacheProblem::SchemaMismatch);
    }
    timings.schema = Some(started.elapsed());
    let started = Instant::now();
    let integrity: Vec<String> = conn
        .prepare("PRAGMA integrity_check")
        .and_then(|mut s| s.query_map([], |row| row.get(0))?.collect())
        .map_err(|e| CacheProblem::IntegrityCheck(e.to_string()))?;
    if integrity != ["ok"] {
        return Err(CacheProblem::IntegrityCheck(integrity.join("; ")));
    }
    timings.integrity = Some(started.elapsed());
    for (table, timing) in [
        ("title_fts", &mut timings.title_fts),
        ("block_fts", &mut timings.block_fts),
    ] {
        let started = Instant::now();
        conn.execute(
            &format!("INSERT INTO {table} ({table}, rank) VALUES ('integrity-check', 1)"),
            [],
        )
        .map_err(|e| CacheProblem::FtsIntegrityCheck(format!("{table}: {e}")))?;
        *timing = Some(started.elapsed());
    }
    Ok(())
}

type SchemaRows = Vec<(String, String, Option<String>)>;

fn schema(conn: &Connection) -> rusqlite::Result<SchemaRows> {
    conn.prepare("SELECT type, name, sql FROM sqlite_schema ORDER BY type, name")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect()
}

fn expected_schema() -> rusqlite::Result<SchemaRows> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA)?;
    schema(&conn)
}

pub(crate) type OrderKey = (u64, Reverse<PageId>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PageKey(i64);

impl PageKey {
    pub fn new(key: i64) -> Option<Self> {
        (1..PAGE_KEYS).contains(&key).then_some(Self(key))
    }

    fn from_stored(key: i64) -> Result<Self, Corrupt> {
        Self::new(key).ok_or(Corrupt::Key)
    }

    pub fn get(self) -> i64 {
        self.0
    }

    pub fn of_block(rowid: i64) -> Result<Self, Corrupt> {
        if rowid < 0 {
            return Err(Corrupt::Key);
        }
        Self::from_stored(rowid >> SLOT_BITS)
    }

    pub fn block(self, slot: i64) -> i64 {
        (self.0 << SLOT_BITS) | slot
    }

    pub fn blocks(self) -> (i64, i64) {
        (self.block(0), self.block(SLOTS - 1))
    }
}

pub(crate) fn allocate(
    existing: &BTreeMap<OrderKey, PageKey>,
    new: &[OrderKey],
) -> Option<Vec<PageKey>> {
    let gap = |key: &OrderKey| {
        (
            existing.range(..key.clone()).next_back().map(|(_, k)| k.0),
            existing.range(key.clone()..).next().map(|(_, k)| k.0),
        )
    };
    let mut keys = Vec::with_capacity(new.len());
    let mut at = 0;
    while at < new.len() {
        let (lo, hi) = gap(&new[at]);
        let run = new[at..]
            .iter()
            .take_while(|key| gap(key) == (lo, hi))
            .count();
        let lo = lo.unwrap_or(0);
        let n = run as i64;
        match hi {
            None => {
                let room = PAGE_KEYS - 1 - lo;
                let spacing = KEY_SPACING.min(room / n.max(1));
                if spacing < 1 {
                    return None;
                }
                keys.extend((1..=n).map(|i| PageKey(lo + i * spacing)));
            }
            Some(hi) => {
                let width = (hi - lo) as i128;
                if width - 1 < n as i128 {
                    return None;
                }
                keys.extend(
                    (1..=n).map(|i| PageKey(lo + (i as i128 * width / (n as i128 + 1)) as i64)),
                );
            }
        }
        at += run;
    }
    Some(keys)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PageColumns {
    pub title: String,
    pub normalized_parent: Option<PageId>,
    pub sibling_order: u64,
    pub favorite_order: Option<u64>,
    pub trash: Option<TrashColumns>,
    pub created_at: u64,
    pub updated_at: u64,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrashColumns {
    pub batch_id: String,
    pub trashed_at: u64,
    pub original_parent: Option<PageId>,
    pub original_order: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CachedPage {
    pub key: PageKey,
    pub columns: PageColumns,
}

impl CachedPage {
    pub fn order(&self, id: &PageId) -> OrderKey {
        (self.columns.created_at, Reverse(id.clone()))
    }
}

pub(crate) fn page_columns(
    records: BTreeMap<PageId, PageRecord>,
    forest: &Forest,
) -> BTreeMap<PageId, PageColumns> {
    records
        .into_iter()
        .map(|(id, record)| {
            let columns = PageColumns {
                normalized_parent: forest.parent[&id].clone(),
                active: forest.active.contains(&id),
                sibling_order: record.placement.order,
                favorite_order: record.favorite.map(|f| f.order),
                trash: record.trash.map(|t| TrashColumns {
                    batch_id: t.batch_id,
                    trashed_at: t.trashed_at,
                    original_parent: t.original_parent,
                    original_order: t.original_order,
                }),
                created_at: record.created_at,
                updated_at: record.updated_at,
                title: record.title,
            };
            (id, columns)
        })
        .collect()
}

pub(crate) fn label(pages: &BTreeMap<PageId, CachedPage>, target: &PageId) -> String {
    match pages.get(target) {
        Some(page) if page.columns.title.is_empty() => "Untitled".to_owned(),
        Some(page) => page.columns.title.clone(),
        None => "Deleted page".to_owned(),
    }
}

fn stored_page_id(id: String) -> Result<PageId, Corrupt> {
    PageId::new(id).map_err(|_| Corrupt::PageId)
}

fn stored_integer(n: i64) -> Result<u64, Corrupt> {
    u64::try_from(n)
        .ok()
        .filter(|n| *n <= MAX_SAFE_INTEGER)
        .ok_or(Corrupt::Field)
}

pub(crate) fn write_page(
    tx: &Transaction,
    id: &PageId,
    page: &CachedPage,
    is_new: bool,
) -> rusqlite::Result<()> {
    let c = &page.columns;
    let trash = c.trash.as_ref();
    let values = params![
        page.key.0,
        id.as_str(),
        c.title,
        c.normalized_parent.as_ref().map(PageId::as_str),
        c.sibling_order as i64,
        c.favorite_order.map(|o| o as i64),
        trash.map(|t| t.batch_id.as_str()),
        trash.map(|t| t.trashed_at as i64),
        trash.and_then(|t| t.original_parent.as_ref().map(PageId::as_str)),
        trash.map(|t| t.original_order as i64),
        c.created_at as i64,
        c.updated_at as i64,
        c.active,
    ];
    if is_new {
        tx.prepare_cached(
            "INSERT INTO pages (rowid, page_id, title, parent_id, sibling_order, favorite_order,
               trash_batch_id, trashed_at, trash_original_parent_id, trash_original_order,
               created_at, updated_at, active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?
        .execute(values)?;
    } else {
        let updated = tx
            .prepare_cached(
                "UPDATE pages SET page_id = ?2, title = ?3, parent_id = ?4, sibling_order = ?5,
                   favorite_order = ?6, trash_batch_id = ?7, trashed_at = ?8,
                   trash_original_parent_id = ?9, trash_original_order = ?10, created_at = ?11,
                   updated_at = ?12, active = ?13
                 WHERE rowid = ?1",
            )?
            .execute(values)?;
        if updated != 1 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
    }
    Ok(())
}

pub(crate) fn write_title(tx: &Transaction, page: &CachedPage) -> rusqlite::Result<()> {
    if page.columns.active {
        tx.prepare_cached(
            "INSERT INTO title_text (rowid, title) VALUES (?1, ?2)
             ON CONFLICT (rowid) DO UPDATE SET title = excluded.title",
        )?
        .execute(params![page.key.0, page.columns.title])?;
    } else {
        tx.prepare_cached("DELETE FROM title_text WHERE rowid = ?1")?
            .execute([page.key.0])?;
    }
    Ok(())
}

pub(crate) fn write_block_texts(
    tx: &Transaction,
    page: &CachedPage,
    pages: &BTreeMap<PageId, CachedPage>,
) -> Result<(), RunError> {
    let (first, last) = page.key.blocks();
    if !page.columns.active {
        tx.prepare_cached("DELETE FROM block_text WHERE rowid BETWEEN ?1 AND ?2")?
            .execute([first, last])?;
        return Ok(());
    }
    let stored: Vec<(i64, String)> = tx
        .prepare_cached("SELECT rowid, segments FROM blocks WHERE rowid BETWEEN ?1 AND ?2")?
        .query_map([first, last], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut write = tx.prepare_cached(
        "INSERT INTO block_text (rowid, text) VALUES (?1, ?2)
         ON CONFLICT (rowid) DO UPDATE SET text = excluded.text",
    )?;
    for (rowid, segments) in stored {
        let text = render(&StoredSegments::decode(&segments)?, |t| label(pages, t));
        write.execute(params![rowid, text])?;
    }
    Ok(())
}

pub(crate) fn sync_body(
    tx: &Transaction,
    page: &CachedPage,
    blocks: &[Block],
    pages: &BTreeMap<PageId, CachedPage>,
) -> Result<(), RunError> {
    struct Old {
        rowid: i64,
        ordinal: i64,
        segments: String,
        text: Option<String>,
    }
    let active = page.columns.active;
    let (first, last) = page.key.blocks();
    let mut old: BTreeMap<String, VecDeque<Old>> = BTreeMap::new();
    let mut used = BTreeSet::new();
    {
        let mut read = tx.prepare_cached(
            "SELECT b.rowid, b.block_id, b.ordinal, b.segments, t.text FROM blocks b
             LEFT JOIN block_text t ON t.rowid = b.rowid
             WHERE b.rowid BETWEEN ?1 AND ?2 ORDER BY b.rowid",
        )?;
        let mut rows = read.query([first, last])?;
        while let Some(row) = rows.next()? {
            let block = Old {
                rowid: row.get(0)?,
                ordinal: row.get(2)?,
                segments: row.get(3)?,
                text: row.get(4)?,
            };
            let block_id: String = row.get(1)?;
            if block_id.is_empty() {
                return Err(Corrupt::BlockId.into());
            }
            if block.ordinal < 0 {
                return Err(Corrupt::Ordinal.into());
            }
            if block.text.is_some() != active {
                return Err(Corrupt::MissingRow.into());
            }
            used.insert(block.rowid - first);
            old.entry(block_id).or_default().push_back(block);
        }
    }
    enum Change<'b> {
        Move(i64, i64),
        Rewrite(i64, i64, String, Option<String>, &'b [Segment]),
        Insert(i64, &'b Block, String, Option<String>),
    }
    let mut changes = Vec::new();
    for (ordinal, block) in blocks.iter().enumerate() {
        if block.segments.is_empty() {
            continue;
        }
        let ordinal = ordinal as i64;
        let segments = StoredSegments::encode(&block.segments);
        let text = active.then(|| render(&block.segments, |t| label(pages, t)));
        match old.get_mut(block.id.as_str()).and_then(VecDeque::pop_front) {
            Some(was) if was.segments == segments && was.text == text => {
                if was.ordinal != ordinal {
                    changes.push(Change::Move(was.rowid, ordinal));
                }
            }
            Some(was) => changes.push(Change::Rewrite(
                was.rowid,
                ordinal,
                segments,
                text,
                &block.segments,
            )),
            None => changes.push(Change::Insert(ordinal, block, segments, text)),
        }
    }
    let mut unlink = tx.prepare_cached("DELETE FROM links WHERE block_rowid = ?1")?;
    let mut link =
        tx.prepare_cached("INSERT OR IGNORE INTO links (target_id, block_rowid) VALUES (?1, ?2)")?;
    let mut set_text = tx.prepare_cached(
        "INSERT INTO block_text (rowid, text) VALUES (?1, ?2)
         ON CONFLICT (rowid) DO UPDATE SET text = excluded.text",
    )?;
    for was in old.into_values().flatten() {
        unlink.execute([was.rowid])?;
        tx.prepare_cached("DELETE FROM block_text WHERE rowid = ?1")?
            .execute([was.rowid])?;
        tx.prepare_cached("DELETE FROM blocks WHERE rowid = ?1")?
            .execute([was.rowid])?;
        used.remove(&(was.rowid - first));
    }
    let links = |rowid: i64, segments: &[Segment]| -> rusqlite::Result<()> {
        for segment in segments {
            if let Segment::Link(target) = segment {
                link.execute(params![target.as_str(), rowid])?;
            }
        }
        Ok(())
    };
    let mut links = links;
    let mut free = (0..SLOTS).filter(|slot| !used.contains(slot));
    for change in changes {
        match change {
            Change::Move(rowid, ordinal) => {
                tx.prepare_cached("UPDATE blocks SET ordinal = ?2 WHERE rowid = ?1")?
                    .execute([rowid, ordinal])?;
            }
            Change::Rewrite(rowid, ordinal, segments, text, parts) => {
                tx.prepare_cached(
                    "UPDATE blocks SET ordinal = ?2, segments = ?3 WHERE rowid = ?1",
                )?
                .execute(params![rowid, ordinal, segments])?;
                if let Some(text) = text {
                    set_text.execute(params![rowid, text])?;
                }
                unlink.execute([rowid])?;
                links(rowid, parts)?;
            }
            Change::Insert(ordinal, block, segments, text) => {
                let slot = free
                    .next()
                    .ok_or(RunError::Replace(CacheProblem::KeySpace))?;
                let rowid = page.key.block(slot);
                tx.prepare_cached(
                    "INSERT INTO blocks (rowid, block_id, ordinal, segments) VALUES (?1, ?2, ?3, ?4)",
                )?
                .execute(params![rowid, block.id.as_str(), ordinal, segments])?;
                if let Some(text) = text {
                    set_text.execute(params![rowid, text])?;
                }
                links(rowid, &block.segments)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn rerender_links_to(
    tx: &Transaction,
    targets: &BTreeSet<PageId>,
    pages: &BTreeMap<PageId, CachedPage>,
) -> Result<(), RunError> {
    let mut blocks = BTreeSet::new();
    let mut linking = tx.prepare_cached("SELECT block_rowid FROM links WHERE target_id = ?1")?;
    for target in targets {
        for rowid in linking.query_map([target.as_str()], |row| row.get::<_, i64>(0))? {
            blocks.insert(rowid?);
        }
    }
    let mut read = tx.prepare_cached(
        "SELECT b.segments, t.text FROM blocks b LEFT JOIN block_text t ON t.rowid = b.rowid
         WHERE b.rowid = ?1",
    )?;
    let mut write = tx.prepare_cached("UPDATE block_text SET text = ?2 WHERE rowid = ?1")?;
    for rowid in blocks {
        PageKey::of_block(rowid)?;
        let (segments, old): (String, Option<String>) = read
            .query_row([rowid], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()?
            .ok_or(Corrupt::MissingRow)?;
        let Some(old) = old else {
            continue;
        };
        let text = render(&StoredSegments::decode(&segments)?, |t| label(pages, t));
        if text != old {
            write.execute(params![rowid, text])?;
        }
    }
    Ok(())
}

pub(crate) struct StoredSegments;

impl StoredSegments {
    pub fn encode(segments: &[Segment]) -> String {
        let values: Vec<serde_json::Value> = segments
            .iter()
            .map(|segment| match segment {
                Segment::Text(text) => serde_json::Value::String(text.clone()),
                Segment::Link(target) => serde_json::json!({ "page": target.as_str() }),
            })
            .collect();
        serde_json::Value::Array(values).to_string()
    }

    pub fn decode(json: &str) -> Result<Vec<Segment>, Corrupt> {
        let Ok(serde_json::Value::Array(values)) = serde_json::from_str(json) else {
            return Err(Corrupt::Segments);
        };
        values
            .into_iter()
            .map(|value| match value {
                serde_json::Value::String(text) => Ok(Segment::Text(text)),
                serde_json::Value::Object(link) if link.len() == 1 => match link.get("page") {
                    Some(serde_json::Value::String(id)) => PageId::new(id.as_str())
                        .map(Segment::Link)
                        .map_err(|_| Corrupt::Segments),
                    _ => Err(Corrupt::Segments),
                },
                _ => Err(Corrupt::Segments),
            })
            .collect()
    }
}

pub(crate) fn stored_block_id(id: String) -> Result<BlockId, Corrupt> {
    BlockId::new(id).ok_or(Corrupt::BlockId)
}
