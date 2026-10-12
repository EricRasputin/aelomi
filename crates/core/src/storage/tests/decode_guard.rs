use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use yrs::encoding::write::Write;
use yrs::updates::encoder::{Encoder, EncoderV1};

use super::{Env, applied, doc, soon, typed};
use crate::storage::log::encode_segment;
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
        assert!(
            growth < 32 * 1024 * 1024,
            "append increased maximum RSS by {growth} bytes"
        );
        assert_eq!(env.doc_files(&doc("a")), Vec::<String>::new());
        return;
    }
    // Main can accept the original payload; the compactor still has its default stack.
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            let env = Env::manual();
            let a = doc("a");
            let store = env.open();
            if case == "above" {
                assert!(matches!(
                    store.append(&a, &nested_update(NESTING_LIMIT + 1)),
                    Err(AppendError::InvalidUpdate)
                ));
                assert_eq!(env.doc_files(&a), Vec::<String>::new());
            } else if case == "boundary" {
                let nested = nested_update(NESTING_LIMIT);
                store.append(&a, &nested).unwrap();
                assert_eq!(store.flush(soon()), []);
                std::thread::scope(|scope| {
                    scope.spawn(|| store.compact(&a).unwrap()).join().unwrap();
                });
                assert_eq!(env.doc_files(&a), ["snapshot-0000000001.ysnap"]);
                store.close(soon()).unwrap();
                let store = env.open();
                let loaded = store.load(&a).unwrap();
                let (held, _) = applied(&loaded, "reopened boundary");
                let (expected, _) = applied(&nested, "sent boundary");
                assert_eq!(held, expected);
                assert_eq!(loaded, nested);
            } else if case == "disk" {
                let good = typed(7, 3);
                let deep = nested_update(6000);
                store.append(&a, &good).unwrap();
                assert_eq!(store.flush(soon()), []);
                store.close(soon()).unwrap();
                for (name, bytes) in [
                    ("log-0000000000-0000000001.ylog", encode_segment(&[&deep])),
                    ("snapshot-0000000000.ysnap", encode_snapshot(&deep)),
                ] {
                    env.fs.put_durable(&env.path("docs/a").join(name), &bytes);
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
                for name in [
                    "log-0000000000-0000000001.ylog",
                    "snapshot-0000000000.ysnap",
                ] {
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
