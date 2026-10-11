use std::collections::{BTreeMap, HashMap, HashSet};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::clock::Clock;
use super::compact::covers;
use super::equality::Ops;
use super::fs::{Fs, FsLock, LockError};
use super::layout::DocFile;
use super::snapshot::read_snapshot;
use super::testkit::SplitMix64;
use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum OpKind {
    CreateDir,
    Create,
    Append,
    Fsync,
    FullSync,
    Rename,
    Remove,
    List,
    Read,
    Lock,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Op {
    pub kind: OpKind,
    pub path: PathBuf,
    pub to: Option<PathBuf>,
    pub ok: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Crash {
    DropUnsynced,
    KeepTornPrefix(u64),
}

struct Fault {
    kind: OpKind,
    at: FaultPath,
    skip: usize,
    effect: Effect,
}

#[derive(Clone, Copy)]
enum Effect {
    Fail(i32),
    PanicOnce,
}

enum FaultPath {
    Containing(String),
    Exactly(PathBuf),
}

impl FaultPath {
    fn matches(&self, path: &Path) -> bool {
        match self {
            Self::Containing(part) => path.to_string_lossy().contains(part),
            Self::Exactly(at) => path == at,
        }
    }
}

pub(crate) struct SimFs {
    sim: Mutex<Sim>,
    locks: Arc<Mutex<HashMap<PathBuf, u64>>>,
    clock: Arc<dyn Clock>,
}

#[derive(Default)]
struct Sim {
    nodes: Vec<Node>,
    ops: Vec<Op>,
    faults: Vec<Fault>,
    latency: HashMap<OpKind, Duration>,
    ops_until_death: Option<usize>,
    session: u64,
    created_this_session: HashSet<usize>,
    failed: HashSet<usize>,
    violations: Vec<String>,
}

enum Node {
    File(FileNode),
    Dir(DirNode),
}

#[derive(Default)]
struct FileNode {
    data: Vec<u8>,
    synced: usize,
    durable: usize,
}

#[derive(Default)]
struct DirNode {
    live: BTreeMap<String, usize>,
    synced: BTreeMap<String, usize>,
    durable: BTreeMap<String, usize>,
}

const ROOT: usize = 0;

impl SimFs {
    pub fn new(clock: Arc<dyn Clock>) -> Arc<Self> {
        let sim = Sim {
            nodes: vec![Node::Dir(DirNode::default())],
            ..Sim::default()
        };
        Arc::new(Self {
            sim: Mutex::new(sim),
            locks: Arc::default(),
            clock,
        })
    }

    fn sim(&self) -> MutexGuard<'_, Sim> {
        self.sim
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set_latency(&self, kind: OpKind, latency: Duration) {
        self.sim().latency.insert(kind, latency);
    }

    pub fn fail(&self, kind: OpKind, path_contains: &str, errno: i32) {
        let at = FaultPath::Containing(path_contains.to_owned());
        self.sim().faults.push(Fault {
            kind,
            at,
            skip: 0,
            effect: Effect::Fail(errno),
        });
    }

    pub fn fail_at(&self, kind: OpKind, path: &Path, errno: i32) {
        self.fail_at_after(kind, path, 0, errno);
    }

    pub fn fail_at_after(&self, kind: OpKind, path: &Path, skip: usize, errno: i32) {
        let at = FaultPath::Exactly(path.to_path_buf());
        self.sim().faults.push(Fault {
            kind,
            at,
            skip,
            effect: Effect::Fail(errno),
        });
    }

    pub fn panic_once(&self, kind: OpKind, path_contains: &str) {
        let at = FaultPath::Containing(path_contains.to_owned());
        self.sim().faults.push(Fault {
            kind,
            at,
            skip: 0,
            effect: Effect::PanicOnce,
        });
    }

    pub fn clear_faults(&self) {
        self.sim().faults.clear();
    }

    pub fn die_after(&self, ops: usize) {
        self.sim().ops_until_death = Some(ops);
    }

    pub fn crash(&self, mode: Crash) {
        let mut sim = self.sim();
        let mut rng = match mode {
            Crash::DropUnsynced => None,
            Crash::KeepTornPrefix(seed) => Some(SplitMix64::new(seed)),
        };
        for node in &mut sim.nodes {
            match node {
                Node::Dir(dir) => {
                    dir.live = dir.durable.clone();
                    dir.synced = dir.durable.clone();
                }
                Node::File(file) => {
                    let unsynced = file.data.len() - file.durable;
                    let kept = rng.as_mut().map_or(0, |rng| rng.below(unsynced as u64 + 1));
                    file.data.truncate(file.durable + kept as usize);
                    file.synced = file.data.len();
                    file.durable = file.data.len();
                }
            }
        }
        sim.ops_until_death = None;
        sim.session += 1;
        sim.created_this_session.clear();
        sim.failed.clear();
        self.locks.lock().unwrap().clear();
    }

    pub fn ops(&self) -> Vec<Op> {
        self.sim().ops.clone()
    }

    pub fn violations(&self) -> Vec<String> {
        self.sim().violations.clone()
    }

    pub fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        self.tree(|dir| &dir.live, |file| &file.data)
    }

    pub fn durable_files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        self.tree(|dir| &dir.durable, |file| &file.data[..file.durable])
    }

    fn tree(
        &self,
        entries: impl Fn(&DirNode) -> &BTreeMap<String, usize>,
        bytes: impl Fn(&FileNode) -> &[u8],
    ) -> BTreeMap<PathBuf, Vec<u8>> {
        let sim = self.sim();
        let mut out = BTreeMap::new();
        let mut stack = vec![(PathBuf::from("/"), ROOT)];
        while let Some((path, id)) = stack.pop() {
            match &sim.nodes[id] {
                Node::Dir(dir) => stack.extend(
                    entries(dir)
                        .iter()
                        .map(|(name, &child)| (path.join(name), child)),
                ),
                Node::File(file) => {
                    out.insert(path, bytes(file).to_vec());
                }
            }
        }
        out
    }

    pub fn put_durable(&self, path: &Path, bytes: &[u8]) {
        let mut sim = self.sim();
        let mut dir = ROOT;
        let parent: Vec<String> = names(path.parent().unwrap());
        for name in parent {
            dir = match sim.dir(dir).live.get(&name) {
                Some(&child) => child,
                None => sim.insert_durable(dir, &name, Node::Dir(DirNode::default())),
            };
        }
        let name = file_name(path);
        let len = bytes.len();
        let file = FileNode {
            data: bytes.to_vec(),
            synced: len,
            durable: len,
        };
        sim.insert_durable(dir, &name, Node::File(file));
    }

    pub fn flip_bit(&self, path: &Path, offset: usize) {
        let mut sim = self.sim();
        let id = sim.lookup(path).expect("file exists");
        let Node::File(file) = &mut sim.nodes[id] else {
            panic!("not a file")
        };
        file.data[offset] ^= 0x10;
    }

    fn run<T>(
        &self,
        kind: OpKind,
        path: &Path,
        to: Option<&Path>,
        bytes: &[u8],
        f: impl FnOnce(&mut Sim) -> io::Result<T>,
    ) -> io::Result<T> {
        let latency = self.sim().latency.get(&kind).copied();
        if let Some(latency) = latency {
            self.clock.sleep(latency);
        }
        let mut sim = self.sim();
        sim.check(kind, path, to);
        match &mut sim.ops_until_death {
            Some(0) => return Err(io::Error::other("simulated process death")),
            Some(left) => *left -= 1,
            None => {}
        }
        let result = match sim.take_fault(kind, path) {
            Some(Effect::Fail(errno)) => {
                sim.fail_op(kind, path, bytes);
                Err(io::Error::from_raw_os_error(errno))
            }
            Some(Effect::PanicOnce) => {
                drop(sim);
                panic!("simulated panic during {kind:?} on {path:?}");
            }
            None => f(&mut sim),
        };
        let ok = result.is_ok();
        let (path, to) = (path.to_path_buf(), to.map(Path::to_path_buf));
        sim.ops.push(Op { kind, path, to, ok });
        result
    }
}

impl Sim {
    fn take_fault(&mut self, kind: OpKind, path: &Path) -> Option<Effect> {
        let at = self
            .faults
            .iter()
            .position(|f| f.kind == kind && f.at.matches(path))?;
        let fault = &mut self.faults[at];
        if fault.skip > 0 {
            fault.skip -= 1;
            return None;
        }
        let effect = fault.effect;
        if let Effect::PanicOnce = effect {
            self.faults.remove(at);
        }
        Some(effect)
    }

    fn dir(&self, id: usize) -> &DirNode {
        match &self.nodes[id] {
            Node::Dir(dir) => dir,
            Node::File(_) => panic!("not a directory"),
        }
    }

    fn dir_mut(&mut self, id: usize) -> &mut DirNode {
        match &mut self.nodes[id] {
            Node::Dir(dir) => dir,
            Node::File(_) => panic!("not a directory"),
        }
    }

    fn insert_durable(&mut self, dir: usize, name: &str, node: Node) -> usize {
        let id = self.nodes.len();
        self.nodes.push(node);
        let dir = self.dir_mut(dir);
        for layer in [&mut dir.live, &mut dir.synced, &mut dir.durable] {
            layer.insert(name.to_owned(), id);
        }
        id
    }

    fn lookup(&self, path: &Path) -> Option<usize> {
        let mut id = ROOT;
        for name in names(path) {
            let Node::Dir(dir) = &self.nodes[id] else {
                return None;
            };
            id = *dir.live.get(&name)?;
        }
        Some(id)
    }

    fn parent(&self, path: &Path) -> io::Result<(usize, String)> {
        let parent = path.parent().ok_or_else(not_found)?;
        match self.lookup(parent) {
            Some(id) if matches!(self.nodes[id], Node::Dir(_)) => Ok((id, file_name(path))),
            _ => Err(not_found()),
        }
    }

    fn file(&mut self, path: &Path) -> io::Result<(usize, &mut FileNode)> {
        let id = self.lookup(path).ok_or_else(not_found)?;
        match &mut self.nodes[id] {
            Node::File(file) => Ok((id, file)),
            Node::Dir(_) => Err(io::Error::from(io::ErrorKind::IsADirectory)),
        }
    }

    fn add(&mut self, path: &Path, node: Node) -> io::Result<usize> {
        let (dir, name) = self.parent(path)?;
        if self.dir(dir).live.contains_key(&name) {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        let id = self.nodes.len();
        self.nodes.push(node);
        self.dir_mut(dir).live.insert(name, id);
        Ok(id)
    }

    fn fail_op(&mut self, kind: OpKind, path: &Path, bytes: &[u8]) {
        let Some(id) = self.lookup(path) else { return };
        if let (OpKind::Append, Node::File(file)) = (kind, &mut self.nodes[id]) {
            file.data.extend_from_slice(&bytes[..bytes.len() / 2]);
        }
        if matches!(kind, OpKind::Append | OpKind::Fsync | OpKind::FullSync) {
            self.failed.insert(id);
        }
    }

    fn check(&mut self, kind: OpKind, path: &Path, to: Option<&Path>) {
        let Some(id) = self.lookup(path) else { return };
        match kind {
            OpKind::Append => {
                if !self.created_this_session.contains(&id) {
                    self.violation(format!("append to {path:?}, created before this session"));
                }
                if self.failed.contains(&id) {
                    self.violation(format!("append to {path:?} after a failed write or sync"));
                }
            }
            OpKind::Rename => {
                let to = to.expect("a rename has a target");
                if self.lookup(to).is_some() {
                    self.violation(format!("rename {path:?} onto existing {to:?}"));
                }
            }
            OpKind::Remove => {
                let (Ok((dir, name)), Node::File(removed)) = (self.parent(path), &self.nodes[id])
                else {
                    return;
                };
                let covered = match DocFile::parse(&name) {
                    Some(DocFile::SnapshotTmp(_)) => true,
                    Some(file) => self.durable_snapshot_covers(dir, file, &removed.data),
                    None => name.ends_with(".tmp"),
                };
                if !covered {
                    self.violation(format!(
                        "remove {path:?}, not a .tmp or covered by a durable snapshot"
                    ));
                }
            }
            _ => {}
        }
    }

    fn violation(&mut self, message: String) -> ! {
        self.violations.push(message.clone());
        panic!("append-only violation: {message}");
    }

    fn fsync(&mut self, path: &Path) -> io::Result<()> {
        let id = self.lookup(path).ok_or_else(not_found)?;
        match &mut self.nodes[id] {
            Node::File(file) => file.synced = file.data.len(),
            Node::Dir(dir) => dir.synced = dir.live.clone(),
        }
        Ok(())
    }

    fn durable_snapshot_covers(&self, dir: usize, file: DocFile, bytes: &[u8]) -> bool {
        self.dir(dir).durable.iter().any(|(name, &id)| {
            let newer =
                matches!(DocFile::parse(name), Some(DocFile::Snapshot(g)) if g > file.generation());
            newer
                && matches!(&self.nodes[id], Node::File(f) if f.durable == f.data.len()
                    && read_snapshot(&f.data)
                        .filter(|payload| applies(payload))
                        .and_then(|payload| Update::decode_v1(payload).ok())
                        .is_some_and(|update| covers(&Ops::of_update(&update), file, bytes)))
        })
    }
}

impl Fs for SimFs {
    fn create_dir(&self, path: &Path) -> io::Result<()> {
        self.run(OpKind::CreateDir, path, None, &[], |sim| {
            sim.add(path, Node::Dir(DirNode::default())).map(drop)
        })
    }

    fn create_new(&self, path: &Path) -> io::Result<()> {
        self.run(OpKind::Create, path, None, &[], |sim| {
            let id = sim.add(path, Node::File(FileNode::default()))?;
            sim.created_this_session.insert(id);
            Ok(())
        })
    }

    fn append(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.run(OpKind::Append, path, None, bytes, |sim| {
            sim.file(path)?.1.data.extend_from_slice(bytes);
            Ok(())
        })
    }

    fn fsync(&self, path: &Path) -> io::Result<()> {
        self.run(OpKind::Fsync, path, None, &[], |sim| sim.fsync(path))
    }

    fn full_sync(&self, path: &Path) -> io::Result<()> {
        self.run(OpKind::FullSync, path, None, &[], |sim| {
            sim.fsync(path)?;
            for node in &mut sim.nodes {
                match node {
                    Node::File(file) => file.durable = file.synced,
                    Node::Dir(dir) => dir.durable = dir.synced.clone(),
                }
            }
            Ok(())
        })
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.run(OpKind::Rename, from, Some(to), &[], |sim| {
            let (from_dir, from_name) = sim.parent(from)?;
            let (to_dir, to_name) = sim.parent(to)?;
            let id = *sim
                .dir(from_dir)
                .live
                .get(&from_name)
                .ok_or_else(not_found)?;
            sim.dir_mut(from_dir).live.remove(&from_name);
            sim.dir_mut(to_dir).live.insert(to_name, id);
            Ok(())
        })
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        self.run(OpKind::Remove, path, None, &[], |sim| {
            let (dir, name) = sim.parent(path)?;
            sim.file(path)?;
            sim.dir_mut(dir).live.remove(&name);
            Ok(())
        })
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<String>> {
        self.run(OpKind::List, dir, None, &[], |sim| {
            let id = sim.lookup(dir).ok_or_else(not_found)?;
            match &sim.nodes[id] {
                Node::Dir(d) => Ok(d.live.keys().cloned().collect()),
                Node::File(_) => Err(io::Error::from(io::ErrorKind::NotADirectory)),
            }
        })
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.run(OpKind::Read, path, None, &[], |sim| {
            Ok(sim.file(path)?.1.data.clone())
        })
    }

    fn lock(&self, path: &Path) -> Result<FsLock, LockError> {
        let session = self.run(OpKind::Lock, path, None, &[], |sim| {
            if sim.lookup(path).is_none() {
                sim.add(path, Node::File(FileNode::default()))?;
            }
            Ok(sim.session)
        });
        let session = session.map_err(LockError::Io)?;
        let mut locks = self.locks.lock().unwrap();
        if locks.contains_key(path) {
            return Err(LockError::Held);
        }
        locks.insert(path.to_path_buf(), session);
        let guard = SimLock {
            locks: self.locks.clone(),
            path: path.to_path_buf(),
            session,
        };
        Ok(FsLock::new(guard))
    }
}

struct SimLock {
    locks: Arc<Mutex<HashMap<PathBuf, u64>>>,
    path: PathBuf,
    session: u64,
}

impl Drop for SimLock {
    fn drop(&mut self) {
        let mut locks = self.locks.lock().unwrap_or_else(|p| p.into_inner());
        if locks.get(&self.path) == Some(&self.session) {
            locks.remove(&self.path);
        }
    }
}

fn names(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

fn not_found() -> io::Error {
    io::Error::from(io::ErrorKind::NotFound)
}

fn applies(payload: &[u8]) -> bool {
    Update::decode_v1(payload)
        .is_ok_and(|update| Doc::new().transact_mut().apply_update(update).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::clock::ManualClock;
    use crate::storage::log::{LOG_MAGIC, encode_record};
    use crate::storage::snapshot::encode_snapshot;
    use crate::storage::testkit::UpdateGen;

    fn sim() -> Arc<SimFs> {
        SimFs::new(Arc::new(ManualClock::default()))
    }

    fn p(path: &str) -> PathBuf {
        PathBuf::from(path)
    }

    #[test]
    fn crash_keeps_only_what_full_sync_made_durable() {
        let fs = sim();
        fs.create_dir(&p("/d")).unwrap();
        fs.create_new(&p("/d/a")).unwrap();
        fs.append(&p("/d/a"), b"one").unwrap();
        fs.fsync(&p("/d/a")).unwrap();
        fs.fsync(&p("/d")).unwrap();
        fs.fsync(&p("/")).unwrap();
        fs.full_sync(&p("/d/a")).unwrap();
        fs.append(&p("/d/a"), b"two").unwrap();
        fs.fsync(&p("/d/a")).unwrap();
        fs.create_new(&p("/d/b")).unwrap();
        fs.crash(Crash::DropUnsynced);
        assert_eq!(fs.files(), BTreeMap::from([(p("/d/a"), b"one".to_vec())]));
    }

    #[test]
    fn crash_can_keep_a_torn_prefix() {
        let kept: HashSet<Vec<u8>> = (0..40)
            .map(|seed| {
                let fs = sim();
                fs.create_new(&p("/a")).unwrap();
                fs.fsync(&p("/")).unwrap();
                fs.full_sync(&p("/a")).unwrap();
                fs.append(&p("/a"), b"0123456789").unwrap();
                fs.crash(Crash::KeepTornPrefix(seed));
                fs.files()[&p("/a")].clone()
            })
            .collect();
        assert!(
            kept.iter().all(|k| b"0123456789".starts_with(k)),
            "{kept:?}"
        );
        assert!(kept.len() > 3, "torn prefixes vary with the seed: {kept:?}");
    }

    #[test]
    fn rename_and_remove_follow_durable_directory_entries() {
        let fs = sim();
        fs.create_new(&p("/a.tmp")).unwrap();
        fs.append(&p("/a.tmp"), b"x").unwrap();
        fs.fsync(&p("/a.tmp")).unwrap();
        fs.fsync(&p("/")).unwrap();
        fs.full_sync(&p("/")).unwrap();
        fs.rename(&p("/a.tmp"), &p("/a")).unwrap();
        fs.crash(Crash::DropUnsynced);
        assert_eq!(fs.list(&p("/")).unwrap(), ["a.tmp"]);
        fs.rename(&p("/a.tmp"), &p("/a")).unwrap();
        fs.fsync(&p("/")).unwrap();
        fs.full_sync(&p("/")).unwrap();
        fs.crash(Crash::DropUnsynced);
        assert_eq!(fs.files(), BTreeMap::from([(p("/a"), b"x".to_vec())]));
    }

    #[test]
    fn faults_and_death_fail_operations() {
        let fs = sim();
        fs.create_new(&p("/a")).unwrap();
        fs.fail(OpKind::Append, "/a", libc::ENOSPC);
        let error = fs.append(&p("/a"), b"abcdefgh").unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::ENOSPC));
        assert_eq!(
            fs.files()[&p("/a")],
            b"abcd",
            "a failed append leaves half its bytes"
        );
        fs.clear_faults();
        fs.die_after(1);
        fs.create_new(&p("/b")).unwrap();
        assert!(fs.create_new(&p("/c")).is_err());
        assert_eq!(
            fs.list(&p("/")).map_err(|e| e.to_string()),
            Err("simulated process death".into())
        );
    }

    #[test]
    fn append_only_rules_are_checked_as_operations_run() {
        let caught = |f: &dyn Fn(&SimFs)| {
            let fs = sim();
            let fs2 = fs.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&fs2)));
            assert!(result.is_err());
            fs.violations()
        };
        let msgs = caught(&|fs| {
            fs.put_durable(&p("/old"), b"x");
            fs.append(&p("/old"), b"y").unwrap();
        });
        assert_eq!(msgs, [r#"append to "/old", created before this session"#]);
        let msgs = caught(&|fs| {
            fs.create_new(&p("/a")).unwrap();
            fs.fail(OpKind::Append, "/a", libc::EIO);
            fs.append(&p("/a"), b"y").unwrap_err();
            fs.clear_faults();
            fs.append(&p("/a"), b"y").unwrap();
        });
        assert_eq!(msgs, [r#"append to "/a" after a failed write or sync"#]);
        let msgs = caught(&|fs| {
            fs.put_durable(&p("/a"), b"x");
            fs.put_durable(&p("/b"), b"y");
            fs.rename(&p("/a"), &p("/b")).unwrap();
        });
        assert_eq!(msgs, [r#"rename "/a" onto existing "/b""#]);
        let msgs = caught(&|fs| {
            fs.put_durable(&p("/d/log-0000000001-0000000000.ylog"), b"x");
            fs.put_durable(&p("/d/snapshot-0000000001.ysnap"), b"y");
            fs.remove(&p("/d/log-0000000001-0000000000.ylog")).unwrap();
        });
        assert_eq!(
            msgs,
            [
                r#"remove "/d/log-0000000001-0000000000.ylog", not a .tmp or covered by a durable snapshot"#
            ]
        );

        for (name, snapshot) in [
            ("fails its CRC", b"y".to_vec()),
            ("is not a Yjs update", encode_snapshot(b"not yjs")),
        ] {
            let msgs = caught(&|fs| {
                fs.put_durable(&p("/d/log-0000000001-0000000000.ylog"), b"x");
                fs.put_durable(&p("/d/snapshot-0000000002.ysnap"), &snapshot);
                fs.remove(&p("/d/log-0000000001-0000000000.ylog")).unwrap();
            });
            assert_eq!(
                msgs,
                [
                    r#"remove "/d/log-0000000001-0000000000.ylog", not a .tmp or covered by a durable snapshot"#
                ],
                "a newer snapshot that {name} covers nothing"
            );
        }

        let held = UpdateGen::new(1, 1).next_update();
        let log = |update: &[u8]| {
            let mut bytes = LOG_MAGIC.to_vec();
            encode_record(update, &mut bytes);
            bytes
        };
        for (name, log) in [
            (
                "an update it lacks",
                log(&UpdateGen::new(2, 1).next_update()),
            ),
            ("corrupt bytes", b"x".to_vec()),
        ] {
            let msgs = caught(&|fs| {
                fs.put_durable(&p("/d/log-0000000001-0000000000.ylog"), &log);
                fs.put_durable(&p("/d/snapshot-0000000002.ysnap"), &encode_snapshot(&held));
                fs.remove(&p("/d/log-0000000001-0000000000.ylog")).unwrap();
            });
            assert_eq!(
                msgs,
                [
                    r#"remove "/d/log-0000000001-0000000000.ylog", not a .tmp or covered by a durable snapshot"#
                ],
                "a newer snapshot doesn't cover a log with {name}"
            );
        }

        let fs = sim();
        fs.put_durable(&p("/d/log-0000000001-0000000000.ylog"), &log(&held));
        fs.put_durable(&p("/d/snapshot-0000000002.ysnap"), &encode_snapshot(&held));
        fs.put_durable(&p("/d/snapshot-0000000003.ysnap.tmp"), b"z");
        fs.remove(&p("/d/log-0000000001-0000000000.ylog")).unwrap();
        fs.remove(&p("/d/snapshot-0000000003.ysnap.tmp")).unwrap();
        assert_eq!(fs.list(&p("/d")).unwrap(), ["snapshot-0000000002.ysnap"]);
        assert_eq!(fs.violations(), Vec::<String>::new());
    }

    #[test]
    fn append_only_rules_are_checked_before_faults_are_injected() {
        let caught = |kind: OpKind, setup: &dyn Fn(&SimFs), op: &dyn Fn(&SimFs)| {
            let fs = sim();
            setup(&fs);
            fs.fail(kind, "/", libc::EIO);
            let fs2 = fs.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| op(&fs2)));
            assert!(result.is_err(), "{kind:?} is a violation");
            fs.violations()
        };
        let msgs = caught(
            OpKind::Append,
            &|fs| fs.put_durable(&p("/old"), b"x"),
            &|fs| drop(fs.append(&p("/old"), b"y")),
        );
        assert_eq!(msgs, [r#"append to "/old", created before this session"#]);
        let msgs = caught(
            OpKind::Append,
            &|fs| fs.create_new(&p("/a")).unwrap(),
            &|fs| {
                fs.append(&p("/a"), b"y").unwrap_err();
                drop(fs.append(&p("/a"), b"y"));
            },
        );
        assert_eq!(msgs, [r#"append to "/a" after a failed write or sync"#]);
        let msgs = caught(
            OpKind::Rename,
            &|fs| {
                fs.put_durable(&p("/a"), b"x");
                fs.put_durable(&p("/b"), b"y");
            },
            &|fs| drop(fs.rename(&p("/a"), &p("/b"))),
        );
        assert_eq!(msgs, [r#"rename "/a" onto existing "/b""#]);
        let msgs = caught(
            OpKind::Remove,
            &|fs| fs.put_durable(&p("/d/log-0000000001-0000000000.ylog"), b"x"),
            &|fs| drop(fs.remove(&p("/d/log-0000000001-0000000000.ylog"))),
        );
        assert_eq!(
            msgs,
            [
                r#"remove "/d/log-0000000001-0000000000.ylog", not a .tmp or covered by a durable snapshot"#
            ]
        );
    }

    #[test]
    fn locks_conflict_until_dropped_or_crashed() {
        let fs = sim();
        let first = fs.lock(&p("/.lock")).unwrap();
        assert!(matches!(fs.lock(&p("/.lock")), Err(LockError::Held)));
        drop(first);
        let _second = fs.lock(&p("/.lock")).unwrap();
        fs.crash(Crash::DropUnsynced);
        let _third = fs.lock(&p("/.lock")).unwrap();
    }

    #[test]
    fn latency_advances_the_clock() {
        let clock = Arc::new(ManualClock::default());
        let fs = SimFs::new(clock.clone());
        fs.set_latency(OpKind::FullSync, Duration::from_millis(6));
        fs.set_latency(OpKind::Fsync, Duration::from_micros(500));
        fs.create_new(&p("/a")).unwrap();
        fs.fsync(&p("/a")).unwrap();
        fs.full_sync(&p("/a")).unwrap();
        assert_eq!(clock.now(), Duration::from_micros(6500));
    }
}
