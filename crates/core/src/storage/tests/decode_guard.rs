use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use yrs::encoding::write::Write;
use yrs::updates::encoder::{Encoder, EncoderV1};

use super::{Env, applied, doc, soon, typed, v2_snapshot_name};
use crate::storage::log::{self, LogContents, LogEnd, LogFormat, SEGMENT_HEADER, encode_segment};
use crate::storage::snapshot::encode_snapshot;
use crate::storage::store::AppendError;

const NESTING_LIMIT: usize = 1024;

pub(crate) fn nested_update(depth: usize) -> Vec<u8> {
    let mut encoder = EncoderV1::new();
    encoder.write_var(1u32);
    encoder.write_var(1u32);
    encoder.write_var(42u64);
    encoder.write_var(0u32);
    encoder.write_u8(8);
    encoder.write_var(1u32);
    encoder.write_string("nested");
    encoder.write_var(1u32);
    for _ in 1..depth {
        encoder.write_u8(117);
        encoder.write_var(1usize);
    }
    encoder.write_u8(126);
    encoder.write_var(0u32);
    encoder.to_vec()
}

fn child(case: &str) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "storage::tests::decode_guard::decode_guard_child",
            "--nocapture",
        ])
        .env("AELOMI_DECODE_GUARD_CASE", case)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "{case} timed out: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{case}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn decode_guard_append_bounds_resident_memory() {
    child("allocation");
}

#[test]
fn decode_guard_append_rejects_above_the_limit() {
    child("above");
}

#[test]
fn decode_guard_at_the_limit_survives_compaction_and_reopen() {
    child("boundary");
}

#[test]
fn decode_guard_overdeep_disk_records_survive_recovery_and_compaction() {
    child("disk");
}

#[test]
fn decode_guard_resync_keeps_an_overdeep_record_without_decoding_it() {
    child("resync");
}

#[test]
fn decode_guard_length_recovery_bounds_resident_memory() {
    child("length-allocation");
}

#[test]
fn decode_guard_length_recovery_bounds_next_record_memory() {
    child("length-next-allocation");
}

#[test]
fn decode_guard_length_recovery_keeps_overdeep_records() {
    child("length-disk");
}

#[test]
fn decode_guard_small_caller_stack_survives_the_boundary() {
    child("small-stack");
}

#[test]
fn decode_guard_threaded_compactor_survives_the_boundary() {
    child("threaded");
}

#[test]
fn decode_guard_child() {
    let Ok(case) = std::env::var("AELOMI_DECODE_GUARD_CASE") else {
        return;
    };
    if case == "allocation" {
        let env = Env::manual();
        let store = env.open();
        let before = max_rss();
        assert!(matches!(
            store.append(&doc("a"), &[0x80, 0x80, 0x80, 0x80, 1]),
            Err(AppendError::InvalidUpdate)
        ));
        let growth = max_rss().saturating_sub(before);
        eprintln!("append maximum RSS growth: {growth} bytes");
        assert!(
            growth < 32 * 1024 * 1024,
            "append increased maximum RSS by {growth} bytes"
        );
        assert_eq!(env.doc_files(&doc("a")), Vec::<String>::new());
        return;
    }
    if case == "length-allocation" || case == "length-next-allocation" {
        let bomb = [0x80, 0x80, 0x80, 0x80, 1];
        let good = typed(7, 3);
        let payloads: Vec<&[u8]> = if case == "length-next-allocation" {
            vec![&good, &bomb, &good]
        } else {
            vec![&bomb, &good]
        };
        let mut bytes = encode_segment(&payloads);
        bytes[SEGMENT_HEADER..SEGMENT_HEADER + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = max_rss();
        let contents = log::read(LogFormat::V1, &bytes);
        let growth = max_rss().saturating_sub(before);
        eprintln!("{case} maximum RSS growth: {growth} bytes");
        assert!(
            growth < 32 * 1024 * 1024,
            "length recovery increased maximum RSS by {growth} bytes"
        );
        assert_eq!(
            contents,
            LogContents {
                records: vec![good.clone()],
                end: LogEnd::Corrupt,
            }
        );
        return;
    }
    let stack_size = if case == "small-stack" {
        2 * 1024 * 1024
    } else {
        32 * 1024 * 1024
    };
    std::thread::Builder::new()
        .stack_size(stack_size)
        .spawn(move || {
            let mut env = if case == "threaded" {
                Env::threaded()
            } else {
                Env::manual()
            };
            if case == "threaded" {
                env.limits.records = 0;
            }
            let a = doc("a");
            let store = env.open();
            if case == "above" {
                assert!(matches!(
                    store.append(&a, &nested_update(NESTING_LIMIT + 1)),
                    Err(AppendError::InvalidUpdate)
                ));
                assert_eq!(env.doc_files(&a), Vec::<String>::new());
            } else if case == "boundary" || case == "small-stack" || case == "threaded" {
                let nested = nested_update(NESTING_LIMIT);
                store.append(&a, &nested).unwrap();
                assert_eq!(store.flush(soon()).docs, []);
                if case == "threaded" {
                    let deadline = Instant::now() + Duration::from_secs(10);
                    while env.doc_files(&a) != [v2_snapshot_name(1)] {
                        assert!(
                            Instant::now() < deadline,
                            "background compaction did not finish"
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                } else {
                    std::thread::scope(|scope| {
                        scope.spawn(|| store.compact(&a).unwrap()).join().unwrap();
                    });
                }
                assert_eq!(env.doc_files(&a), [v2_snapshot_name(1)]);
                store.close(soon()).unwrap();
                let store = env.open();
                let loaded = store.load(&a).unwrap();
                if case != "small-stack" {
                    let (held, _) = applied(&loaded, "reopened boundary");
                    let (expected, _) = applied(&nested, "sent boundary");
                    assert_eq!(held, expected);
                }
                assert_eq!(loaded, nested);
            } else if case == "disk" || case == "resync" || case == "length-disk" {
                let good = typed(7, 3);
                let deep = nested_update(6000);
                store.append(&a, &good).unwrap();
                assert_eq!(store.flush(soon()).docs, []);
                store.close(soon()).unwrap();
                let mut log = encode_segment(&[&deep]);
                if case == "resync" {
                    log.insert(SEGMENT_HEADER, 0xf0);
                } else if case == "length-disk" {
                    log[SEGMENT_HEADER..SEGMENT_HEADER + 4]
                        .copy_from_slice(&u32::MAX.to_le_bytes());
                }
                let mut planted = vec![("log-0000000000-0000000001.ylog", log)];
                if case != "length-disk" {
                    planted.push(("snapshot-0000000000.ysnap", encode_snapshot(&deep)));
                }
                for (name, bytes) in &planted {
                    env.fs.put_durable(&env.path("docs/a").join(name), bytes);
                }
                let stored = env.fs.files();
                let store = env.open();
                std::thread::scope(|scope| {
                    scope
                        .spawn(|| {
                            assert_eq!(store.load(&a).unwrap(), good);
                            store.compact(&a).unwrap();
                        })
                        .join()
                        .unwrap();
                });
                for (name, _) in planted {
                    let original = env.path("docs/a").join(name);
                    let kept = env.fs.files();
                    assert_eq!(
                        kept.get(&original)
                            .or_else(|| kept.get(&env.path("quarantine/a").join(name))),
                        stored.get(&original),
                        "{name} preserved byte for byte"
                    );
                }
                store.close(soon()).unwrap();
                assert_eq!(env.open().load(&a).unwrap(), good);
            } else {
                panic!("unknown case {case}");
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[cfg(unix)]
fn max_rss() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    assert_eq!(
        unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) },
        0
    );
    let rss = unsafe { usage.assume_init() }.ru_maxrss as u64;
    if cfg!(target_os = "macos") {
        rss
    } else {
        rss * 1024
    }
}
