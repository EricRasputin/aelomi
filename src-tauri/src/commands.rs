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
