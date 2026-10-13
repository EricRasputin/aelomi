//! The workspace Y.Doc's page records (DATA-2). See `ARCHITECTURE.md`, "Workspace records".

use std::collections::BTreeMap;
use std::fmt;

use yrs::{Any, Array, ArrayRef, Map, MapRef, Out, ReadTxn};

use crate::storage::{DocId, InvalidDocId};

/// The workspace doc's root map: page ID to that page's record, a nested Y.Map.
pub const PAGES: &str = "pages";

pub(crate) mod key {
    pub const TITLE: &str = "title";
    pub const PLACEMENT: &str = "placement";
    pub const FAVORITE: &str = "favorite";
    pub const TRASH: &str = "trash";
    pub const CREATED_AT: &str = "createdAt";
    pub const UPDATED_AT: &str = "updatedAt";
    pub const PARENT_ID: &str = "parentId";
    pub const ORDER: &str = "order";
    pub const ORIGINAL_PARENT_ID: &str = "originalParentId";
    pub const ORIGINAL_ORDER: &str = "originalOrder";
    pub const BATCH_ID: &str = "batchId";
    pub const TRASHED_AT: &str = "trashedAt";
}

/// The largest integer a JavaScript number holds exactly, 2^53 - 1.
pub const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// A page's ID: a [`DocId`] other than the workspace doc's.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageId(DocId);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPageId(pub String);

impl PageId {
    pub fn new(id: impl Into<String>) -> Result<Self, InvalidPageId> {
        let doc = DocId::new(id).map_err(|InvalidDocId(id)| InvalidPageId(id))?;
        if doc == DocId::workspace() {
            return Err(InvalidPageId(doc.as_str().to_owned()));
        }
        Ok(Self(doc))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn doc_id(&self) -> &DocId {
        &self.0
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PageId({})", self.as_str())
    }
}

impl fmt::Display for InvalidPageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid page ID {:?}: use 1 to 64 of [0-9a-z-], not \"workspace\"",
            self.0
        )
    }
}

impl std::error::Error for InvalidPageId {}

/// One page's record. Unknown keys in the Y.Map are ignored here and kept by every writer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRecord {
    /// May be empty; the UI shows "Untitled" (TREE-7).
    pub title: String,
    pub placement: Placement,
    pub favorite: Option<Favorite>,
    pub trash: Option<Trash>,
    /// Milliseconds since the Unix epoch.
    pub created_at: u64,
    pub updated_at: u64,
}

/// Parent and sibling order, one Any object so neither changes without the other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub parent: Option<PageId>,
    pub order: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Favorite {
    pub order: u64,
}

/// Set while the page is in trash. `placement` keeps its value meanwhile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trash {
    pub original_parent: Option<PageId>,
    pub original_order: u64,
    /// Shared by the page the user trashed and the descendants trashed with it.
    pub batch_id: String,
    pub trashed_at: u64,
}

/// A record that breaks the schema. Names the page and field, never a title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordError {
    pub page: Option<String>,
    pub problem: String,
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.page {
            Some(page) => write!(f, "page record {page:?}: {}", self.problem),
            None => write!(f, "workspace doc: {}", self.problem),
        }
    }
}

impl std::error::Error for RecordError {}

/// Every page record in the workspace doc. A doc without the `pages` root has none.
pub fn read_pages<T: ReadTxn>(txn: &T) -> Result<BTreeMap<PageId, PageRecord>, RecordError> {
    let mut pages = BTreeMap::new();
    let Some(Out::YMap(root)) = txn.get(PAGES).map(|root| as_map(txn, root)) else {
        return Ok(pages);
    };
    for (id, value) in root.iter(txn) {
        let page = PageId::new(id).map_err(|e| RecordError {
            page: Some(id.to_owned()),
            problem: format!("the key is not a page ID ({e})"),
        })?;
        let fail = |problem: String| RecordError {
            page: Some(id.to_owned()),
            problem,
        };
        let Out::YMap(record) = value else {
            return Err(fail("the record is not a Y.Map".into()));
        };
        let record = read_record(txn, &record).map_err(fail)?;
        pages.insert(page, record);
    }
    Ok(pages)
}

/// A root that arrived in an update has no type yet. Reading it as a map is safe only when it
/// has no sequence content, which a map never has.
fn as_map<T: ReadTxn>(txn: &T, root: Out) -> Out {
    match root {
        Out::UndefinedRef(branch) => {
            if ArrayRef::from(branch).len(txn) > 0 {
                Out::Any(Any::Undefined)
            } else {
                Out::YMap(MapRef::from(branch))
            }
        }
        other => other,
    }
}

fn read_record<T: ReadTxn>(txn: &T, record: &MapRef) -> Result<PageRecord, String> {
    let get = |name: &str| record.get(txn, name);
    let title = match get(key::TITLE) {
        Some(Out::Any(Any::String(title))) => title.to_string(),
        other => return Err(expected(key::TITLE, "a string", other.as_ref())),
    };
    let placement = object(key::PLACEMENT, get(key::PLACEMENT))?;
    let placement = Placement {
        parent: optional_page(key::PLACEMENT, key::PARENT_ID, &placement)?,
        order: integer_field(key::PLACEMENT, key::ORDER, &placement)?,
    };
    let favorite = match get(key::FAVORITE) {
        None | Some(Out::Any(Any::Null)) => None,
        value => {
            let favorite = object(key::FAVORITE, value)?;
            Some(Favorite {
                order: integer_field(key::FAVORITE, key::ORDER, &favorite)?,
            })
        }
    };
    let trash = match get(key::TRASH) {
        None | Some(Out::Any(Any::Null)) => None,
        value => {
            let trash = object(key::TRASH, value)?;
            Some(Trash {
                original_parent: optional_page(key::TRASH, key::ORIGINAL_PARENT_ID, &trash)?,
                original_order: integer_field(key::TRASH, key::ORIGINAL_ORDER, &trash)?,
                batch_id: match trash.get(key::BATCH_ID) {
                    Some(Any::String(batch)) if !batch.is_empty() => batch.to_string(),
                    _ => {
                        return Err(format!(
                            "{}.{} is not a non-empty string",
                            key::TRASH,
                            key::BATCH_ID
                        ));
                    }
                },
                trashed_at: integer_field(key::TRASH, key::TRASHED_AT, &trash)?,
            })
        }
    };
    Ok(PageRecord {
        title,
        placement,
        favorite,
        trash,
        created_at: integer(key::CREATED_AT, get(key::CREATED_AT).as_ref())?,
        updated_at: integer(key::UPDATED_AT, get(key::UPDATED_AT).as_ref())?,
    })
}

type Object = std::collections::HashMap<String, Any>;

fn object(name: &str, value: Option<Out>) -> Result<Object, String> {
    match value {
        Some(Out::Any(Any::Map(map))) => Ok((*map).clone()),
        other => Err(expected(name, "an object", other.as_ref())),
    }
}

fn optional_page(object: &str, field: &str, fields: &Object) -> Result<Option<PageId>, String> {
    match fields.get(field) {
        Some(Any::Null) => Ok(None),
        Some(Any::String(id)) => PageId::new(id.to_string())
            .map(Some)
            .map_err(|e| format!("{object}.{field} is not a page ID ({e})")),
        _ => Err(format!("{object}.{field} is not a page ID or null")),
    }
}

fn integer_field(object: &str, field: &str, fields: &Object) -> Result<u64, String> {
    safe_integer(fields.get(field))
        .ok_or_else(|| format!("{object}.{field} is not an integer from 0 to 2^53 - 1"))
}

fn integer(name: &str, value: Option<&Out>) -> Result<u64, String> {
    match value {
        Some(Out::Any(any)) => safe_integer(Some(any)),
        _ => None,
    }
    .ok_or_else(|| expected(name, "an integer from 0 to 2^53 - 1", value))
}

/// Yjs stores a JS number as an integer or a float, depending on its size.
pub(crate) fn safe_integer(value: Option<&Any>) -> Option<u64> {
    let n = match value? {
        Any::Number(yrs::any::Number::Int(n)) => u64::try_from(*n).ok()?,
        Any::Number(yrs::any::Number::Float(f)) if f.fract() == 0.0 && *f >= 0.0 => {
            if *f > MAX_SAFE_INTEGER as f64 {
                return None;
            }
            *f as u64
        }
        _ => return None,
    };
    (n <= MAX_SAFE_INTEGER).then_some(n)
}

fn expected(name: &str, what: &str, found: Option<&Out>) -> String {
    let found = match found {
        None => "nothing",
        Some(Out::Any(any)) => match any {
            Any::Null => "null",
            Any::Undefined => "undefined",
            Any::Bool(_) => "a boolean",
            Any::Number(_) => "a number",
            Any::String(_) => "a string",
            Any::Buffer(_) => "a buffer",
            Any::Array(_) => "an array",
            Any::Map(_) => "an object",
        },
        Some(_) => "a shared type",
    };
    format!("{name} is not {what}, found {found}")
}
