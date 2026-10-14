use aelomi_core::index::{self as core, Reply};
use aelomi_core::workspace::records::PageId;
use aelomi_core::workspace::tree::Tree;
use aelomi_core::workspace::{TreeError, Workspace};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use std::time::Instant;
use tauri::State;

use crate::storage::Storage;
use crate::trace;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum IndexStatus {
    Ready,
    Rebuilding,
    Failed { reason: String },
}

/// The initial index status and every later status change.
#[derive(Clone, Serialize, Type, tauri_specta::Event)]
pub struct IndexStatusChanged(pub IndexStatus);

/// A committed index batch: search and backlink results may differ after it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type, tauri_specta::Event)]
pub struct IndexChanged {
    /// The batch applied workspace changes: titles, the tree, or trash.
    pub workspace: bool,
    /// Pages whose body was indexed again, sorted.
    pub pages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum QueryError {
    /// The store is not open. `storage_status` says why.
    Unavailable,
    /// A page ID argument, or one in `exclude`, is not a valid page ID.
    InvalidPageId,
    InvalidQuery(InvalidQuery),
    /// The index failed, or the workspace doc has a malformed record.
    Failed {
        reason: String,
    },
    Io(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum InvalidQuery {
    /// `exclude` names more distinct pages than `max`.
    TooManyExcluded { max: u32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct PageTree {
    /// Every active page, depth-first, parents before children.
    pub pages: Vec<TreePage>,
    /// Active favorites in favorite order.
    pub favorites: Vec<String>,
    /// The pages the user trashed, without their descendants, newest first.
    pub trash: Vec<TrashedPage>,
}

/// Times are milliseconds since the Unix epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct TreePage {
    pub id: String,
    /// Empty for an untitled page.
    pub title: String,
    pub parent_id: Option<String>,
    pub created_at: f64,
    pub updated_at: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct TrashedPage {
    pub id: String,
    pub title: String,
    pub batch_id: String,
    pub trashed_at: f64,
}

/// Which pages a search returns. Both order pages newest created first.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Type)]
pub enum SearchScope {
    /// Pages whose title matches, then pages whose body matches.
    All,
    /// Pages whose body matches, except `exclude`, the palette's title group: at most 200
    /// distinct page IDs.
    Content { exclude: Vec<String> },
}

/// `Indexing` until the index can answer from current data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum SearchReply {
    Indexing,
    /// `issues` lists every active page the index holds only in part, sorted by page ID.
    Ready {
        hits: Vec<SearchHit>,
        issues: Vec<PageIssue>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct SearchHit {
    pub page_id: String,
    /// The raw title, highlighted where it matched.
    pub title: Vec<TextPart>,
    /// The page's first matching block. Always set in a `Content` search; null for a page that
    /// matched by title.
    pub block: Option<BlockHit>,
}

/// An active page whose body the index could read only in part. Its doc is unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct PageIssue {
    pub page_id: String,
    pub problem: PageProblem,
    /// The blocks the index could read.
    pub indexed_blocks: u32,
}

/// The most serious problem in the page body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum PageProblem {
    /// The doc is not a valid Yjs update.
    Undecodable,
    /// The body is under `root`, not `prosemirror`.
    UnrecognizedRoot { root: String },
    /// A block has no ID. Its children were read.
    MissingBlockId,
    /// A page link has no valid target. It reads as a space.
    InvalidPageLink,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct BlockHit {
    pub block_id: String,
    pub text: Vec<TextPart>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct TextPart {
    pub text: String,
    pub highlighted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum BacklinksReply {
    Indexing,
    /// `issues` as in `SearchReply`.
    Ready {
        backlinks: Vec<Backlink>,
        issues: Vec<PageIssue>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct Backlink {
    pub page_id: String,
    pub title: String,
    pub blocks: Vec<BacklinkBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct BacklinkBlock {
    pub block_id: String,
    /// Links show the target's current title; `\n` is a hard break or a code newline.
    pub text: String,
}

/// Never touches SQLite, so it answers at once on any thread.
#[tauri::command]
#[specta::specta]
pub fn index_status(storage: State<'_, Storage>) -> IndexStatus {
    storage.index_status()
}

/// Reads the workspace doc, not the index, so it works while the index rebuilds.
#[tauri::command]
#[specta::specta]
pub async fn page_tree(storage: State<'_, Storage>) -> Result<PageTree, QueryError> {
    off_main(storage.workspace()?, |w| {
        let started = Instant::now();
        let tree: PageTree = w.tree()?.into();
        trace::ipc("page_tree", started, || {
            format!("pages={} bytes={}", tree.pages.len(), json_len(&tree))
        });
        Ok(tree)
    })
    .await
}

/// Active pages matching `query`, whose last word matches as a prefix. `limit` is clamped to 1
/// through 200.
#[tauri::command]
#[specta::specta]
pub async fn search(
    storage: State<'_, Storage>,
    query: String,
    limit: u32,
    scope: SearchScope,
) -> Result<SearchReply, QueryError> {
    let exclude = match &scope {
        SearchScope::All => Vec::new(),
        SearchScope::Content { exclude } => exclude
            .iter()
            .map(|id| PageId::new(id.as_str()).map_err(|_| QueryError::InvalidPageId))
            .collect::<Result<Vec<_>, _>>()?,
    };
    off_main(storage.workspace()?, move |w| {
        let scope = match scope {
            SearchScope::All => core::SearchScope::All,
            SearchScope::Content { .. } => core::SearchScope::Content { exclude: &exclude },
        };
        let started = Instant::now();
        let found = w.search(&core::SearchQuery {
            text: &query,
            limit: limit as usize,
            scope,
        })?;
        let core_time = started.elapsed();
        let reply = match found {
            Reply::Indexing => SearchReply::Indexing,
            Reply::Ready { value, issues } => SearchReply::Ready {
                hits: value.into_iter().map(Into::into).collect(),
                issues: issues.iter().map(Into::into).collect(),
            },
        };
        let dto_time = started.elapsed() - core_time;
        trace::ipc("search", started, || {
            let hits = match &reply {
                SearchReply::Indexing => "indexing".to_owned(),
                SearchReply::Ready { hits, .. } => hits.len().to_string(),
            };
            format!(
                "core_ms={} dto_ms={} hits={hits} bytes={}",
                trace::ms(core_time),
                trace::ms(dto_time),
                json_len(&reply)
            )
        });
        Ok(reply)
    })
    .await
}

/// Active pages linking to `page_id`, most recently updated first.
#[tauri::command]
#[specta::specta]
pub async fn backlinks(
    storage: State<'_, Storage>,
    page_id: String,
) -> Result<BacklinksReply, QueryError> {
    let target = PageId::new(page_id).map_err(|_| QueryError::InvalidPageId)?;
    off_main(storage.workspace()?, move |w| {
        let started = Instant::now();
        let reply = match w.backlinks(&target)? {
            Reply::Indexing => BacklinksReply::Indexing,
            Reply::Ready { value, issues } => BacklinksReply::Ready {
                backlinks: value.into_iter().map(Into::into).collect(),
                issues: issues.iter().map(Into::into).collect(),
            },
        };
        trace::ipc("backlinks", started, || {
            format!("bytes={}", json_len(&reply))
        });
        Ok(reply)
    })
    .await
}

fn json_len(value: &impl Serialize) -> usize {
    serde_json::to_vec(value).map_or(0, |bytes| bytes.len())
}

async fn off_main<T: Send + 'static>(
    workspace: Arc<Workspace>,
    f: impl FnOnce(&Workspace) -> Result<T, QueryError> + Send + 'static,
) -> Result<T, QueryError> {
    tauri::async_runtime::spawn_blocking(move || f(&workspace))
        .await
        .map_err(|e| QueryError::Io(e.to_string()))?
}

impl From<core::IndexStatus> for IndexStatus {
    fn from(status: core::IndexStatus) -> Self {
        match status {
            core::IndexStatus::Ready => Self::Ready,
            core::IndexStatus::Rebuilding => Self::Rebuilding,
            core::IndexStatus::Failed { reason } => Self::Failed { reason },
        }
    }
}

impl From<core::IndexChange> for IndexChanged {
    fn from(change: core::IndexChange) -> Self {
        Self {
            workspace: change.workspace,
            pages: change.pages.iter().map(|p| p.to_string()).collect(),
        }
    }
}

impl From<core::QueryError> for QueryError {
    fn from(error: core::QueryError) -> Self {
        match error {
            core::QueryError::Failed(reason) => Self::Failed { reason },
            core::QueryError::InvalidQuery(core::InvalidQuery::TooManyExcluded { max }) => {
                Self::InvalidQuery(InvalidQuery::TooManyExcluded { max: max as u32 })
            }
            core::QueryError::Sql(e) => Self::Io(e),
        }
    }
}

impl From<TreeError> for QueryError {
    fn from(error: TreeError) -> Self {
        match error {
            TreeError::Io(e) => Self::Io(e.to_string()),
            TreeError::Malformed(e) => Self::Failed {
                reason: e.to_string(),
            },
        }
    }
}

impl From<Arc<Tree>> for PageTree {
    fn from(tree: Arc<Tree>) -> Self {
        Self {
            pages: tree
                .pages
                .iter()
                .map(|p| TreePage {
                    id: p.id.to_string(),
                    title: p.title.clone(),
                    parent_id: p.parent.as_ref().map(|p| p.to_string()),
                    created_at: p.created_at as f64,
                    updated_at: p.updated_at as f64,
                })
                .collect(),
            favorites: tree.favorites.iter().map(|p| p.to_string()).collect(),
            trash: tree
                .trash
                .iter()
                .map(|t| TrashedPage {
                    id: t.id.to_string(),
                    title: t.title.clone(),
                    batch_id: t.batch_id.clone(),
                    trashed_at: t.trashed_at as f64,
                })
                .collect(),
        }
    }
}

impl From<&core::PageIssue> for PageIssue {
    fn from(issue: &core::PageIssue) -> Self {
        Self {
            page_id: issue.page_id.to_string(),
            problem: match &issue.problem {
                core::PageProblem::Undecodable => PageProblem::Undecodable,
                core::PageProblem::UnrecognizedRoot(root) => {
                    PageProblem::UnrecognizedRoot { root: root.clone() }
                }
                core::PageProblem::MissingBlockId => PageProblem::MissingBlockId,
                core::PageProblem::InvalidPageLink => PageProblem::InvalidPageLink,
            },
            indexed_blocks: issue.indexed_blocks,
        }
    }
}

fn parts(parts: Vec<core::TextPart>) -> Vec<TextPart> {
    parts
        .into_iter()
        .map(|p| TextPart {
            text: p.text,
            highlighted: p.highlighted,
        })
        .collect()
}

impl From<core::SearchHit> for SearchHit {
    fn from(hit: core::SearchHit) -> Self {
        Self {
            page_id: hit.page_id.to_string(),
            title: parts(hit.title),
            block: hit.block.map(|b| BlockHit {
                block_id: b.block_id.as_str().to_owned(),
                text: parts(b.text),
            }),
        }
    }
}

impl From<core::Backlink> for Backlink {
    fn from(link: core::Backlink) -> Self {
        Self {
            page_id: link.page_id.to_string(),
            title: link.title,
            blocks: link
                .blocks
                .into_iter()
                .map(|b| BacklinkBlock {
                    block_id: b.block_id.as_str().to_owned(),
                    text: b.text,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use serde_json::{Value, json};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{
        INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
    };
    use tauri::webview::InvokeRequest;
    use tauri::{App, Listener, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

    use super::*;
    use crate::storage::DataLocation;

    struct Shell {
        app: App<MockRuntime>,
        webview: WebviewWindow<MockRuntime>,
        events: Arc<Mutex<Vec<String>>>,
    }

    fn shell(dir: Option<&std::path::Path>) -> Shell {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                index_status,
                page_tree,
                search,
                backlinks
            ])
            .build(mock_context(noop_assets()))
            .expect("builds the mock app");
        tauri_specta::Builder::<MockRuntime>::new()
            .events(tauri_specta::collect_events![
                IndexStatusChanged,
                IndexChanged
            ])
            .mount_events(&app);
        let events = Arc::new(Mutex::new(Vec::new()));
        for name in ["index-status-changed", "index-changed"] {
            let seen = events.clone();
            app.listen_any(name, move |event| {
                seen.lock()
                    .unwrap()
                    .push(format!("{name} {}", event.payload()));
            });
        }
        let handle = app.handle().clone();
        let storage = match dir {
            Some(dir) => Storage::open(
                &DataLocation {
                    path: dir.to_path_buf(),
                    cloud_roots: vec![],
                },
                move |event| crate::publish_index(&handle, event),
            ),
            None => Storage::not_open(),
        };
        app.manage(storage);
        let webview = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .expect("builds the webview");
        Shell {
            app,
            webview,
            events,
        }
    }

    fn invoke(shell: &Shell, cmd: &str, args: Value) -> Result<Value, Value> {
        get_ipc_response(
            &shell.webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    }

    fn wait_for(shell: &Shell, f: impl Fn(&Shell) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !f(shell) {
            assert!(Instant::now() < deadline, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn index_commands_and_events_round_trip_over_ipc() {
        let dir = tempfile::tempdir().unwrap();
        let store = aelomi_core::storage::Store::open(dir.path()).unwrap();
        for (doc, update) in aelomi_core::workspace::fixture::index_fixture() {
            store.append(&doc, &update).unwrap();
        }
        assert!(store.flush(Instant::now() + Duration::from_secs(2)).is_empty());
        assert!(store.close(Instant::now() + Duration::from_secs(2)).is_ok());
        let shell = shell(Some(dir.path()));
        let workspace = shell.app.state::<Storage>().workspace().unwrap();
        let cafe =
            json!({ "query": "cafe", "limit": 5, "scope": { "Content": { "exclude": [] } } });
        wait_for(&shell, |s| {
            invoke(s, "index_status", json!({})) == Ok(json!("Ready"))
                && invoke(s, "search", cafe.clone())
                    != Ok(json!({ "Ready": { "hits": [], "issues": [] } }))
        });

        assert_eq!(
            invoke(&shell, "search", cafe),
            Ok(json!({ "Ready": { "hits": [{
                "page_id": "reading",
                "title": [{ "text": "Reading list", "highlighted": false }],
                "block": {
                    "block_id": "rd1",
                    "text": [
                        { "text": "Books about ", "highlighted": false },
                        { "text": "café", "highlighted": true },
                        { "text": " culture", "highlighted": false },
                    ],
                },
            }], "issues": [] } }))
        );
        assert_eq!(
            invoke(
                &shell,
                "search",
                json!({ "query": "roadm", "limit": 5, "scope": "All" })
            )
            .unwrap()["Ready"]["hits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|hit| (hit["page_id"].clone(), hit["block"].is_null()))
                .collect::<Vec<_>>(),
            [
                (json!("roadmap"), true),
                (json!("aelomi"), false),
                (json!("daily"), false)
            ],
            "titles first, then bodies, each newest first then by ID"
        );
        assert_eq!(
            invoke(
                &shell,
                "search",
                json!({ "query": "roadm", "limit": 5, "scope": { "Content": { "exclude": ["aelomi", "aelomi"] } } })
            )
            .unwrap()["Ready"]["hits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|hit| hit["page_id"].clone())
                .collect::<Vec<_>>(),
            [json!("daily")]
        );
        assert_eq!(
            invoke(
                &shell,
                "search",
                json!({ "query": "x", "limit": 5, "scope": { "Content": { "exclude": ["Not An ID"] } } })
            ),
            Err(json!("InvalidPageId"))
        );
        let too_many: Vec<String> = (0..201).map(|n| format!("p{n}")).collect();
        assert_eq!(
            invoke(
                &shell,
                "search",
                json!({ "query": "x", "limit": 5, "scope": { "Content": { "exclude": too_many } } })
            ),
            Err(json!({ "InvalidQuery": { "TooManyExcluded": { "max": 200 } } }))
        );
        assert_eq!(
            invoke(&shell, "backlinks", json!({ "pageId": "reading" })),
            Ok(json!({ "Ready": { "backlinks": [{
                "page_id": "daily",
                "title": "Daily",
                "blocks": [{ "block_id": "d4", "text": "call Reading list" }],
            }], "issues": [] } }))
        );
        assert_eq!(
            invoke(&shell, "backlinks", json!({ "pageId": "workspace" })),
            Err(json!("InvalidPageId"))
        );
        let tree = invoke(&shell, "page_tree", json!({})).unwrap();
        assert_eq!(
            tree["pages"][1],
            json!({
                "id": "aelomi",
                "title": "Aelomi",
                "parent_id": "projects",
                "created_at": 1_700_000_000_000.0,
                "updated_at": 1_700_000_000_300.0,
            })
        );
        assert_eq!(tree["favorites"], json!(["reading", "aelomi"]));
        assert_eq!(
            tree["trash"],
            json!([{
                "id": "archive",
                "title": "Archive",
                "batch_id": "batch-archive",
                "trashed_at": 1_700_000_900_000.0,
            }])
        );
        use aelomi_core::storage::DocId;
        use aelomi_core::workspace::fixture::{Editor, insert_block, paragraph, text};
        let daily = DocId::new("daily").unwrap();
        let mut editor = Editor::new(9);
        editor.apply(&daily, &workspace.load(&daily).unwrap());
        let edit = editor.edit(&daily, |txn| {
            insert_block(txn, 0, &paragraph("new", vec![text("ipcword")]))
        });
        workspace.append(&daily, &edit).unwrap();
        wait_for(&shell, |s| s.events.lock().unwrap().len() >= 3);
        assert_eq!(
            *shell.events.lock().unwrap(),
            [
                "index-status-changed \"Rebuilding\"",
                "index-status-changed \"Ready\"",
                "index-changed {\"workspace\":false,\"pages\":[\"daily\"]}",
            ]
        );
        assert_eq!(
            invoke(
                &shell,
                "search",
                json!({ "query": "ipcword", "limit": 5, "scope": "All" })
            )
            .unwrap()["Ready"]["hits"][0]["page_id"],
            json!("daily")
        );
    }

    #[test]
    fn index_commands_without_a_data_folder_say_why() {
        let shell = shell(None);
        assert_eq!(
            invoke(&shell, "index_status", json!({})),
            Ok(json!({ "Failed": { "reason": "the data folder is not open" } }))
        );
        for (cmd, args) in [
            (
                "search",
                json!({ "query": "x", "limit": 5, "scope": "All" }),
            ),
            ("backlinks", json!({ "pageId": "x" })),
            ("page_tree", json!({})),
        ] {
            assert_eq!(
                invoke(&shell, cmd, args),
                Err(json!("Unavailable")),
                "{cmd}"
            );
        }
    }

    #[test]
    fn replies_and_statuses_serialize_as_the_bindings_say() {
        assert_eq!(
            serde_json::to_value(SearchReply::Indexing).unwrap(),
            json!("Indexing")
        );
        assert_eq!(
            serde_json::to_value(BacklinksReply::Indexing).unwrap(),
            json!("Indexing")
        );
        assert_eq!(
            serde_json::to_value(IndexStatus::Rebuilding).unwrap(),
            json!("Rebuilding")
        );
        assert_eq!(
            serde_json::to_value(IndexStatusChanged(IndexStatus::Failed {
                reason: "r".into()
            }))
            .unwrap(),
            json!({ "Failed": { "reason": "r" } })
        );
        assert_eq!(
            serde_json::to_value(QueryError::Failed { reason: "r".into() }).unwrap(),
            json!({ "Failed": { "reason": "r" } })
        );
        assert_eq!(
            serde_json::to_value(PageIssue::from(&core::PageIssue {
                page_id: PageId::new("daily").unwrap(),
                problem: core::PageProblem::UnrecognizedRoot("content".into()),
                indexed_blocks: 0,
            }))
            .unwrap(),
            json!({
                "page_id": "daily",
                "problem": { "UnrecognizedRoot": { "root": "content" } },
                "indexed_blocks": 0,
            })
        );
        assert_eq!(
            serde_json::from_value::<SearchScope>(json!({ "Content": { "exclude": ["a"] } }))
                .unwrap(),
            SearchScope::Content {
                exclude: vec!["a".into()]
            }
        );
    }
}

#[cfg(test)]
mod measure {
    use std::time::{Duration, Instant};

    use aelomi_core::index::{self as core, Reply, SearchScope};
    use aelomi_core::storage::test_support::SimDisk;
    use aelomi_core::workspace::Workspace;
    use aelomi_core::workspace::fixture::search_fixture;

    use super::{SearchHit, SearchReply};

    fn p(samples: &mut [Duration], percent: usize) -> Duration {
        samples.sort();
        samples[(samples.len() * percent).div_ceil(100).max(1) - 1]
    }

    /// How much of a search reply's cost is the shell's: converting core hits to IPC types and
    /// serializing them, apart from the core query. Prints, never fails.
    #[test]
    #[ignore = "a release-build measurement: cargo nextest run -p aelomi --cargo-profile release --run-ignored only --no-capture measure_search_reply"]
    fn measure_search_reply_conversion_and_serialization() {
        let jsonl = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../fixtures/generated/search-10k-pages.jsonl"),
        )
        .unwrap();
        let disk = SimDisk::default();
        let dir = tempfile::tempdir().unwrap();
        let index = std::fs::canonicalize(dir.path())
            .unwrap()
            .join(aelomi_core::index::FILE);
        let workspace = Workspace::with_store(
            disk.open(std::path::Path::new("/data")).unwrap(),
            index,
            |_| {},
        );
        for (doc, update) in search_fixture(&jsonl, 0) {
            workspace.append(&doc, &update).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(120);
        while !matches!(
            workspace.search(&core::SearchQuery {
                text: "zorvak",
                limit: 1,
                scope: SearchScope::All
            }),
            Ok(Reply::Ready { ref value, .. }) if !value.is_empty()
        ) {
            assert!(Instant::now() < deadline, "the index never became ready");
            std::thread::sleep(Duration::from_millis(20));
        }
        for (text, limit) in [("zitida", 200), ("zitida", 20), ("kestrel", 20)] {
            let (mut query, mut convert, mut serialize, mut bytes) =
                (Vec::new(), Vec::new(), Vec::new(), 0);
            for _ in 0..200 {
                let started = Instant::now();
                let Reply::Ready { value, issues } = workspace
                    .search(&core::SearchQuery {
                        text,
                        limit,
                        scope: SearchScope::Content { exclude: &[] },
                    })
                    .unwrap()
                else {
                    panic!("indexing")
                };
                query.push(started.elapsed());
                let started = Instant::now();
                let reply = SearchReply::Ready {
                    hits: value.into_iter().map(SearchHit::from).collect(),
                    issues: issues.iter().map(Into::into).collect(),
                };
                convert.push(started.elapsed());
                let started = Instant::now();
                bytes = serde_json::to_vec(&reply).unwrap().len();
                serialize.push(started.elapsed());
            }
            eprintln!(
                "search reply query={text:?} limit={limit} bytes={bytes} core_p50_us={} core_p95_us={} convert_p50_us={} convert_p95_us={} serialize_p50_us={} serialize_p95_us={}",
                p(&mut query, 50).as_micros(),
                p(&mut query, 95).as_micros(),
                p(&mut convert, 50).as_micros(),
                p(&mut convert, 95).as_micros(),
                p(&mut serialize, 50).as_micros(),
                p(&mut serialize, 95).as_micros(),
            );
        }
    }
}
