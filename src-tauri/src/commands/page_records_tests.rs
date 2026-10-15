use std::sync::{Arc, Mutex};

use aelomi_core::storage::test_support::SimDisk;
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::{PageRecordsChanged, page_create, page_rename, page_trash};
use crate::storage::Storage;

struct Shell {
    app: App<MockRuntime>,
    window: WebviewWindow<MockRuntime>,
    events: Arc<Mutex<usize>>,
}
fn shell(storage: Storage) -> Shell {
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![
            page_create,
            page_rename,
            page_trash
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    tauri_specta::Builder::<MockRuntime>::new()
        .events(tauri_specta::collect_events![PageRecordsChanged])
        .mount_events(&app);
    let events = Arc::new(Mutex::new(0));
    let seen = events.clone();
    app.listen_any("page-records-changed", move |_| {
        *seen.lock().unwrap() += 1;
    });
    app.manage(storage);
    let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .unwrap();
    Shell {
        app,
        window,
        events,
    }
}
fn invoke(shell: &Shell, cmd: &str, args: Value) -> Result<Value, Value> {
    get_ipc_response(
        &shell.window,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        },
    )
    .map(|body| body.deserialize::<Value>().unwrap())
}
fn created(shell: &Shell, parent: Option<&str>) -> String {
    invoke(shell, "page_create", json!({"parentId": parent}))
        .unwrap()
        .as_str()
        .unwrap()
        .into()
}

#[test]
fn page_commands_accept_pending_edits_and_publish_source_changes_before_durability() {
    let disk = SimDisk::default();
    let storage = crate::storage::tests::open_on(&disk);
    disk.fail_appends("/docs/workspace/");
    let shell = shell(storage);
    let parent = created(&shell, None);
    let child = created(&shell, Some(&parent));
    assert_eq!(
        invoke(
            &shell,
            "page_rename",
            json!({"pageId": parent, "title": "Projects"})
        ),
        Ok(Value::Null)
    );
    let workspace = shell.app.state::<Storage>().workspace().unwrap();
    assert_eq!(
        workspace
            .tree()
            .unwrap()
            .pages
            .iter()
            .map(|p| (p.id.as_str(), p.title.as_str()))
            .collect::<Vec<_>>(),
        [(parent.as_str(), "Projects"), (child.as_str(), "")]
    );
    assert_eq!(*shell.events.lock().unwrap(), 3);
    let batch = invoke(&shell, "page_trash", json!({"pageId": parent})).unwrap();
    let tree = workspace.tree().unwrap();
    assert_eq!(tree.pages, []);
    assert_eq!(tree.trash.len(), 1);
    assert_eq!(tree.trash[0].id.as_str(), parent);
    assert_eq!(json!(tree.trash[0].batch_id), batch);
    assert_eq!(*shell.events.lock().unwrap(), 4);
    assert_eq!(
        shell.app.state::<Storage>().pending_docs(),
        [aelomi_core::storage::DocId::workspace()]
    );
    assert_eq!(
        invoke(
            &shell,
            "page_rename",
            json!({"pageId": child, "title": "hidden"})
        ),
        Err(json!("NotActive"))
    );
    assert_eq!(*shell.events.lock().unwrap(), 4);
}

#[test]
fn page_commands_return_typed_errors_and_do_not_write_when_storage_is_unavailable() {
    let refused = shell(Storage::not_open());
    for (command, args) in [
        ("page_create", json!({"parentId": null})),
        ("page_rename", json!({"pageId": "a", "title": "A"})),
        ("page_trash", json!({"pageId": "a"})),
    ] {
        assert_eq!(invoke(&refused, command, args), Err(json!("Unavailable")));
    }
    assert_eq!(*refused.events.lock().unwrap(), 0);
    let disk = SimDisk::default();
    let shell = shell(crate::storage::tests::open_on(&disk));
    assert_eq!(
        invoke(&shell, "page_create", json!({"parentId": "workspace"})),
        Err(json!("InvalidPageId"))
    );
    assert_eq!(
        invoke(&shell, "page_create", json!({"parentId": "missing"})),
        Err(json!("NotActive"))
    );
    assert_eq!(
        invoke(
            &shell,
            "page_rename",
            json!({"pageId": "bad/id", "title": "A"})
        ),
        Err(json!("InvalidPageId"))
    );
    assert_eq!(
        invoke(&shell, "page_trash", json!({"pageId": "missing"})),
        Err(json!("NotActive"))
    );
    assert_eq!(
        shell
            .app
            .state::<Storage>()
            .workspace()
            .unwrap()
            .tree()
            .unwrap()
            .pages,
        []
    );
    assert_eq!(*shell.events.lock().unwrap(), 0);
}
