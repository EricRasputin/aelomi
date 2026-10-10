use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Deserialize, Type)]
pub struct PingRequest {
    pub seq: u32,
}

#[derive(Serialize, Type)]
pub struct PingReply {
    pub seq: u32,
    pub core_version: String,
}

#[tauri::command]
#[specta::specta]
pub fn ping(request: PingRequest) -> PingReply {
    println!("ping seq={}", request.seq);
    PingReply {
        seq: request.seq,
        core_version: aelomi_core::VERSION.to_owned(),
    }
}

pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new().commands(tauri_specta::collect_commands![ping])
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    #[test]
    fn bindings_are_current() {
        let scratch =
            std::env::temp_dir().join(format!("aelomi-bindings-{}.ts", std::process::id()));
        super::builder()
            .export(specta_typescript::Typescript::default(), &scratch)
            .expect("generate bindings");
        let generated = format!(
            "{}\n",
            fs::read_to_string(&scratch)
                .expect("read generated bindings")
                .trim_end()
        );
        fs::remove_file(&scratch).expect("remove generated bindings");
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
