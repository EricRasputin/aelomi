//! Content-free timing lines for native measurement: process-relative times on index log lines,
//! and per-command timings and byte counts when `AELOMI_TRACE_IPC=1`.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use aelomi_core::index::StartupReport;

/// Milliseconds since the process started, for log lines.
pub fn since_start_ms() -> f64 {
    static START_US: OnceLock<u64> = OnceLock::new();
    let start = *START_US.get_or_init(crate::perf::process_start_us);
    let now = crate::perf::now_us();
    if start == 0 || now < start {
        return 0.0;
    }
    (now - start) as f64 / 1000.0
}

pub fn ipc_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("AELOMI_TRACE_IPC").is_some_and(|v| v == "1"))
}

pub fn ms(duration: Duration) -> String {
    format!("{:.3}", duration.as_secs_f64() * 1000.0)
}

/// Prints `ipc <command> ms=<since started> <fields>` when tracing is on.
pub fn ipc(command: &str, started: Instant, fields: impl FnOnce() -> String) {
    if ipc_enabled() {
        println!("ipc {command} ms={} {}", ms(started.elapsed()), fields());
    }
}

pub fn startup_line(report: &StartupReport) -> String {
    let optional = |d: Option<Duration>| d.map_or_else(|| "-".to_owned(), ms);
    format!(
        "index startup total_ms={} check_open_ms={} schema_ms={} integrity_ms={} \
         title_fts_ms={} block_fts_ms={} workspace_load_ms={} workspace_decode_ms={} \
         workspace_forest_ms={} workspace_sql_ms={} pages={} loaded_bytes={} page_load_ms={} \
         page_decode_ms={} page_project_ms={} page_sql_ms={} publish_ms={} t_ms={:.1}",
        ms(report.total),
        ms(report.check.open),
        optional(report.check.schema),
        optional(report.check.integrity),
        optional(report.check.title_fts),
        optional(report.check.block_fts),
        ms(report.workspace_load),
        ms(report.workspace_decode),
        ms(report.workspace_forest),
        ms(report.workspace_sql),
        report.pages,
        report.loaded_bytes,
        ms(report.page_load),
        ms(report.page_decode),
        ms(report.page_project),
        ms(report.page_sql),
        ms(report.publish),
        since_start_ms(),
    )
}
