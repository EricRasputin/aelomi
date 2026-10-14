use std::ffi::CString;
use std::sync::OnceLock;

use objc2::encode::{Encode, EncodeArguments, EncodeReturn};
use objc2::runtime::{AnyClass, AnyObject, Imp, MethodImplementation, Sel};
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSApplication, NSApplicationTerminateReply};
use tauri::menu::{Menu, MenuItem, MenuItemKind};
use tauri::{AppHandle, Runtime};

use crate::storage::DataLocation;

const QUIT: &str = "quit";

static APP: OnceLock<AppHandle> = OnceLock::new();

type ShouldTerminate =
    extern "C" fn(*mut AnyObject, Sel, *mut AnyObject) -> NSApplicationTerminateReply;

pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    let menu = Menu::default(app.handle())?;
    replace_quit(app, &menu)?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id() == QUIT {
            app.exit(0);
        }
    });
    hold_terminate(app);
    Ok(())
}

fn replace_quit(app: &tauri::App, menu: &Menu<tauri::Wry>) -> tauri::Result<()> {
    let Some(MenuItemKind::Submenu(app_menu)) = menu.items()?.into_iter().next() else {
        return Ok(());
    };
    let predefined_quit = app_menu.items()?.into_iter().find(|item| {
        item.as_predefined_menuitem()
            .and_then(|item| item.text().ok())
            .is_some_and(|text| text.starts_with("Quit"))
    });
    if let Some(item) = predefined_quit {
        app_menu.remove(&item)?;
    }
    let text = format!("Quit {}", app.package_info().name);
    app_menu.append(&MenuItem::with_id(
        app,
        QUIT,
        text,
        true,
        Some("CmdOrCtrl+Q"),
    )?)
}

/// Logout, the Dock's Quit, and AppleScript `quit` send `terminate:`. tao's app delegate has no
/// `applicationShouldTerminate:`, so without this the app exits at once, losing updates still
/// pending after the `RunEvent::Exit` flush.
fn hold_terminate(app: &tauri::App) {
    let _ = APP.set(app.handle().clone());
    let mtm = MainThreadMarker::new().expect("setup runs on the main thread");
    let installed = match NSApplication::sharedApplication(mtm).delegate() {
        Some(delegate) => add_should_terminate(AsRef::<AnyObject>::as_ref(&*delegate).class()),
        None => Err("the app has no delegate".to_owned()),
    };
    if let Err(error) = installed {
        eprintln!("quit hold not installed, logout and the Dock's Quit skip the flush: {error}");
    }
}

fn add_should_terminate(class: &AnyClass) -> Result<(), String> {
    let mut types = format!(
        "{}{}{}",
        <ShouldTerminate as MethodImplementation>::Return::ENCODING_RETURN,
        <*mut AnyObject>::ENCODING,
        Sel::ENCODING
    );
    for argument in <ShouldTerminate as MethodImplementation>::Arguments::ENCODINGS {
        types.push_str(&argument.to_string());
    }
    let types = CString::new(types).expect("encodings have no NUL");
    let added = unsafe {
        objc2::ffi::class_addMethod(
            class as *const AnyClass as *mut AnyClass,
            sel!(applicationShouldTerminate:),
            std::mem::transmute::<ShouldTerminate, Imp>(should_terminate),
            types.as_ptr(),
        )
    };
    if added.as_bool() {
        Ok(())
    } else {
        Err(format!(
            "{} already implements applicationShouldTerminate:",
            class.name().to_string_lossy()
        ))
    }
}

extern "C" fn should_terminate(
    _: *mut AnyObject,
    _: Sel,
    _: *mut AnyObject,
) -> NSApplicationTerminateReply {
    APP.get()
        .map_or(NSApplicationTerminateReply::TerminateNow, terminate_reply)
}

/// Never `NSTerminateLater`: while AppKit waits for that reply it handles no input and drops key
/// presses.
fn terminate_reply<R: Runtime>(app: &AppHandle<R>) -> NSApplicationTerminateReply {
    if crate::flush_then_allow_quit(app, "Terminate") {
        NSApplicationTerminateReply::TerminateNow
    } else {
        NSApplicationTerminateReply::TerminateCancel
    }
}

pub fn data_location() -> DataLocation {
    let home = std::env::home_dir().unwrap_or_default();
    let folder = if cfg!(debug_assertions) {
        "Aelomi Dev"
    } else {
        "Aelomi"
    };
    DataLocation {
        path: home.join("Library/Application Support").join(folder),
        cloud_roots: vec![
            home.join("Library/Mobile Documents"),
            home.join("Library/CloudStorage"),
        ],
    }
}

pub fn process_start_us() -> u64 {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let written = unsafe {
        libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTBSDINFO,
            0,
            &mut info as *mut libc::proc_bsdinfo as *mut libc::c_void,
            size,
        )
    };
    if written != size {
        return 0;
    }
    info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec
}

#[cfg(test)]
mod tests {
    use aelomi_core::storage::test_support::SimDisk;
    use std::ffi::CStr;

    use objc2::rc::Retained;
    use objc2::runtime::{ClassBuilder, NSObject};
    use objc2::{ClassType, msg_send};
    use tauri::Manager;

    use crate::quit::{QuitPhase, QuitStatus};

    use super::*;

    #[test]
    fn the_hook_installs_once_on_the_delegate_class() {
        let class = ClassBuilder::new(c"AelomiStandInDelegate", NSObject::class())
            .unwrap()
            .register();
        let selector = sel!(applicationShouldTerminate:);
        assert!(!class.responds_to(selector));

        assert_eq!(add_should_terminate(class), Ok(()));
        assert!(class.responds_to(selector));
        let types = unsafe {
            CStr::from_ptr(objc2::ffi::method_getTypeEncoding(
                class.instance_method(selector).unwrap(),
            ))
        };
        assert_eq!(types, c"Q@:@");
        let delegate: Retained<NSObject> = unsafe { msg_send![class, new] };
        let reply: NSApplicationTerminateReply = unsafe {
            msg_send![&delegate, applicationShouldTerminate: std::ptr::null_mut::<AnyObject>()]
        };
        assert_eq!(reply, NSApplicationTerminateReply::TerminateNow);

        assert_eq!(
            add_should_terminate(class),
            Err("AelomiStandInDelegate already implements applicationShouldTerminate:".to_owned())
        );
    }

    #[test]
    fn terminating_is_cancelled_while_updates_are_pending() {
        let disk = SimDisk::default();
        let crate::tests::BlockedApp {
            app, quit_events, ..
        } = crate::tests::app_with_page_pending(&disk);

        assert_eq!(
            terminate_reply(app.handle()),
            NSApplicationTerminateReply::TerminateCancel
        );
        assert_eq!(
            *quit_events.lock().unwrap(),
            [r#"quit-status-changed {"revision":1,"phase":{"Asking":{"pending_docs":["page"]}}}"#]
        );
        assert_eq!(
            crate::commands::quit_status(app.state()),
            QuitStatus {
                revision: 1,
                phase: QuitPhase::Asking {
                    pending_docs: vec!["page".into()],
                }
            }
        );

        disk.clear_faults();
        assert_eq!(
            terminate_reply(app.handle()),
            NSApplicationTerminateReply::TerminateNow
        );
        assert_eq!(
            *quit_events.lock().unwrap(),
            [r#"quit-status-changed {"revision":1,"phase":{"Asking":{"pending_docs":["page"]}}}"#]
        );
    }

    #[test]
    fn terminating_waits_for_a_registered_webview_without_blocking() {
        let disk = SimDisk::default();
        let crate::tests::BlockedApp {
            app, quit_events, ..
        } = crate::tests::app_with_page_pending(&disk);
        app.state::<crate::storage::Storage>()
            .quit()
            .register_frontend();

        assert_eq!(
            terminate_reply(app.handle()),
            NSApplicationTerminateReply::TerminateCancel
        );
        assert_eq!(
            terminate_reply(app.handle()),
            NSApplicationTerminateReply::TerminateCancel
        );
        disk.clear_faults();
        let mut exits = 0;
        crate::commands::complete_quit_with(app.handle(), 1, || exits += 1);
        assert_eq!(exits, 1);
        assert_eq!(
            terminate_reply(app.handle()),
            NSApplicationTerminateReply::TerminateNow
        );
        assert_eq!(
            *quit_events.lock().unwrap(),
            [
                r#"quit-status-changed {"revision":1,"phase":"Draining"}"#,
                r#"quit-status-changed {"revision":2,"phase":"Confirmed"}"#,
            ]
        );
    }

    #[test]
    fn confirming_after_cancel_allows_terminate_while_writes_still_fail() {
        let disk = SimDisk::default();
        let crate::tests::BlockedApp {
            app, quit_events, ..
        } = crate::tests::app_with_page_pending(&disk);
        assert_eq!(
            terminate_reply(app.handle()),
            NSApplicationTerminateReply::TerminateCancel
        );
        let mut exits = 0;
        crate::commands::quit_anyway_with(app.handle(), || exits += 1);
        assert_eq!(exits, 1);
        assert_eq!(
            terminate_reply(app.handle()),
            NSApplicationTerminateReply::TerminateNow
        );
        assert_eq!(
            *quit_events.lock().unwrap(),
            [
                r#"quit-status-changed {"revision":1,"phase":{"Asking":{"pending_docs":["page"]}}}"#,
                r#"quit-status-changed {"revision":2,"phase":"Confirmed"}"#,
            ]
        );
        assert_eq!(
            crate::commands::quit_status(app.state()),
            QuitStatus {
                revision: 2,
                phase: QuitPhase::Confirmed
            }
        );
    }
}
