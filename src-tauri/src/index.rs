use aelomi_core::index::{self as core, Reply};
use aelomi_core::workspace::records::PageId;
use aelomi_core::workspace::tree::Tree;
use aelomi_core::workspace::{TreeError, Workspace};
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use tauri::State;

use crate::storage::Storage;

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
    InvalidPageId,
    /// The index failed, or the workspace doc has a malformed record.
    Failed {
        reason: String,
    },
    Io(String),
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

/// `Indexing` until the index can answer from current data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub enum SearchReply {
    Indexing,
    Ready(Vec<SearchHit>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct SearchHit {
    pub page_id: String,
    /// The raw title, highlighted where it matched.
    pub title: Vec<TextPart>,
    /// The best matching block, or null when only the title matched.
    pub block: Option<BlockHit>,
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
    Ready(Vec<Backlink>),
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
    off_main(storage.workspace()?, |w| Ok(w.tree()?.into())).await
}

/// Active pages whose title or a block matches `query`; the last word matches as a prefix.
#[tauri::command]
#[specta::specta]
pub async fn search(
    storage: State<'_, Storage>,
    query: String,
    limit: u32,
) -> Result<SearchReply, QueryError> {
    off_main(storage.workspace()?, move |w| {
        Ok(match w.search(&query, limit as usize)? {
            Reply::Indexing => SearchReply::Indexing,
            Reply::Ready(hits) => SearchReply::Ready(hits.into_iter().map(Into::into).collect()),
        })
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
        Ok(match w.backlinks(&target)? {
            Reply::Indexing => BacklinksReply::Indexing,
            Reply::Ready(links) => {
                BacklinksReply::Ready(links.into_iter().map(Into::into).collect())
            }
        })
    })
    .await
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

impl From<Tree> for PageTree {
    fn from(tree: Tree) -> Self {
        Self {
            pages: tree
                .pages
                .into_iter()
                .map(|p| TreePage {
                    id: p.id.to_string(),
                    title: p.title,
                    parent_id: p.parent.map(|p| p.to_string()),
                    created_at: p.created_at as f64,
                    updated_at: p.updated_at as f64,
                })
                .collect(),
            favorites: tree.favorites.iter().map(|p| p.to_string()).collect(),
            trash: tree
                .trash
                .into_iter()
                .map(|t| TrashedPage {
                    id: t.id.to_string(),
                    title: t.title,
                    batch_id: t.batch_id,
                    trashed_at: t.trashed_at as f64,
                })
                .collect(),
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
        let shell = shell(Some(dir.path()));
        let workspace = shell.app.state::<Storage>().workspace().unwrap();
        for (doc, update) in aelomi_core::workspace::fixture::index_fixture() {
            workspace.append(&doc, &update).unwrap();
        }
        wait_for(&shell, |s| {
            invoke(s, "index_status", json!({})) == Ok(json!("Ready"))
                && invoke(s, "search", json!({ "query": "cafe", "limit": 5 }))
                    != Ok(json!({ "Ready": [] }))
        });

        assert_eq!(
            invoke(&shell, "search", json!({ "query": "cafe", "limit": 5 })),
            Ok(json!({ "Ready": [{
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
            }] }))
        );
        assert_eq!(
            invoke(&shell, "backlinks", json!({ "pageId": "reading" })),
            Ok(json!({ "Ready": [{
                "page_id": "daily",
                "title": "Daily",
                "blocks": [{ "block_id": "d4", "text": "call Reading list" }],
            }] }))
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
            invoke(&shell, "search", json!({ "query": "ipcword", "limit": 5 })).unwrap()["Ready"]
                [0]["page_id"],
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
            ("search", json!({ "query": "x", "limit": 5 })),
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
    }
}
