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
        .events(tauri_specta::collect_events![
            PageRecordsChanged,
            crate::quit::QuitStatusChanged
        ])
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

#[test]
fn complete_quit_waits_for_a_rename_held_on_the_record_lock_and_reopens_with_its_title() {
    use aelomi_core::storage::DocId;
    use aelomi_core::workspace::PageEdit;
    use aelomi_core::workspace::records::read_pages;
    use std::future::Future;
    use std::sync::mpsc;
    use std::task::{Context, Poll, Waker};
    use std::time::{Duration, Instant};
    use yrs::updates::decoder::Decode;
    use yrs::{Doc, Transact, Update};

    let disk = SimDisk::default();
    let shell = shell(crate::storage::tests::open_on(&disk));
    let workspace = shell.app.state::<Storage>().workspace().unwrap();
    let id = workspace
        .edit_page(PageEdit::Create {
            parent: None,
            title: "Before".into(),
        })
        .unwrap()
        .created_page()
        .unwrap();
    assert_eq!(workspace.flush(Instant::now() + Duration::from_secs(5)), []);
    let handle = shell.app.handle();
    let held = workspace.hold_record_edits();
    let mut rename = Box::pin(page_rename(
        handle.clone(),
        handle.state(),
        id.to_string(),
        "Just before quit".into(),
    ));
    assert!(matches!(
        rename
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    let storage = handle.state::<Storage>();
    storage.quit().register_frontend();
    let crate::quit::QuitRequest::StartDrain(status) = storage.quit().request() else {
        panic!()
    };
    let (send, receive) = mpsc::channel();
    let waited = std::thread::scope(|scope| {
        let quit = scope.spawn(|| {
            let result =
                super::complete_quit_with(handle, status.revision, || send.send(()).unwrap());
            assert_eq!(result.phase, crate::quit::QuitPhase::Confirmed);
        });
        let waited = matches!(
            receive.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        drop(held);
        assert!(tauri::async_runtime::block_on(rename).is_ok());
        quit.join().unwrap();
        waited
    });
    assert!(
        waited,
        "quit confirmed before the blocked page command appended"
    );
    disk.crash();
    let reopened = disk.open(std::path::Path::new("/data")).unwrap();
    let doc = Doc::new();
    doc.transact_mut()
        .apply_update(Update::decode_v1(&reopened.load(&DocId::workspace()).unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        read_pages(&doc.transact()).unwrap()[&id].title,
        "Just before quit"
    );
}

#[test]
fn a_second_quit_drain_accepts_a_rename_and_reopens_with_its_title() {
    use aelomi_core::storage::DocId;
    use std::time::{Duration, Instant};
    use yrs::updates::decoder::Decode;
    use yrs::{Doc, Transact, Update};

    let disk = SimDisk::default();
    let shell = shell(crate::storage::tests::open_on(&disk));
    let id = created(&shell, None);
    let storage = shell.app.state::<Storage>();
    storage.quit().register_frontend();
    let crate::quit::QuitRequest::StartDrain(first) = storage.quit().request() else {
        panic!()
    };
    assert!(storage.quit().begin_completing(first.revision));
    storage.flush_before_quit(Duration::from_secs(1)).unwrap();
    storage
        .quit()
        .finish_drain(first.revision, &[DocId::workspace()])
        .unwrap();
    let crate::quit::QuitRequest::StartDrain(second) = storage.quit().request() else {
        panic!()
    };
    assert_eq!(
        invoke(
            &shell,
            "page_rename",
            json!({"pageId": id, "title": "Second drain title"})
        ),
        Ok(Value::Null)
    );
    let result = super::complete_quit_with(shell.app.handle(), second.revision, || {});
    assert_eq!(result.phase, crate::quit::QuitPhase::Confirmed);
    assert_eq!(
        storage
            .workspace()
            .unwrap()
            .flush(Instant::now() + Duration::from_secs(1)),
        []
    );
    disk.crash();
    let reopened = disk.open(std::path::Path::new("/data")).unwrap();
    let doc = Doc::new();
    doc.transact_mut()
        .apply_update(Update::decode_v1(&reopened.load(&DocId::workspace()).unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        aelomi_core::workspace::records::read_pages(&doc.transact()).unwrap()
            [&aelomi_core::workspace::records::PageId::new(id).unwrap()]
            .title,
        "Second drain title"
    );
}
