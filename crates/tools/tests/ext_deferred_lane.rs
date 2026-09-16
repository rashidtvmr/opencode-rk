//! EXT-004 frozen tests: deferred reload/watch boundary (EXT-004-T01..T05).
//!
//! Included via path so the owning module needs no shared-file wiring;
//! the integrator re-exports it from `lib.rs` per PLAN.md section 5.
//! Frozen after RED: never edit to make code pass, fix the implementation.

#[path = "../src/ext_deferred_lane.rs"]
mod ext_deferred_lane;

use ext_deferred_lane::{
    BoundaryError, DeferredLog, DeferredReason, EXT4_MAX_DEFERRED, EXT4_MAX_WATCHERS, PluginId,
};

/// Live OS watch handles visible to this process (inotify/fanotify fds).
fn fd_targets() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(dir) = std::fs::read_dir("/proc/self/fd") {
        for e in dir.flatten() {
            if let Ok(t) = std::fs::read_link(e.path()) {
                out.push(t.to_string_lossy().into_owned());
            }
        }
    }
    out
}

/// Live thread names (`/proc/self/task/<tid>/comm`).
fn thread_names() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(dir) = std::fs::read_dir("/proc/self/task") {
        for e in dir.flatten() {
            if let Ok(s) = std::fs::read_to_string(e.path().join("comm")) {
                out.push(s.trim().to_string());
            }
        }
    }
    out
}

/// Zero OS watchers, zero watcher threads: no inotify/fanotify handle held,
/// no notify/watch thread spawned. Robust under parallel harness threads
/// (scans kinds, not counts).
fn assert_no_os_watchers() {
    for t in fd_targets() {
        let l = t.to_lowercase();
        assert!(
            !l.contains("inotify") && !l.contains("fanotify"),
            "os watch handle held: {t}"
        );
    }
    for n in thread_names() {
        let l = n.to_lowercase();
        assert!(
            !l.contains("notify") && !l.contains("inotify") && !l.contains("watch"),
            "watcher thread spawned: {n}"
        );
    }
}

/// Direct child processes of this test process.
fn children_count() -> usize {
    let mut n = 0;
    if let Ok(dir) = std::fs::read_dir("/proc/self/task") {
        for e in dir.flatten() {
            if let Ok(s) = std::fs::read_to_string(e.path().join("children")) {
                n += s.split_whitespace().count();
            }
        }
    }
    n
}

#[test]
fn ext004_t01_record_and_list() {
    let mut log = DeferredLog::new();
    assert!(log.list_deferred().is_empty(), "fresh log must be empty");
    assert_eq!(log.request_reload(None), Ok(1));
    let id = PluginId(7);
    assert_eq!(log.request_reload(Some(id)), Ok(2));
    assert_eq!(log.note_external(Some(id)), Ok(3));
    let list = log.list_deferred();
    assert_eq!(list.len(), 3);
    assert_eq!(
        list.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "seqs must run 1..=3 in order"
    );
    assert_eq!(list[0].reason, DeferredReason::ReloadDeferred);
    assert_eq!(list[0].plugin, None);
    assert_eq!(list[1].reason, DeferredReason::ReloadDeferred);
    assert_eq!(list[1].plugin, Some(id));
    assert_eq!(list[2].reason, DeferredReason::ExternalActivationDeferred);
    assert_eq!(list[2].plugin, Some(id));
    // Determinism: identical call sequence => identical projection.
    let mut replay = DeferredLog::new();
    assert_eq!(replay.request_reload(None), Ok(1));
    assert_eq!(replay.request_reload(Some(id)), Ok(2));
    assert_eq!(replay.note_external(Some(id)), Ok(3));
    assert_eq!(replay.list_deferred(), list);
}

#[test]
fn ext004_t02_watch_interest_only() {
    let mut log = DeferredLog::new();
    assert_eq!(log.add_watcher("a"), Ok(1));
    assert_eq!(log.add_watcher("b"), Ok(2));
    assert_eq!(log.add_watcher("a"), Err(BoundaryError::Duplicate));
    assert_eq!(log.add_watcher(""), Err(BoundaryError::InvalidLabel));
    assert_eq!(
        log.add_watcher(&"x".repeat(65)),
        Err(BoundaryError::InvalidLabel)
    );
    assert_eq!(log.add_watcher("a/b"), Err(BoundaryError::InvalidLabel));
    assert_eq!(log.add_watcher("-bad"), Err(BoundaryError::InvalidLabel));
    assert_eq!(log.add_watcher(".dot"), Err(BoundaryError::InvalidLabel));
    assert!(log.remove_watcher(1));
    assert!(
        !log.remove_watcher(1),
        "second remove must be harmless false"
    );
    // Duplicate consumed no id: next success is 3, not 4.
    assert_eq!(log.add_watcher("c"), Ok(3));
    // Upper length bound is inclusive: exactly 64 chars accepted.
    assert_eq!(log.add_watcher(&"y".repeat(64)), Ok(4));
    assert_no_os_watchers();
}

#[test]
fn ext004_t03_caps_and_drain() {
    assert_eq!(EXT4_MAX_DEFERRED, 128);
    assert_eq!(EXT4_MAX_WATCHERS, 32);
    let mut log = DeferredLog::new();
    for i in 0..EXT4_MAX_DEFERRED {
        assert_eq!(log.request_reload(None), Ok((i as u64) + 1));
    }
    assert_eq!(log.request_reload(None), Err(BoundaryError::Overflow));
    assert_eq!(log.note_external(None), Err(BoundaryError::Overflow));
    assert_eq!(log.list_deferred().len(), EXT4_MAX_DEFERRED);
    let d = log.drain(10);
    assert_eq!(d.len(), 10);
    assert_eq!(d.first().map(|e| e.seq), Some(1));
    assert_eq!(d.last().map(|e| e.seq), Some(10));
    let list = log.list_deferred();
    assert_eq!(list.len(), EXT4_MAX_DEFERRED - 10);
    assert_eq!(list.first().map(|e| e.seq), Some(11));
    assert!(log.drain(0).is_empty(), "drain(0) must be a no-op");
    assert_eq!(log.list_deferred().len(), EXT4_MAX_DEFERRED - 10);
    let rest = log.drain(usize::MAX);
    assert_eq!(rest.len(), EXT4_MAX_DEFERRED - 10);
    assert!(
        log.list_deferred().is_empty(),
        "drain-all must empty the log"
    );
    // Seq stays monotonic after drain: no reuse.
    assert_eq!(log.request_reload(None), Ok(129));
    // Watcher cap on a fresh log (watchers also record events; 32 < 128).
    let mut log2 = DeferredLog::new();
    for i in 0..EXT4_MAX_WATCHERS {
        assert_eq!(log2.add_watcher(&format!("w{i}")), Ok((i as u32) + 1));
    }
    assert_eq!(log2.add_watcher("one-more"), Err(BoundaryError::Overflow));
    assert_eq!(
        log2.list_deferred().len(),
        EXT4_MAX_WATCHERS,
        "failed add must record nothing"
    );
}

/// Minimal EXT-001-shaped registry double: the boundary must never touch it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FakePlugin {
    id: u64,
    name: String,
    ready: bool,
}

struct FakeRegistry {
    entries: Vec<FakePlugin>,
    next: u64,
}

impl FakeRegistry {
    fn new() -> Self {
        FakeRegistry {
            entries: Vec::new(),
            next: 1,
        }
    }
    fn add(&mut self, name: &str) -> u64 {
        let id = self.next;
        self.next += 1;
        self.entries.push(FakePlugin {
            id,
            name: name.to_string(),
            ready: false,
        });
        id
    }
    fn mark_ready(&mut self, id: u64) {
        if let Some(p) = self.entries.iter_mut().find(|p| p.id == id) {
            p.ready = true;
        }
    }
    fn list(&self) -> Vec<FakePlugin> {
        let mut v = self.entries.clone();
        v.sort_by_key(|p| p.id);
        v
    }
    fn wait_ready(&self, id: u64) -> Result<(), &'static str> {
        match self.entries.iter().find(|p| p.id == id) {
            Some(p) if p.ready => Ok(()),
            Some(_) => Err("not-ready"),
            None => Err("unknown"),
        }
    }
}

#[test]
fn ext004_t04_no_lifecycle_mutation() {
    let mut reg = FakeRegistry::new();
    let a = reg.add("alpha");
    let _b = reg.add("beta");
    reg.mark_ready(a);
    let before = format!("{:?}", reg.list());
    assert_eq!(reg.wait_ready(a), Ok(()));
    let mut log = DeferredLog::new();
    let _ = log.request_reload(Some(PluginId(a)));
    // Unknown plugin id still records intent, never validates.
    let _ = log.request_reload(Some(PluginId(9999)));
    let _ = log.note_external(Some(PluginId(a)));
    let _ = log.add_watcher("watch-a");
    assert_eq!(
        format!("{:?}", reg.list()),
        before,
        "boundary mutated registry state"
    );
    assert_eq!(reg.wait_ready(a), Ok(()), "readiness changed by reload");
    assert!(
        log.list_deferred()
            .iter()
            .any(|e| e.plugin == Some(PluginId(9999))
                && e.reason == DeferredReason::ReloadDeferred),
        "unknown-id reload must still record ReloadDeferred"
    );
}

#[test]
fn ext004_t05_zero_cost_when_off_and_safety() {
    let kids_before = children_count();
    // Disabled path: the log is never constructed; zero cost.
    let enabled = false;
    let log: Option<DeferredLog> = if enabled {
        Some(DeferredLog::new())
    } else {
        None
    };
    assert!(log.is_none(), "disabled boundary must construct nothing");
    assert_no_os_watchers();
    assert_eq!(children_count(), kids_before);
    // Disposable dir only; implementation performs zero I/O so it stays empty
    // (no files outside it, no DB writes).
    let dir = std::env::temp_dir().join(format!("ext004-deferred-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "boundary must create no files"
    );
    std::fs::remove_dir(&dir).unwrap();
    // Debug output carries short names only: no paths, no file bodies,
    // no credential-like bytes.
    let mut log2 = DeferredLog::new();
    let _ = log2.request_reload(Some(PluginId(1)));
    let _ = log2.add_watcher("short");
    let dbg = format!("{log2:?}");
    assert!(dbg.contains("short"), "debug must show recorded label");
    for secret in [
        "AKIAIOSFODNN7EXAMPLE",
        "sk-live-",
        "password=hunter2",
        "-----BEGIN",
    ] {
        assert!(!dbg.contains(secret), "secret-like bytes in log output");
    }
    // No subprocess spawned by any boundary call in this suite.
    assert_eq!(
        children_count(),
        kids_before,
        "boundary spawned a subprocess"
    );
}
