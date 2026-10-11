use tauri::menu::{Menu, MenuItem, MenuItemKind};

use crate::storage::DataLocation;

const QUIT: &str = "quit";

pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    let menu = Menu::default(app.handle())?;
    replace_quit(app, &menu)?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id() == QUIT {
            app.exit(0);
        }
    });
    Ok(())
}

/// The predefined Quit item sends `terminate:`, which tao reports only as `RunEvent::Exit`, once
/// the exit can no longer be stopped. This item quits through `RunEvent::ExitRequested` instead,
/// so pending updates can keep the app open.
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
