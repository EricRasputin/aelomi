use std::sync::OnceLock;
use std::time::{Duration, Instant};

use aelomi_core::index::StartupReport;

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

pub enum SearchHits {
    Indexing,
    Count(usize),
}

pub enum IpcMeasurements {
    DocLoad {
        base64_bytes: usize,
    },
    DocAppend {
        delta_bytes: usize,
    },
    PageTree {
        pages: usize,
        json_bytes: usize,
    },
    Search {
        core: Duration,
        dto: Duration,
        hits: SearchHits,
        json_bytes: usize,
    },
    Backlinks {
        json_bytes: usize,
    },
}

pub fn ipc(started: Instant, measure: impl FnOnce() -> IpcMeasurements) {
    if ipc_enabled() {
        let elapsed = started.elapsed();
        println!("{}", format_ipc(elapsed, measure()));
    }
}

fn format_ipc(elapsed: Duration, measurements: IpcMeasurements) -> String {
    let total = ms(elapsed);
    match measurements {
        IpcMeasurements::DocLoad { base64_bytes } => {
            format!("ipc doc_load ms={total} bytes={base64_bytes}")
        }
        IpcMeasurements::DocAppend { delta_bytes } => {
            format!("ipc doc_append ms={total} bytes={delta_bytes}")
        }
        IpcMeasurements::PageTree { pages, json_bytes } => {
            format!("ipc page_tree ms={total} pages={pages} bytes={json_bytes}")
        }
        IpcMeasurements::Search {
            core,
            dto,
            hits,
            json_bytes,
        } => {
            let hits = match hits {
                SearchHits::Indexing => "indexing".to_owned(),
                SearchHits::Count(count) => count.to_string(),
            };
            format!(
                "ipc search ms={total} core_ms={} dto_ms={} hits={hits} bytes={json_bytes}",
                ms(core),
                ms(dto)
            )
        }
        IpcMeasurements::Backlinks { json_bytes } => {
            format!("ipc backlinks ms={total} bytes={json_bytes}")
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_ipc_line_names_its_command_and_only_numbers() {
        let lines: Vec<String> = [
            IpcMeasurements::DocLoad { base64_bytes: 344 },
            IpcMeasurements::DocAppend { delta_bytes: 27 },
            IpcMeasurements::PageTree {
                pages: 6,
                json_bytes: 1_203,
            },
            IpcMeasurements::Search {
                core: Duration::from_micros(812),
                dto: Duration::from_nanos(45_678),
                hits: SearchHits::Count(3),
                json_bytes: 2_048,
            },
            IpcMeasurements::Search {
                core: Duration::from_micros(90),
                dto: Duration::ZERO,
                hits: SearchHits::Indexing,
                json_bytes: 10,
            },
            IpcMeasurements::Backlinks { json_bytes: 77 },
        ]
        .into_iter()
        .map(|measurements| format_ipc(Duration::from_nanos(2_345_678), measurements))
        .collect();
        assert_eq!(
            lines,
            [
                "ipc doc_load ms=2.346 bytes=344",
                "ipc doc_append ms=2.346 bytes=27",
                "ipc page_tree ms=2.346 pages=6 bytes=1203",
                "ipc search ms=2.346 core_ms=0.812 dto_ms=0.046 hits=3 bytes=2048",
                "ipc search ms=2.346 core_ms=0.090 dto_ms=0.000 hits=indexing bytes=10",
                "ipc backlinks ms=2.346 bytes=77",
            ]
        );
    }
}
