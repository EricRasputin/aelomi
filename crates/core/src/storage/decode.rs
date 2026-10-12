use std::cell::Cell;
use std::io;
use std::panic;

use yrs::Update;
use yrs::encoding::read::{Cursor, Error, Read};
use yrs::updates::decoder::{Decode, DecoderV1};

pub(crate) const STACK_SIZE: usize = 8 * 1024 * 1024;
const STACK_RESERVE: usize = 2 * 1024 * 1024;
// Release Any::decode measured about 350 bytes per level. Allow 16 times that for
// debug frames and other recursive operations, then round the available depth down.
const BYTES_PER_LEVEL: usize = 350 * 16;
pub(crate) const MAX_DEPTH: usize = 1 << ((STACK_SIZE - STACK_RESERVE) / BYTES_PER_LEVEL).ilog2();

thread_local! { static ON_DECODE_STACK: Cell<bool> = const { Cell::new(false) }; }

pub(crate) fn on_stack<R>(f: impl FnOnce() -> R) -> R {
    ON_DECODE_STACK.set(true);
    f()
}

pub(crate) fn with_stack<R: Send>(f: impl FnOnce() -> R + Send) -> io::Result<R> {
    if ON_DECODE_STACK.get() {
        return Ok(f());
    }
    std::thread::scope(|scope| {
        let thread = std::thread::Builder::new()
            .name("aelomi-storage-decode".into())
            .stack_size(STACK_SIZE)
            .spawn_scoped(scope, || on_stack(f))?;
        match thread.join() {
            Ok(result) => Ok(result),
            Err(panic) => panic::resume_unwind(panic),
        }
    })
}

pub(crate) fn update(bytes: &[u8]) -> std::result::Result<Update, Error> {
    check(bytes).map_err(|_| Error::UnexpectedValue)?;
    Update::decode_v1(bytes)
}

/// yrs stops reading at the end of an update, so a payload stretched over the records after it
/// still decodes.
pub(crate) fn decodes_exactly(bytes: &[u8]) -> bool {
    if check(bytes).is_err() {
        return false;
    }
    let mut decoder = DecoderV1::from(bytes);
    Update::decode(&mut decoder).is_ok() && decoder.read_u8().is_err()
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Rejected {
    Malformed,
    TooDeep,
}

impl From<Error> for Rejected {
    fn from(_: Error) -> Self {
        Self::Malformed
    }
}

type Result<T> = std::result::Result<T, Rejected>;

pub(crate) fn check(bytes: &[u8]) -> Result<()> {
    Walker {
        cursor: Cursor::new(bytes),
        parents: [Container::default(); MAX_DEPTH],
    }
    .update()
}

struct Walker<'a> {
    cursor: Cursor<'a>,
    parents: [Container; MAX_DEPTH],
}

#[derive(Clone, Copy, Default)]
struct Container {
    left: usize,
    map: bool,
}

impl Walker<'_> {
    fn count(&self, count: usize, minimum: usize) -> Result<()> {
        if count > (self.cursor.buf.len() - self.cursor.next) / minimum {
            Err(Rejected::Malformed)
        } else {
            Ok(())
        }
    }

    fn len(&mut self) -> Result<usize> {
        Ok(self.cursor.read_var::<u32>()? as usize)
    }

    fn id(&mut self) -> Result<()> {
        self.cursor.read_var::<u64>()?;
        self.cursor.read_var::<u32>()?;
        Ok(())
    }

    fn string(&mut self) -> Result<()> {
        self.cursor.read_buf()?;
        Ok(())
    }

    fn integer(&mut self) -> Result<()> {
        let mut byte = self.cursor.read_u8()?;
        let mut bits = 6;
        while byte >= 128 {
            byte = self.cursor.read_u8()?;
            bits += 7;
            if byte >= 128 && bits > 70 {
                return Err(Rejected::Malformed);
            }
        }
        Ok(())
    }

    fn any(&mut self) -> Result<()> {
        self.parents[0] = Container {
            left: 1,
            map: false,
        };
        let mut level = 0;
        loop {
            if self.parents[level].left == 0 {
                if level == 0 {
                    return Ok(());
                }
                level -= 1;
                continue;
            }
            self.parents[level].left -= 1;
            if self.parents[level].map {
                self.string()?;
            }
            let tag = self.cursor.read_u8()?;
            match tag {
                127 | 126 | 121 | 120 => {}
                125 => self.integer()?,
                124 => {
                    self.cursor.read_exact(4)?;
                }
                123 | 122 => {
                    self.cursor.read_exact(8)?;
                }
                119 | 116 => self.string()?,
                118 | 117 => {
                    let left = self.cursor.read_var::<usize>()?;
                    self.count(left, if tag == 118 { 2 } else { 1 })?;
                    if left != 0 {
                        if level + 1 == MAX_DEPTH {
                            return Err(Rejected::TooDeep);
                        }
                        level += 1;
                        self.parents[level] = Container {
                            left,
                            map: tag == 118,
                        };
                    }
                }
                _ => return Err(Rejected::Malformed),
            }
        }
    }

    fn content(&mut self, info: u8) -> Result<()> {
        match info & 15 {
            1 => {
                self.len()?;
            }
            2 => {
                // yrs 0.28 reads len + 1 JSON strings, including when len is zero.
                let count = self.len()? as i32;
                if count < 0 {
                    return Err(Rejected::Malformed);
                }
                let count = count as usize + 1;
                self.count(count, 1)?;
                for _ in 0..count {
                    self.string()?;
                }
            }
            3..=5 => self.string()?,
            6 => {
                self.string()?;
                self.string()?;
            }
            7 => match self.cursor.read_u8()? {
                3 => self.string()?,
                0 | 1 | 2 | 4 | 5 | 6 | 9 | 15 => {}
                _ => return Err(Rejected::Malformed),
            },
            8 => {
                let count = self.len()?;
                self.count(count, 1)?;
                for _ in 0..count {
                    self.any()?;
                }
            }
            9 => {
                self.string()?;
                self.any()?;
            }
            _ => return Err(Rejected::Malformed),
        }
        Ok(())
    }

    fn block(&mut self) -> Result<()> {
        let info = self.cursor.read_u8()?;
        if info == 0 || info == 10 {
            self.len()?;
            return Ok(());
        }
        if info & 128 != 0 {
            self.id()?;
        }
        if info & 64 != 0 {
            self.id()?;
        }
        if info & 192 == 0 {
            if self.len()? == 1 {
                self.string()?;
            } else {
                self.id()?;
            }
            if info & 32 != 0 {
                self.string()?;
            }
        }
        self.content(info)
    }

    fn update(&mut self) -> Result<()> {
        let clients = self.len()?;
        self.count(clients, 3)?;
        for _ in 0..clients {
            let blocks = self.len()?;
            self.cursor.read_var::<u64>()?;
            self.len()?;
            self.count(blocks, 2)?;
            for _ in 0..blocks {
                self.block()?;
            }
        }
        let clients = self.len()?;
        self.count(clients, 2)?;
        for _ in 0..clients {
            self.cursor.read_var::<u64>()?;
            let ranges = self.len()?;
            self.count(ranges, 2)?;
            for _ in 0..ranges {
                self.len()?;
                self.len()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::panic::catch_unwind;

    use proptest::prelude::*;
    use yrs::encoding::write::Write;
    use yrs::updates::decoder::Decode;
    use yrs::updates::encoder::{Encode, Encoder, EncoderV1};
    use yrs::{Any, Doc, Map, ReadTxn, StateVector, Transact, Update};

    use super::{Rejected, check};
    use crate::storage::testkit::UpdateGen;

    struct CountedAllocator;
    thread_local! {
        static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
        static ORACLE_LIMIT: Cell<bool> = const { Cell::new(false) };
    }

    #[global_allocator]
    static ALLOCATOR: CountedAllocator = CountedAllocator;

    unsafe impl GlobalAlloc for CountedAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if layout.size() > 1024 * 1024 && ORACLE_LIMIT.try_with(Cell::get).unwrap_or(false) {
                return std::ptr::null_mut();
            }
            let _ = ALLOCATIONS.try_with(|count| {
                if let Some(n) = count.get() {
                    count.set(Some(n + 1));
                }
            });
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            if size > 1024 * 1024 && ORACLE_LIMIT.try_with(Cell::get).unwrap_or(false) {
                return std::ptr::null_mut();
            }
            let _ = ALLOCATIONS.try_with(|count| {
                if let Some(n) = count.get() {
                    count.set(Some(n + 1));
                }
            });
            unsafe { System.realloc(ptr, layout, size) }
        }
    }

    fn accepted(bytes: &[u8]) {
        assert!(Update::decode_v1(bytes).is_ok());
        ALLOCATIONS.set(Some(0));
        let result = check(bytes);
        let allocations = ALLOCATIONS.replace(None);
        assert_eq!(result, Ok(()));
        assert_eq!(allocations, Some(0), "walker allocations");
    }

    #[test]
    fn decode_guard_mirrors_the_pinned_yrs_version() {
        let lock = include_str!("../../../../Cargo.lock");
        let versions: Vec<_> = lock
            .split("[[package]]")
            .filter(|package| package.lines().any(|line| line == "name = \"yrs\""))
            .flat_map(|package| {
                package
                    .lines()
                    .filter(|line| line.starts_with("version = "))
            })
            .collect();
        assert_eq!(versions, ["version = \"0.28.0\""]);
    }

    #[test]
    fn decode_guard_accepts_realistic_yrs_updates_without_allocating() {
        for seed in 0..40 {
            let updates = UpdateGen::new(seed, 4).updates(30);
            for update in &updates {
                accepted(update);
            }
            let doc = crate::storage::testkit::doc_from(updates.iter().map(Vec::as_slice));
            accepted(
                &doc.transact()
                    .encode_state_as_update_v1(&StateVector::default()),
            );
        }
        let doc = Doc::with_client_id(17);
        let map = doc.get_or_insert_map("values");
        let values = Any::from_json(
            r#"{"array":[null,true,false,42,3.25,"text"],"object":{"key":"value"}}"#,
        )
        .unwrap();
        let mut txn = doc.transact_mut();
        map.insert(&mut txn, "nested", values);
        map.insert(&mut txn, "buffer", Any::Buffer(vec![0, 128, 255].into()));
        map.insert(&mut txn, "subdoc", Doc::new());
        accepted(&txn.encode_update_v1());
    }

    #[test]
    fn decode_guard_accepts_repo_yjs_wire_fixtures() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/data-folders/format-1/docs");
        let mut count = 0;
        for doc in std::fs::read_dir(root).unwrap() {
            for file in std::fs::read_dir(doc.unwrap().path()).unwrap() {
                let file = file.unwrap();
                let bytes = std::fs::read(file.path()).unwrap();
                if file.path().extension().unwrap() == "ysnap" {
                    let snapshot = crate::storage::snapshot::read(
                        crate::storage::snapshot::SnapshotFormat::V1,
                        &bytes,
                    );
                    accepted(snapshot.unwrap().0);
                    count += 1;
                } else {
                    let records = crate::storage::log::read_log(&bytes);
                    for update in records.records {
                        accepted(update);
                        count += 1;
                    }
                }
            }
        }
        assert_eq!(count, 7);
    }

    fn item(content: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![1, 1, 1, 0, content, 1, 0];
        bytes.extend_from_slice(payload);
        bytes.push(0);
        bytes
    }

    #[test]
    fn decode_guard_accepts_minimum_and_overlong_encodings() {
        for bytes in [
            vec![0, 0],
            vec![0, 0, 255],
            vec![1, 0, 0, 0, 0],
            vec![1, 1, 1, 0, 0, 1, 0],
            vec![1, 1, 1, 0, 10, 1, 0],
            vec![0x80, 0x80, 0x80, 0x80, 0x10, 0],
        ] {
            accepted(&bytes);
        }
        for payload in [
            vec![0],
            vec![1, 127],
            vec![1, 126],
            vec![1, 121],
            vec![1, 120],
            vec![1, 125, 0],
            vec![1, 124, 0, 0, 0, 0],
            vec![1, 123, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 122, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 119, 0],
            vec![1, 116, 0],
            vec![1, 117, 0],
            vec![1, 118, 0],
            vec![1, 118, 1, 0, 126],
            vec![1, 117, 1, 126],
        ] {
            accepted(&item(8, &payload));
        }
        for tag in [0, 1, 2, 3, 4, 5, 6, 9, 15] {
            let payload = if tag == 3 { vec![3, 0] } else { vec![tag] };
            accepted(&item(7, &payload));
        }
        for (tag, payload) in [
            (1, vec![0]),
            (2, vec![0, 0]),
            (3, vec![0]),
            (4, vec![0]),
            (0x14, vec![0]),
            (5, vec![4, b'n', b'u', b'l', b'l']),
            (6, vec![0, 4, b'n', b'u', b'l', b'l']),
            (9, vec![0, 126]),
        ] {
            accepted(&item(tag, &payload));
        }
        let mut overlong_parent = item(8, &[1, 126]);
        overlong_parent.splice(5..6, [0x81, 0]);
        accepted(&overlong_parent);
    }

    #[test]
    fn decode_guard_exact_decode_rejects_trailing_bytes() {
        assert!(super::decodes_exactly(&[0, 0]));
        for bytes in [
            &[0][..],
            &[0, 0, 0],
            &[0, 0, 255],
            &[0x80, 0x80, 0x80, 0x80, 1],
        ] {
            assert!(!super::decodes_exactly(bytes));
        }
    }

    #[test]
    fn decode_guard_runtime_json_keeps_its_recursion_limit() {
        let json = format!("{}null{}", "[".repeat(200), "]".repeat(200));
        let mut encoder = EncoderV1::new();
        encoder.write_string(&json);
        let update = item(5, &encoder.to_vec());
        assert_eq!(check(&update), Ok(()));
        assert!(Update::decode_v1(&update).is_err());
        accepted(&item(5, &[4, b'n', b'u', b'l', b'l']));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20_000))]
        #[test]
        fn decode_guard_rejects_only_bytes_yrs_cannot_decode(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
            if check(&bytes) == Err(Rejected::Malformed) {
                // A valid 512-byte update needs far less than 1 MiB per allocation.
                // Invalid counts can otherwise make the oracle touch gigabytes.
                ORACLE_LIMIT.set(true);
                let decoded = catch_unwind(|| Update::decode_v1(&bytes));
                ORACLE_LIMIT.set(false);
                prop_assert!(!matches!(decoded, Ok(Ok(_))), "yrs accepted walker-rejected bytes: {bytes:02x?}");
            }
        }
    }

    #[test]
    fn decode_guard_checks_every_reservation_count() {
        for bytes in [
            vec![10],
            vec![1, 10, 1, 0],
            item(2, &[10]),
            item(8, &[10]),
            item(8, &[1, 118, 10]),
            item(8, &[1, 117, 10]),
            item(9, &[0, 118, 10]),
            vec![0, 10],
            vec![0, 1, 1, 10],
        ] {
            assert_eq!(check(&bytes), Err(Rejected::Malformed));
            assert!(Update::decode_v1(&bytes).is_err());
        }
        accepted(&Update::default().encode_v1());
    }
}
