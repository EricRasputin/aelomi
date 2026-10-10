pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    app.set_menu(tauri::menu::Menu::default(app.handle())?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Read, pipe},
        os::fd::AsRawFd,
        path::Path,
        thread,
    };

    #[test]
    fn bindings_are_current() {
        let (mut reader, writer) = pipe().expect("create bindings pipe");
        let read = thread::spawn(move || {
            let mut bindings = String::new();
            reader.read_to_string(&mut bindings).expect("read bindings");
            bindings
        });
        let path = format!("/dev/fd/{}", writer.as_raw_fd());
        let result =
            crate::commands::builder().export(specta_typescript::Typescript::default(), path);
        drop(writer);
        result.expect("generate bindings");
        let generated = format!("{}\n", read.join().expect("join bindings reader").trim_end());
        let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/ipc/bindings.ts");
        if std::env::var_os("AELOMI_WRITE_BINDINGS").is_some_and(|value| value == "1") {
            fs::write(&committed, &generated).expect("write bindings");
        }
        assert_eq!(
            fs::read_to_string(committed).expect("read committed bindings; run pnpm bindings"),
            generated,
            "IPC bindings changed; run pnpm bindings and check the TypeScript callers"
        );
    }
}
