mod commands;
#[cfg(target_os = "macos")]
mod macos;
mod perf;

use tauri::webview::PageLoadEvent;

pub fn run() {
    perf::stamp(&perf::MAIN_US);
    let commands = commands::builder();
    let mut builder = tauri::Builder::default()
        .invoke_handler(commands.invoke_handler())
        .on_page_load(|_, payload| match payload.event() {
            PageLoadEvent::Started => perf::stamp(&perf::PAGE_START_US),
            PageLoadEvent::Finished => perf::stamp(&perf::PAGE_DONE_US),
        })
        .setup(|app| {
            perf::stamp(&perf::SETUP_US);
            #[cfg(target_os = "macos")]
            macos::setup(app)?;
            Ok(())
        });
    if perf::report_path().is_some() {
        builder = builder.plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("perf")
                .js_init_script(perf::ARM_SCRIPT)
                .build(),
        );
    }
    builder
        .run(tauri::generate_context!())
        .expect("failed to run Aelomi");
}
