pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    app.set_menu(tauri::menu::Menu::default(app.handle())?)?;
    Ok(())
}
