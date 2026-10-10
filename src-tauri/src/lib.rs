mod commands;
#[cfg(target_os = "macos")]
mod macos;

pub fn run() {
    let commands = commands::builder();
    tauri::Builder::default()
        .invoke_handler(commands.invoke_handler())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            macos::setup(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Aelomi");
}
