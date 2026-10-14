use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use yrs::types::Attrs;
use yrs::updates::decoder::Decode;
use yrs::{
    Any, Doc, Map, MapPrelim, MapRef, Out, ReadTxn, Text, Transact, TransactionMut, WriteTxn, Xml,
    XmlElementPrelim, XmlElementRef, XmlFragment, XmlFragmentRef, XmlTextPrelim,
};

use super::body::{BODY_ROOT, node};
use super::records::{PAGES, PageId, PageRecord, key};
use crate::storage::DocId;

/// Local copies of docs that turn each edit into the Yjs update the app would append.
pub struct Editor {
    client: u64,
    docs: BTreeMap<DocId, Doc>,
}

impl Editor {
    pub fn new(client: u64) -> Self {
        Self {
            client,
            docs: BTreeMap::new(),
        }
    }

    /// Runs `f` on this editor's copy of `doc` and returns the update it made.
    pub fn edit(&mut self, doc: &DocId, f: impl FnOnce(&mut TransactionMut)) -> Vec<u8> {
        let client = self.client;
        let ydoc = self
            .docs
            .entry(doc.clone())
            .or_insert_with(|| Doc::with_client_id(client));
        let mut txn = ydoc.transact_mut();
        f(&mut txn);
        txn.encode_update_v1()
    }

    /// Applies `update` to this editor's copy of `doc`, as receiving it from another editor.
    pub fn apply(&mut self, doc: &DocId, update: &[u8]) {
        let client = self.client;
        let ydoc = self
            .docs
            .entry(doc.clone())
            .or_insert_with(|| Doc::with_client_id(client));
        let update = yrs::Update::decode_v1(update).expect("a valid update");
        ydoc.transact_mut()
            .apply_update(update)
            .expect("the update applies");
    }

    /// The full state of this editor's copy of `doc`.
    pub fn state(&self, doc: &DocId) -> Vec<u8> {
        self.docs.get(doc).map_or_else(
            || {
                Doc::new()
                    .transact()
                    .encode_state_as_update_v1(&Default::default())
            },
            |ydoc| {
                ydoc.transact()
                    .encode_state_as_update_v1(&Default::default())
            },
        )
    }
}

fn pages(txn: &mut TransactionMut) -> MapRef {
    txn.get_or_insert_map(PAGES)
}

/// Writes `record` as page `id`'s record, keeping every key and object field it does not know.
pub fn put_page(txn: &mut TransactionMut, id: &PageId, record: &PageRecord) {
    let pages = pages(txn);
    let map = match pages.get(txn, id.as_str()) {
        Some(Out::YMap(map)) => map,
        _ => pages.insert(txn, id.as_str(), MapPrelim::default()),
    };
    map.insert(txn, key::TITLE, record.title.as_str());
    let placement = merge(
        &map,
        txn,
        key::PLACEMENT,
        [
            (
                key::PARENT_ID,
                page_or_null(record.placement.parent.as_ref()),
            ),
            (key::ORDER, number(record.placement.order)),
        ],
    );
    map.insert(txn, key::PLACEMENT, placement);
    let favorite = match &record.favorite {
        None => Any::Null,
        Some(favorite) => merge(
            &map,
            txn,
            key::FAVORITE,
            [(key::ORDER, number(favorite.order))],
        ),
    };
    map.insert(txn, key::FAVORITE, favorite);
    let trash = match &record.trash {
        None => Any::Null,
        Some(trash) => merge(
            &map,
            txn,
            key::TRASH,
            [
                (
                    key::ORIGINAL_PARENT_ID,
                    page_or_null(trash.original_parent.as_ref()),
                ),
                (key::ORIGINAL_ORDER, number(trash.original_order)),
                (key::BATCH_ID, Any::from(trash.batch_id.as_str())),
                (key::TRASHED_AT, number(trash.trashed_at)),
            ],
        ),
    };
    map.insert(txn, key::TRASH, trash);
    map.insert(txn, key::CREATED_AT, number(record.created_at));
    map.insert(txn, key::UPDATED_AT, number(record.updated_at));
}

/// Removes page `id`'s record, as erasing does (TREE-14).
pub fn erase_page(txn: &mut TransactionMut, id: &PageId) {
    pages(txn).remove(txn, id.as_str());
}

fn merge<const N: usize>(
    map: &MapRef,
    txn: &TransactionMut,
    name: &str,
    fields: [(&str, Any); N],
) -> Any {
    let mut object: HashMap<String, Any> = match map.get(txn, name) {
        Some(Out::Any(Any::Map(old))) => (*old).clone(),
        _ => HashMap::new(),
    };
    for (field, value) in fields {
        object.insert(field.to_owned(), value);
    }
    Any::from(object)
}

fn number(n: u64) -> Any {
    Any::Number(yrs::any::Number::Int(n as i64))
}

fn page_or_null(page: Option<&PageId>) -> Any {
    page.map_or(Any::Null, |p| Any::from(p.as_str()))
}

/// A block as BlockNote's binding stores it: `kind` is its content node's name.
#[derive(Clone, Debug)]
pub struct BlockSpec {
    pub id: String,
    pub kind: String,
    pub attrs: Vec<(String, Any)>,
    pub content: Vec<Inline>,
    pub children: Vec<BlockSpec>,
}

#[derive(Clone, Debug)]
pub enum Inline {
    Text(String),
    /// Text with one mark, stored as y-prosemirror stores marks: a formatting attribute.
    Marked(String, String),
    Link(PageId),
    HardBreak,
}

pub fn block(id: &str, kind: &str, content: Vec<Inline>) -> BlockSpec {
    BlockSpec {
        id: id.to_owned(),
        kind: kind.to_owned(),
        attrs: Vec::new(),
        content,
        children: Vec::new(),
    }
}

pub fn paragraph(id: &str, content: Vec<Inline>) -> BlockSpec {
    block(id, "paragraph", content)
}

pub fn text(t: &str) -> Inline {
    Inline::Text(t.to_owned())
}

pub fn link(target: &str) -> Inline {
    Inline::Link(PageId::new(target).expect("a valid page ID"))
}

impl BlockSpec {
    pub fn with_children(mut self, children: Vec<BlockSpec>) -> Self {
        self.children = children;
        self
    }

    pub fn with_attr(mut self, name: &str, value: impl Into<Any>) -> Self {
        self.attrs.push((name.to_owned(), value.into()));
        self
    }
}

fn body(txn: &mut TransactionMut) -> XmlFragmentRef {
    txn.get_or_insert_xml_fragment(BODY_ROOT)
}

fn top_group(txn: &mut TransactionMut) -> XmlElementRef {
    let fragment = body(txn);
    match fragment.get(txn, 0) {
        Some(yrs::XmlOut::Element(group)) => group,
        _ => fragment.insert(txn, 0, XmlElementPrelim::empty(node::BLOCK_GROUP)),
    }
}

/// Replaces the whole body with `blocks`.
pub fn write_body(txn: &mut TransactionMut, blocks: &[BlockSpec]) {
    let fragment = body(txn);
    let len = fragment.len(txn);
    fragment.remove_range(txn, 0, len);
    let group = fragment.insert(txn, 0, XmlElementPrelim::empty(node::BLOCK_GROUP));
    for (i, spec) in blocks.iter().enumerate() {
        insert_into(txn, &group, i as u32, spec);
    }
}

/// Inserts `spec` as the top-level block at `index`.
pub fn insert_block(txn: &mut TransactionMut, index: u32, spec: &BlockSpec) {
    let group = top_group(txn);
    insert_into(txn, &group, index, spec);
}

/// Removes the top-level block at `index`.
pub fn remove_block(txn: &mut TransactionMut, index: u32) {
    let group = top_group(txn);
    group.remove_range(txn, index, 1);
}

fn insert_into(txn: &mut TransactionMut, group: &XmlElementRef, index: u32, spec: &BlockSpec) {
    let container = group.insert(txn, index, XmlElementPrelim::empty(node::BLOCK_CONTAINER));
    container.insert_attribute(txn, node::ID, spec.id.as_str());
    let content = container.insert(txn, 0, XmlElementPrelim::empty(spec.kind.as_str()));
    for (name, value) in &spec.attrs {
        content.insert_attribute(txn, name.as_str(), value.clone());
    }
    let mut at = 0;
    let mut run: Option<yrs::XmlTextRef> = None;
    for inline in &spec.content {
        match inline {
            Inline::Text(t) | Inline::Marked(t, _) => {
                let attrs: Attrs = match inline {
                    Inline::Marked(_, mark) => HashMap::from([(
                        Arc::from(mark.as_str()),
                        Any::from(HashMap::<String, Any>::new()),
                    )]),
                    _ => HashMap::new(),
                };
                let text = match &run {
                    Some(text) => text.clone(),
                    None => {
                        let text = content.insert(txn, at, XmlTextPrelim::new(""));
                        at += 1;
                        run = Some(text.clone());
                        text
                    }
                };
                let end = text.len(txn);
                text.insert_with_attributes(txn, end, t, attrs);
            }
            Inline::Link(target) => {
                let link = content.insert(txn, at, XmlElementPrelim::empty(node::PAGE_LINK));
                link.insert_attribute(txn, node::TARGET_PAGE_ID, target.as_str());
                at += 1;
                run = None;
            }
            Inline::HardBreak => {
                content.insert(txn, at, XmlElementPrelim::empty(node::HARD_BREAK));
                at += 1;
                run = None;
            }
        }
    }
    if !spec.children.is_empty() {
        let nested = container.insert(txn, 1, XmlElementPrelim::empty(node::BLOCK_GROUP));
        for (i, child) in spec.children.iter().enumerate() {
            insert_into(txn, &nested, i as u32, child);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrokenIndex {
    Missing,
    Garbage,
    OldSchema,
}

/// Leaves the index at `path` in the `broken` state. Removes only the index file and its
/// sidecars.
pub fn break_index(path: &Path, broken: BrokenIndex) -> std::io::Result<()> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        match std::fs::remove_file(&name) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    match broken {
        BrokenIndex::Missing => Ok(()),
        BrokenIndex::Garbage => std::fs::write(path, b"this is not a SQLite database\n".repeat(64)),
        BrokenIndex::OldSchema => {
            let conn = rusqlite::Connection::open(path).map_err(std::io::Error::other)?;
            conn.execute_batch(
                "CREATE TABLE pages (id TEXT PRIMARY KEY, title TEXT);
                 INSERT INTO pages VALUES ('stale', 'A stale cached title');
                 PRAGMA user_version = 0;",
            )
            .map_err(std::io::Error::other)
        }
    }
}

/// The index fixture's pages: nesting, favorites, a trashed subtree with a favorite inside,
/// an empty title, an erased page whose body files remain, links to every kind of target,
/// marks, a hard break, a code block, and an image caption.
pub fn index_fixture() -> Vec<(DocId, Vec<u8>)> {
    use super::records::{Favorite, Placement, Trash};
    let record = |title: &str, parent: Option<&str>, order: u64, updated_at: u64| PageRecord {
        title: title.to_owned(),
        placement: Placement {
            parent: parent.map(|p| PageId::new(p).expect("a valid page ID")),
            order,
        },
        favorite: None,
        trash: None,
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000 + updated_at,
    };
    let trash = |mut r: PageRecord| {
        r.trash = Some(Trash {
            original_parent: r.placement.parent.clone(),
            original_order: r.placement.order,
            batch_id: "batch-archive".to_owned(),
            trashed_at: 1_700_000_900_000,
        });
        r
    };
    let favorite = |mut r: PageRecord, order: u64| {
        r.favorite = Some(Favorite { order });
        r
    };
    let pages = [
        ("projects", record("Projects", None, 0, 100)),
        (
            "aelomi",
            favorite(record("Aelomi", Some("projects"), 0, 300), 1),
        ),
        ("roadmap", record("Roadmap", Some("aelomi"), 0, 200)),
        (
            "reading",
            favorite(record("Reading list", Some("projects"), 1, 150), 0),
        ),
        ("daily", record("Daily", None, 1, 500)),
        ("archive", trash(record("Archive", None, 2, 400))),
        (
            "old-notes",
            favorite(trash(record("Old notes", Some("archive"), 0, 450)), 2),
        ),
        ("untitled", record("", None, 3, 350)),
        ("erased", record("Erased page", None, 4, 50)),
    ];
    let bodies = [
        (
            "projects",
            vec![
                paragraph("p1", vec![text("project index")]),
                block("p2", node::IMAGE, vec![])
                    .with_attr(node::CAPTION, "Project diagram")
                    .with_attr("url", "asset://diagram.png"),
            ],
        ),
        (
            "aelomi",
            vec![
                block("a1", "heading", vec![text("Aelomi app")]).with_attr("level", 1.0),
                block(
                    "a2",
                    "codeBlock",
                    vec![text("fn kestrel() {\n    // [[not a link]]\n}")],
                )
                .with_attr("language", "rust"),
                paragraph(
                    "a3",
                    vec![
                        text("links to "),
                        link("roadmap"),
                        text(" and "),
                        link("untitled"),
                    ],
                ),
            ],
        ),
        (
            "roadmap",
            vec![
                paragraph("r1", vec![text("Milestones for "), link("aelomi")]),
                paragraph(
                    "r2",
                    vec![text("line one"), Inline::HardBreak, text("line two")],
                ),
            ],
        ),
        (
            "reading",
            vec![paragraph(
                "rd1",
                vec![
                    text("Books about "),
                    Inline::Marked("café".into(), "bold".into()),
                    text(" culture"),
                ],
            )],
        ),
        (
            "daily",
            vec![
                paragraph("d1", vec![text("see "), link("aelomi"), text(" today")]),
                paragraph("d2", vec![text("quarterly roadmap review")]),
                block("d3", "bulletListItem", vec![text("tasks")]).with_children(vec![
                    block("d4", "bulletListItem", vec![text("call "), link("reading")]),
                    block("d5", "checkListItem", vec![text("read "), link("erased")])
                        .with_attr("checked", false),
                ]),
                paragraph("d6", vec![link("archive"), text(" is archived")]),
                paragraph("d7", vec![link("aelomi"), text(" and "), link("aelomi")]),
                block("d8", "divider", vec![]),
            ],
        ),
        (
            "archive",
            vec![paragraph(
                "ar1",
                vec![text("archived thoughts about "), link("aelomi")],
            )],
        ),
        (
            "old-notes",
            vec![paragraph(
                "on1",
                vec![text("quillmere in old notes "), link("daily")],
            )],
        ),
        (
            "untitled",
            vec![paragraph(
                "u1",
                vec![text("untitled body mentions "), link("daily")],
            )],
        ),
        (
            "erased",
            vec![paragraph("e1", vec![text("orphanword "), link("daily")])],
        ),
    ];
    let mut editor = Editor::new(7);
    let workspace = DocId::workspace();
    let mut updates = vec![(
        workspace.clone(),
        editor.edit(&workspace, |txn| {
            for (id, record) in &pages {
                put_page(txn, &PageId::new(*id).expect("a valid page ID"), record);
            }
        }),
    )];
    for (id, blocks) in &bodies {
        let doc = DocId::new(*id).expect("a valid doc ID");
        updates.push((
            doc.clone(),
            editor.edit(&doc, |txn| write_body(txn, blocks)),
        ));
    }
    updates.push((
        workspace.clone(),
        editor.edit(&workspace, |txn| {
            erase_page(txn, &PageId::new("erased").expect("a valid page ID"))
        }),
    ));
    updates
}

/// When the first page of the search fixture was created. Page `n`, its line in the file, was
/// created `n` seconds later, so the content order is the reverse of the file order.
pub const SEARCH_CREATED_AT: u64 = 1_700_000_000_000;

/// The checked-in `search-10k-pages.jsonl` as Yjs updates: one for the workspace doc, then one
/// per page body, each block a paragraph `<page ID>-<line in the body>`. `copies` adds that many
/// renamed copies of every page, `<ID>-c<k>`, created after the originals.
pub fn search_fixture(jsonl: &str, copies: usize) -> Vec<(DocId, Vec<u8>)> {
    use super::records::{Placement, Trash};
    let lines: Vec<serde_json::Value> = jsonl
        .lines()
        .map(|line| serde_json::from_str(line).expect("a fixture line is JSON"))
        .collect();
    let mut editor = Editor::new(11);
    let mut records = Vec::new();
    let mut bodies = Vec::new();
    for copy in 0..=copies {
        let rename = |id: &str| match copy {
            0 => PageId::new(id).expect("a valid page ID"),
            k => PageId::new(format!("{id}-c{k}")).expect("a valid page ID"),
        };
        let mut siblings: HashMap<Option<PageId>, u64> = HashMap::new();
        for (line, page) in lines.iter().enumerate() {
            let id = rename(page["id"].as_str().expect("an ID"));
            let parent = page["parentId"].as_str().map(rename);
            let order = siblings.entry(parent.clone()).or_default();
            let created_at = SEARCH_CREATED_AT + ((copy * lines.len() + line) as u64) * 1000;
            let trash = page["trash"].as_object().map(|trash| Trash {
                original_parent: parent.clone(),
                original_order: *order,
                batch_id: trash["batchId"].as_str().expect("a batch").to_owned(),
                trashed_at: trash["at"].as_u64().expect("a time"),
            });
            records.push((
                id.clone(),
                PageRecord {
                    title: page["title"].as_str().expect("a title").to_owned(),
                    placement: Placement {
                        parent,
                        order: *order,
                    },
                    favorite: None,
                    trash,
                    created_at,
                    updated_at: created_at,
                },
            ));
            *order += 1;
            let blocks: Vec<BlockSpec> = page["body"]
                .as_str()
                .expect("a body")
                .split('\n')
                .enumerate()
                .map(|(i, text)| paragraph(&format!("{id}-{i}"), vec![Inline::Text(text.into())]))
                .collect();
            let doc = id.doc_id().clone();
            bodies.push((
                doc.clone(),
                editor.edit(&doc, |txn| write_body(txn, &blocks)),
            ));
        }
    }
    let workspace = DocId::workspace();
    let update = editor.edit(&workspace, |txn| {
        for (id, record) in &records {
            put_page(txn, id, record);
        }
    });
    let mut updates = vec![(workspace, update)];
    updates.extend(bodies);
    updates
}

/// A page body in BlockNote's JSON block format, such as `page-5000-blocks.json`, as block
/// specs. Props become attributes; styles and URL links become marks.
pub fn blocknote_blocks(blocks: &serde_json::Value) -> Vec<BlockSpec> {
    let Some(blocks) = blocks.as_array() else {
        return Vec::new();
    };
    blocks
        .iter()
        .map(|block| {
            let mut spec = BlockSpec {
                id: block["id"].as_str().unwrap_or_default().to_owned(),
                kind: block["type"].as_str().unwrap_or("paragraph").to_owned(),
                attrs: Vec::new(),
                content: Vec::new(),
                children: blocknote_blocks(&block["children"]),
            };
            if let Some(props) = block["props"].as_object() {
                for (name, value) in props {
                    let value = match value {
                        serde_json::Value::String(s) => Any::from(s.as_str()),
                        serde_json::Value::Bool(b) => Any::from(*b),
                        serde_json::Value::Number(n) => Any::from(n.as_f64().unwrap_or(0.0)),
                        _ => continue,
                    };
                    spec.attrs.push((name.clone(), value));
                }
            }
            for inline in block["content"].as_array().into_iter().flatten() {
                let runs = match inline["type"].as_str() {
                    Some("link") => inline["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|run| (run, Some("link")))
                        .collect(),
                    _ => vec![(inline, None)],
                };
                for (run, mark) in runs {
                    let text = run["text"].as_str().unwrap_or_default().to_owned();
                    let style = run["styles"]
                        .as_object()
                        .and_then(|styles| styles.keys().next().cloned());
                    spec.content.push(match mark.map(str::to_owned).or(style) {
                        Some(mark) => Inline::Marked(text, mark),
                        None => Inline::Text(text),
                    });
                }
            }
            spec
        })
        .collect()
}
