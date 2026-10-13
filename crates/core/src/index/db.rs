use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, Transaction, params};

use crate::workspace::body::{Block, Segment, render};
use crate::workspace::records::{PageId, PageRecord};
use crate::workspace::tree::Forest;

/// The index's file name, directly in the data folder.
pub const FILE: &str = "index.sqlite";
const SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

pub(crate) const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = r#"
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
CREATE TABLE blocks (
  rowid INTEGER PRIMARY KEY,
  page_id TEXT NOT NULL,
  block_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  segments TEXT NOT NULL,
  text TEXT NOT NULL
);
CREATE INDEX blocks_by_page ON blocks (page_id, ordinal);
CREATE TABLE links (
  target_id TEXT NOT NULL,
  block_rowid INTEGER NOT NULL,
  source_id TEXT NOT NULL,
  PRIMARY KEY (target_id, block_rowid)
) WITHOUT ROWID;
CREATE INDEX links_by_source ON links (source_id);
CREATE INDEX links_by_block ON links (block_rowid);
CREATE VIRTUAL TABLE title_fts USING fts5 (
  title, content = 'pages', content_rowid = 'rowid',
  tokenize = 'unicode61 remove_diacritics 2'
);
CREATE VIRTUAL TABLE block_fts USING fts5 (
  text, content = 'blocks', content_rowid = 'rowid',
  tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER pages_insert AFTER INSERT ON pages BEGIN
  INSERT INTO title_fts (rowid, title) VALUES (new.rowid, new.title);
END;
CREATE TRIGGER pages_delete AFTER DELETE ON pages BEGIN
  INSERT INTO title_fts (title_fts, rowid, title) VALUES ('delete', old.rowid, old.title);
END;
CREATE TRIGGER pages_title AFTER UPDATE OF title ON pages BEGIN
  INSERT INTO title_fts (title_fts, rowid, title) VALUES ('delete', old.rowid, old.title);
  INSERT INTO title_fts (rowid, title) VALUES (new.rowid, new.title);
END;
CREATE TRIGGER blocks_insert AFTER INSERT ON blocks BEGIN
  INSERT INTO block_fts (rowid, text) VALUES (new.rowid, new.text);
END;
CREATE TRIGGER blocks_delete AFTER DELETE ON blocks BEGIN
  INSERT INTO block_fts (block_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
END;
CREATE TRIGGER blocks_text AFTER UPDATE OF text ON blocks BEGIN
  INSERT INTO block_fts (block_fts, rowid, text) VALUES ('delete', old.rowid, old.text);
  INSERT INTO block_fts (rowid, text) VALUES (new.rowid, new.text);
END;
"#;

/// What the startup check found.
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
    OldSchema {
        version: i64,
    },
    SchemaMismatch,
    IntegrityCheck(String),
    FtsIntegrityCheck(String),
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
            Self::OldSchema { version } => write!(f, "old-schema (version {version})"),
            Self::SchemaMismatch => f.write_str("schema-mismatch"),
            Self::IntegrityCheck(e) => write!(f, "integrity-check ({e})"),
            Self::FtsIntegrityCheck(e) => write!(f, "fts-integrity-check ({e})"),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Fatal {
    Sql(rusqlite::Error),
    Io(io::Error),
    Source(crate::storage::DocId, io::Error),
    Folder(String),
}

impl fmt::Display for Fatal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(e) => write!(f, "index database error: {e}"),
            Self::Io(e) => write!(f, "index file error: {e}"),
            Self::Source(doc, e) => write!(f, "could not load doc {doc}: {e}"),
            Self::Folder(name) => write!(f, "{name} is a folder, so the index cannot be created"),
        }
    }
}

impl From<rusqlite::Error> for Fatal {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sql(e)
    }
}

impl From<io::Error> for Fatal {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

pub(crate) struct Db(Connection);

impl Db {
    pub fn open(path: &Path) -> Result<(Self, CacheState), Fatal> {
        let state = match entries(path)? {
            Entries::Missing => {
                remove(path)?;
                CacheState::Created
            }
            Entries::Files => match check(path) {
                Ok(()) => CacheState::Reused,
                Err(problem) => {
                    remove(path)?;
                    CacheState::Replaced(problem)
                }
            },
            Entries::NotFiles => {
                remove(path)?;
                CacheState::Replaced(CacheProblem::NotAFile)
            }
        };
        let conn = Connection::open_with_flags(path, write_flags())?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        if state != CacheState::Reused {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(SCHEMA)?;
            tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            tx.commit()?;
        }
        Ok((Self(conn), state))
    }

    pub fn transaction(&mut self) -> rusqlite::Result<Transaction<'_>> {
        self.0.transaction()
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

enum Entries {
    Missing,
    Files,
    NotFiles,
}

fn names(path: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    std::iter::once(path.to_path_buf()).chain(SIDECARS.iter().map(move |suffix| {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        PathBuf::from(name)
    }))
}

fn entries(path: &Path) -> Result<Entries, Fatal> {
    let mut main = false;
    for (i, name) in names(path).enumerate() {
        match std::fs::symlink_metadata(&name) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
            Ok(meta) if meta.is_dir() => return Err(Fatal::Folder(file_name(&name))),
            Ok(meta) if meta.is_file() => main |= i == 0,
            Ok(_) => return Ok(Entries::NotFiles),
        }
    }
    Ok(if main {
        Entries::Files
    } else {
        Entries::Missing
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

fn remove(path: &Path) -> Result<(), Fatal> {
    for name in names(path) {
        match std::fs::symlink_metadata(&name) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
            Ok(meta) if meta.is_dir() => return Err(Fatal::Folder(file_name(&name))),
            Ok(_) => std::fs::remove_file(&name)?,
        }
    }
    Ok(())
}

fn check(path: &Path) -> Result<(), CacheProblem> {
    let not_db = |e: rusqlite::Error| CacheProblem::NotADatabase(e.to_string());
    let conn = Connection::open_with_flags(path, write_flags()).map_err(not_db)?;
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(not_db)?;
    if version != SCHEMA_VERSION {
        return Err(CacheProblem::OldSchema { version });
    }
    if schema(&conn).map_err(not_db)? != expected_schema().map_err(not_db)? {
        return Err(CacheProblem::SchemaMismatch);
    }
    let integrity: Vec<String> = conn
        .prepare("PRAGMA integrity_check")
        .and_then(|mut s| s.query_map([], |row| row.get(0))?.collect())
        .map_err(|e| CacheProblem::IntegrityCheck(e.to_string()))?;
    if integrity != ["ok"] {
        return Err(CacheProblem::IntegrityCheck(integrity.join("; ")));
    }
    for table in ["title_fts", "block_fts"] {
        conn.execute(
            &format!("INSERT INTO {table} ({table}, rank) VALUES ('integrity-check', 1)"),
            [],
        )
        .map_err(|e| CacheProblem::FtsIntegrityCheck(format!("{table}: {e}")))?;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PageRow {
    pub record: PageRecord,
    pub parent: Option<PageId>,
    pub active: bool,
}

pub(crate) fn page_rows(
    records: BTreeMap<PageId, PageRecord>,
    forest: &Forest,
) -> BTreeMap<PageId, PageRow> {
    records
        .into_iter()
        .map(|(id, record)| {
            let row = PageRow {
                parent: forest.parent[&id].clone(),
                active: forest.active.contains(&id),
                record,
            };
            (id, row)
        })
        .collect()
}

pub(crate) fn label(rows: &BTreeMap<PageId, PageRow>, target: &PageId) -> String {
    match rows.get(target) {
        Some(row) if row.record.title.is_empty() => "Untitled".to_owned(),
        Some(row) => row.record.title.clone(),
        None => "Deleted page".to_owned(),
    }
}

pub(crate) fn upsert_page(tx: &Transaction, id: &PageId, row: &PageRow) -> rusqlite::Result<()> {
    let record = &row.record;
    let trash = record.trash.as_ref();
    tx.prepare_cached(
        "INSERT INTO pages (page_id, title, parent_id, sibling_order, favorite_order,
           trash_batch_id, trashed_at, trash_original_parent_id, trash_original_order,
           created_at, updated_at, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT (page_id) DO UPDATE SET title = excluded.title,
           parent_id = excluded.parent_id, sibling_order = excluded.sibling_order,
           favorite_order = excluded.favorite_order, trash_batch_id = excluded.trash_batch_id,
           trashed_at = excluded.trashed_at,
           trash_original_parent_id = excluded.trash_original_parent_id,
           trash_original_order = excluded.trash_original_order,
           created_at = excluded.created_at, updated_at = excluded.updated_at,
           active = excluded.active",
    )?
    .execute(params![
        id.as_str(),
        record.title,
        row.parent.as_ref().map(PageId::as_str),
        record.placement.order as i64,
        record.favorite.as_ref().map(|f| f.order as i64),
        trash.map(|t| t.batch_id.as_str()),
        trash.map(|t| t.trashed_at as i64),
        trash.and_then(|t| t.original_parent.as_ref().map(PageId::as_str)),
        trash.map(|t| t.original_order as i64),
        record.created_at as i64,
        record.updated_at as i64,
        row.active,
    ])?;
    Ok(())
}

pub(crate) fn delete_page(tx: &Transaction, page: &str) -> rusqlite::Result<()> {
    tx.prepare_cached("DELETE FROM pages WHERE page_id = ?1")?
        .execute([page])?;
    tx.prepare_cached("DELETE FROM links WHERE source_id = ?1")?
        .execute([page])?;
    tx.prepare_cached("DELETE FROM blocks WHERE page_id = ?1")?
        .execute([page])?;
    Ok(())
}

pub(crate) fn retain_pages(tx: &Transaction, keep: &BTreeSet<PageId>) -> rusqlite::Result<()> {
    let keep = serde_json::to_string(&keep.iter().map(PageId::as_str).collect::<Vec<_>>())
        .expect("strings serialize");
    for sql in [
        "DELETE FROM pages WHERE page_id NOT IN (SELECT value FROM json_each(?1))",
        "DELETE FROM links WHERE source_id NOT IN (SELECT value FROM json_each(?1))",
        "DELETE FROM blocks WHERE page_id NOT IN (SELECT value FROM json_each(?1))",
    ] {
        tx.execute(sql, [&keep])?;
    }
    Ok(())
}

pub(crate) fn replace_body(
    tx: &Transaction,
    page: &PageId,
    blocks: &[Block],
    rows: &BTreeMap<PageId, PageRow>,
) -> rusqlite::Result<()> {
    struct Old {
        rowid: i64,
        ordinal: i64,
        segments: String,
        text: String,
    }
    let mut old: BTreeMap<String, VecDeque<Old>> = BTreeMap::new();
    let mut read = tx.prepare_cached(
        "SELECT rowid, block_id, ordinal, segments, text FROM blocks WHERE page_id = ?1 ORDER BY rowid",
    )?;
    for row in read.query_map([page.as_str()], |row| {
        Ok((
            row.get::<_, String>(1)?,
            Old {
                rowid: row.get(0)?,
                ordinal: row.get(2)?,
                segments: row.get(3)?,
                text: row.get(4)?,
            },
        ))
    })? {
        let (block_id, block) = row?;
        old.entry(block_id).or_default().push_back(block);
    }
    let mut insert = tx.prepare_cached(
        "INSERT INTO blocks (page_id, block_id, ordinal, segments, text) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    let mut move_block = tx.prepare_cached("UPDATE blocks SET ordinal = ?2 WHERE rowid = ?1")?;
    let mut rewrite = tx.prepare_cached(
        "UPDATE blocks SET ordinal = ?2, segments = ?3, text = ?4 WHERE rowid = ?1",
    )?;
    let mut unlink = tx.prepare_cached("DELETE FROM links WHERE block_rowid = ?1")?;
    let mut link = tx.prepare_cached(
        "INSERT OR IGNORE INTO links (target_id, block_rowid, source_id) VALUES (?1, ?2, ?3)",
    )?;
    let mut delete = tx.prepare_cached("DELETE FROM blocks WHERE rowid = ?1")?;
    for (ordinal, block) in blocks.iter().enumerate() {
        if block.segments.is_empty() {
            continue;
        }
        let ordinal = ordinal as i64;
        let segments = encode_segments(&block.segments);
        let text = render(&block.segments, |target| label(rows, target));
        let rowid = match old.get_mut(block.id.as_str()).and_then(VecDeque::pop_front) {
            Some(was) if was.segments == segments && was.text == text => {
                if was.ordinal != ordinal {
                    move_block.execute(params![was.rowid, ordinal])?;
                }
                continue;
            }
            Some(was) => {
                rewrite.execute(params![was.rowid, ordinal, segments, text])?;
                unlink.execute([was.rowid])?;
                was.rowid
            }
            None => {
                insert.execute(params![
                    page.as_str(),
                    block.id.as_str(),
                    ordinal,
                    segments,
                    text
                ])?;
                tx.last_insert_rowid()
            }
        };
        for segment in &block.segments {
            if let Segment::Link(target) = segment {
                link.execute(params![target.as_str(), rowid, page.as_str()])?;
            }
        }
    }
    for was in old.into_values().flatten() {
        unlink.execute([was.rowid])?;
        delete.execute([was.rowid])?;
    }
    Ok(())
}

pub(crate) fn rerender_links_to(
    tx: &Transaction,
    targets: &BTreeSet<PageId>,
    rows: &BTreeMap<PageId, PageRow>,
) -> rusqlite::Result<()> {
    let mut blocks = BTreeSet::new();
    let mut linking = tx.prepare_cached("SELECT block_rowid FROM links WHERE target_id = ?1")?;
    for target in targets {
        for rowid in linking.query_map([target.as_str()], |row| row.get::<_, i64>(0))? {
            blocks.insert(rowid?);
        }
    }
    let mut read = tx.prepare_cached("SELECT segments, text FROM blocks WHERE rowid = ?1")?;
    let mut write = tx.prepare_cached("UPDATE blocks SET text = ?2 WHERE rowid = ?1")?;
    for rowid in blocks {
        let (segments, old): (String, String) =
            read.query_row([rowid], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let text = render(&decode_segments(&segments), |target| label(rows, target));
        if text != old {
            write.execute(params![rowid, text])?;
        }
    }
    Ok(())
}

fn encode_segments(segments: &[Segment]) -> String {
    let values: Vec<serde_json::Value> = segments
        .iter()
        .map(|segment| match segment {
            Segment::Text(text) => serde_json::Value::String(text.clone()),
            Segment::Link(target) => serde_json::json!({ "page": target.as_str() }),
        })
        .collect();
    serde_json::Value::Array(values).to_string()
}

fn decode_segments(json: &str) -> Vec<Segment> {
    let Ok(serde_json::Value::Array(values)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    values
        .into_iter()
        .filter_map(|value| match value {
            serde_json::Value::String(text) => Some(Segment::Text(text)),
            value => PageId::new(value.get("page")?.as_str()?)
                .ok()
                .map(Segment::Link),
        })
        .collect()
}
