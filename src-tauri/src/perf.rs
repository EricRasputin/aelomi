use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;

pub static MAIN_US: AtomicU64 = AtomicU64::new(0);
pub static SETUP_US: AtomicU64 = AtomicU64::new(0);
pub static PAGE_START_US: AtomicU64 = AtomicU64::new(0);
pub static PAGE_DONE_US: AtomicU64 = AtomicU64::new(0);

const REPORT_ENV: &str = "AELOMI_PERF_REPORT";
pub const ARM_SCRIPT: &str = "window.__AELOMI_PERF__ = true;";

#[derive(Deserialize, Serialize, Type)]
pub struct ColdStartMarks {
    pub variant: String,
    pub time_origin: f64,
    pub module_eval: f64,
    pub commit: f64,
    pub raf1: f64,
    pub mc_after_raf1: f64,
    pub raf2: f64,
    pub mc_after_raf2: f64,
    pub fcp: Option<f64>,
}

#[derive(Serialize)]
struct RustStamps {
    process_start_us: u64,
    main_us: u64,
    setup_us: u64,
    page_start_us: u64,
    page_done_us: u64,
    received_us: u64,
}

#[derive(Serialize)]
struct ColdStartReport {
    js: ColdStartMarks,
    rust: RustStamps,
}

pub fn report_path() -> Option<std::path::PathBuf> {
    std::env::var_os(REPORT_ENV).map(Into::into)
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

pub fn stamp(slot: &AtomicU64) {
    let _ = slot.compare_exchange(0, now_us(), Relaxed, Relaxed);
}

fn process_start_us() -> u64 {
    #[cfg(target_os = "macos")]
    {
        crate::macos::process_start_us()
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

pub fn write_report(path: &std::path::Path, js: ColdStartMarks) -> std::io::Result<()> {
    let report = ColdStartReport {
        js,
        rust: RustStamps {
            process_start_us: process_start_us(),
            main_us: MAIN_US.load(Relaxed),
            setup_us: SETUP_US.load(Relaxed),
            page_start_us: PAGE_START_US.load(Relaxed),
            page_done_us: PAGE_DONE_US.load(Relaxed),
            received_us: now_us(),
        },
    };
    std::fs::write(path, serde_json::to_vec(&report)?)
}
