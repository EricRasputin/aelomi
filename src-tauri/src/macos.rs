use std::ffi::CString;
use std::sync::OnceLock;

use objc2::encode::{Encode, EncodeArguments, EncodeReturn};
use objc2::runtime::{AnyClass, AnyObject, Imp, MethodImplementation, Sel};
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSApplication, NSApplicationTerminateReply, NSEventModifierFlags};
use objc2_foundation::NSString;
use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};
use tauri_specta::Event;

use crate::storage::DataLocation;

static APP: OnceLock<AppHandle> = OnceLock::new();

type ShouldTerminate =
    extern "C" fn(*mut AnyObject, Sel, *mut AnyObject) -> NSApplicationTerminateReply;

#[derive(serde::Deserialize)]
struct MenuDefinition {
    id: String,
    label: String,
    items: Vec<MenuEntry>,
}

#[derive(serde::Deserialize)]
struct MenuEntry {
    command: String,
    label: String,
    group: u32,
    accelerator: Option<String>,
    action: MenuAction,
}

#[derive(serde::Deserialize)]
#[serde(tag = "owner", rename_all = "camelCase")]
enum MenuAction {
    Webview,
    Native { role: NativeRole },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum NativeRole {
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    CloseWindow,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Fullscreen,
    Minimize,
    Zoom,
    BringAllToFront,
}

const MENU: &str = include_str!("../menu.json");

fn menu_definitions() -> serde_json::Result<Vec<MenuDefinition>> {
    serde_json::from_str(MENU)
}

enum MenuRow<'a> {
    Separator,
    Command(&'a MenuEntry),
}

fn menu_rows(definition: &MenuDefinition) -> Vec<MenuRow<'_>> {
    let mut rows = Vec::new();
    let mut group = None;
    for entry in &definition.items {
        if group.is_some_and(|group| group != entry.group) {
            rows.push(MenuRow::Separator);
        }
        rows.push(MenuRow::Command(entry));
        group = Some(entry.group);
    }
    rows
}

fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    definitions: &[MenuDefinition],
) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    for definition in definitions {
        let submenu = Submenu::with_id(app, &definition.id, &definition.label, true)?;
        for row in menu_rows(definition) {
            match row {
                MenuRow::Separator => submenu.append(&PredefinedMenuItem::separator(app)?)?,
                MenuRow::Command(entry) => match &entry.action {
                    MenuAction::Webview
                    | MenuAction::Native {
                        role: NativeRole::Quit,
                    } => {
                        submenu.append(&MenuItem::with_id(
                            app,
                            &entry.command,
                            &entry.label,
                            true,
                            entry.accelerator.as_deref(),
                        )?)?;
                    }
                    MenuAction::Native { role } => {
                        let text = Some(entry.label.as_str());
                        let item = match role {
                            NativeRole::About => PredefinedMenuItem::about(
                                app,
                                text,
                                Some(AboutMetadata {
                                    name: Some(app.package_info().name.clone()),
                                    version: Some(app.package_info().version.to_string()),
                                    ..Default::default()
                                }),
                            ),
                            NativeRole::Services => PredefinedMenuItem::services(app, text),
                            NativeRole::Hide => PredefinedMenuItem::hide(app, text),
                            NativeRole::HideOthers => PredefinedMenuItem::hide_others(app, text),
                            NativeRole::ShowAll => PredefinedMenuItem::show_all(app, text),
                            NativeRole::CloseWindow => PredefinedMenuItem::close_window(app, text),
                            NativeRole::Cut => PredefinedMenuItem::cut(app, text),
                            NativeRole::Copy => PredefinedMenuItem::copy(app, text),
                            NativeRole::Paste => PredefinedMenuItem::paste(app, text),
                            NativeRole::SelectAll => PredefinedMenuItem::select_all(app, text),
                            NativeRole::Fullscreen => PredefinedMenuItem::fullscreen(app, text),
                            NativeRole::Minimize => PredefinedMenuItem::minimize(app, text),
                            NativeRole::Zoom => PredefinedMenuItem::maximize(app, text),
                            NativeRole::BringAllToFront => {
                                PredefinedMenuItem::bring_all_to_front(app, text)
                            }
                            NativeRole::Quit => {
                                unreachable!("Quit uses the drain-aware custom item")
                            }
                        }?;
                        submenu.append(&item)?;
                    }
                },
            }
        }
        menu.append(&submenu)?;
        if definition.id == "window" {
            submenu.set_as_windows_menu_for_nsapp()?;
        } else if definition.id == "help" {
            submenu.set_as_help_menu_for_nsapp()?;
        }
    }
    Ok(menu)
}

fn key_equivalent(accelerator: Option<&str>) -> (String, NSEventModifierFlags) {
    let mut key = String::new();
    let mut modifiers = NSEventModifierFlags::empty();
    for part in accelerator.unwrap_or_default().split('+') {
        match part {
            "Super" => modifiers |= NSEventModifierFlags::Command,
            "Control" => modifiers |= NSEventModifierFlags::Control,
            "Alt" => modifiers |= NSEventModifierFlags::Option,
            "Shift" => modifiers |= NSEventModifierFlags::Shift,
            "Backslash" => key = "\\".into(),
            "Comma" => key = ",".into(),
            value => key = value.to_lowercase(),
        }
    }
    (key, modifiers)
}

// Tauri's predefined roles have fixed accelerators and expose no setter. Apply the table's
// equivalents to AppKit too, so those roles do not become a second shortcut source.
fn apply_native_accelerators(definitions: &[MenuDefinition]) {
    let mtm = MainThreadMarker::new().expect("setup runs on the main thread");
    let menu = NSApplication::sharedApplication(mtm)
        .mainMenu()
        .expect("the app menu is installed");
    for definition in definitions {
        let submenu = menu
            .itemArray()
            .iter()
            .filter_map(|item| item.submenu())
            .find(|submenu| submenu.title().to_string() == definition.label)
            .expect("every generated submenu is installed");
        let items = submenu.itemArray();
        for entry in &definition.items {
            let item = items
                .iter()
                .find(|item| item.title().to_string() == entry.label)
                .expect("every generated menu item is installed");
            let (key, modifiers) = key_equivalent(entry.accelerator.as_deref());
            item.setKeyEquivalent(&NSString::from_str(&key));
            item.setKeyEquivalentModifierMask(modifiers);
        }
    }
}

pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    let definitions = menu_definitions()?;
    app.set_menu(build_menu(app.handle(), &definitions)?)?;
    apply_native_accelerators(&definitions);
    app.on_menu_event(move |app, event| {
        let Some(entry) = definitions
            .iter()
            .flat_map(|menu| &menu.items)
            .find(|entry| entry.command == event.id().as_ref())
        else {
            return;
        };
        match entry.action {
            MenuAction::Native {
                role: NativeRole::Quit,
            } => app.exit(0),
            MenuAction::Webview => {
                if let Err(error) = (crate::commands::MenuCommand {
                    command: entry.command.clone(),
                })
                .emit(app)
                {
                    eprintln!("menu command delivery failed: {error}");
                }
            }
            MenuAction::Native { .. } => {}
        }
    });
    hold_terminate(app);
    Ok(())
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

pub fn peak_rss_bytes() -> std::io::Result<u64> {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    u64::try_from(usage.ru_maxrss)
        .map_err(|_| std::io::Error::other("the native RSS peak was negative"))
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
    fn generated_menu_build_preserves_items_groups_and_accelerators() {
        let definitions = menu_definitions().unwrap();
        let built: Vec<_> = definitions
            .iter()
            .map(|menu| {
                let rows: Vec<_> = menu_rows(menu)
                    .into_iter()
                    .map(|row| match row {
                        MenuRow::Separator => "---".to_owned(),
                        MenuRow::Command(entry) => format!(
                            "{}|{}|{}",
                            entry.command,
                            entry.label,
                            entry.accelerator.as_deref().unwrap_or("")
                        ),
                    })
                    .collect();
                (menu.label.as_str(), rows)
            })
            .collect();
        assert_eq!(
            built,
            vec![
                (
                    "Aelomi",
                    vec![
                        "about|About Aelomi|",
                        "---",
                        "openSettings|Settings…|Super+Comma",
                        "---",
                        "services|Services|",
                        "---",
                        "hide|Hide Aelomi|Super+H",
                        "hideOthers|Hide Others|Super+Alt+H",
                        "showAll|Show All|",
                        "---",
                        "quit|Quit Aelomi|Super+Q"
                    ]
                ),
                ("File", vec!["closeWindow|Close Window|Super+W"]),
                (
                    "Edit",
                    vec![
                        "undo|Undo|Super+Z",
                        "redo|Redo|Super+Shift+Z",
                        "---",
                        "cut|Cut|Super+X",
                        "copy|Copy|Super+C",
                        "paste|Paste|Super+V",
                        "selectAll|Select All|Super+A"
                    ]
                ),
                (
                    "View",
                    vec![
                        "openPalette|Search and commands|Super+K",
                        "toggleSidebar|Toggle Sidebar|Super+Backslash",
                        "---",
                        "fullscreen|Enter Full Screen|Super+Control+F"
                    ]
                ),
                (
                    "Window",
                    vec![
                        "minimize|Minimize|Super+M",
                        "zoom|Zoom|",
                        "---",
                        "bringAllToFront|Bring All to Front|"
                    ]
                ),
                ("Help", vec![]),
            ]
            .into_iter()
            .map(|(label, rows)| (
                label,
                rows.into_iter().map(str::to_owned).collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
        );
        let custom: Vec<_> = definitions
            .iter()
            .flat_map(|menu| &menu.items)
            .filter(|entry| {
                matches!(
                    entry.action,
                    MenuAction::Webview
                        | MenuAction::Native {
                            role: NativeRole::Quit
                        }
                )
            })
            .map(|entry| entry.command.as_str())
            .collect();
        assert_eq!(
            custom,
            [
                "openSettings",
                "quit",
                "undo",
                "redo",
                "openPalette",
                "toggleSidebar"
            ]
        );
    }

    #[test]
    fn generated_accelerators_reach_appkit_with_every_modifier() {
        assert_eq!(
            key_equivalent(Some("Super+Shift+Z")),
            (
                "z".into(),
                NSEventModifierFlags::Command | NSEventModifierFlags::Shift
            )
        );
        assert_eq!(
            key_equivalent(Some("Super+Alt+H")),
            (
                "h".into(),
                NSEventModifierFlags::Command | NSEventModifierFlags::Option
            )
        );
        assert_eq!(
            key_equivalent(Some("Super+Control+F")),
            (
                "f".into(),
                NSEventModifierFlags::Command | NSEventModifierFlags::Control
            )
        );
        assert_eq!(
            key_equivalent(Some("Super+Backslash")),
            ("\\".into(), NSEventModifierFlags::Command)
        );
        assert_eq!(
            key_equivalent(Some("Super+Comma")),
            (",".into(), NSEventModifierFlags::Command)
        );
        assert_eq!(
            key_equivalent(None),
            (String::new(), NSEventModifierFlags::empty())
        );
    }

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
            [
                r#"quit-status-changed {"revision":1,"phase":{"Asking":{"pending_docs":["page"],"pending_settings":false}}}"#
            ]
        );
        assert_eq!(
            crate::commands::quit_status(app.state()),
            QuitStatus {
                revision: 1,
                phase: QuitPhase::Asking {
                    pending_docs: vec!["page".into()],
                    pending_settings: false,
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
            [
                r#"quit-status-changed {"revision":1,"phase":{"Asking":{"pending_docs":["page"],"pending_settings":false}}}"#
            ]
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
                r#"quit-status-changed {"revision":1,"phase":{"Asking":{"pending_docs":["page"],"pending_settings":false}}}"#,
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
