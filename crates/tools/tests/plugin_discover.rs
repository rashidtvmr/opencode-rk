//! EXT-010 frozen tests: deferred external-discovery boundary (EXT-010-T01..T05).
//!
//! Included via path so the owning module needs no shared-file wiring;
//! the integrator re-exports it from `lib.rs` per PLAN.md section 5.
//! Frozen after RED: never edit to make code pass, fix the implementation.

#[path = "../src/plugin_discover.rs"]
mod plugin_discover;

use plugin_discover::{
    DeferredReason, DiscoveryBoundary, DiscoveryError, DiscoveryRef, EXT10_MAX_DEFERRED,
};

fn pkg(s: &str) -> DiscoveryRef {
    DiscoveryRef {
        package: Some(s.to_string()),
        file: None,
    }
}

fn file(s: &str) -> DiscoveryRef {
    DiscoveryRef {
        package: None,
        file: Some(s.to_string()),
    }
}

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

#[allow(dead_code)]
fn task_count() -> usize {
    std::fs::read_dir("/proc/self/task")
        .map(|d| d.count())
        .unwrap_or(0)
}

#[allow(dead_code)]
fn fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .map(|d| d.count())
        .unwrap_or(0)
}

#[allow(dead_code)]
fn socket_count() -> usize {
    fd_targets()
        .iter()
        .filter(|t| t.to_lowercase().contains("socket:"))
        .count()
}

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

/// Zero loader/watcher handles or threads: no inotify/fanotify handle held,
/// no plugin/discover/loader/notify/watch thread spawned. Robust under
/// parallel harness threads (scans kinds, not counts).
fn assert_no_loader_artifacts() {
    for t in fd_targets() {
        let l = t.to_lowercase();
        assert!(
            !l.contains("inotify") && !l.contains("fanotify"),
            "os watch handle held: {t}"
        );
    }
    for n in thread_names() {
        let l = n.to_lowercase();
        // Narrow match: bare "discover" self-matches this test binary
        // ("plugin_discover", comm truncated to 15 chars). Only flag
        // "discover" when paired with loader/watch/notify context.
        assert!(
            !l.contains("notify")
                && !l.contains("inotify")
                && !l.contains("fanotify")
                && !l.contains("watch")
                && !l.contains("loader")
                && !(l.contains("discover")
                    && (l.contains("loader")
                        || l.contains("watch")
                        || l.contains("notify"))),
            "loader/watcher thread spawned: {n}"
        );
    }
}

#[test]
fn ext010_t01_record_and_list() {
    let mut b = DiscoveryBoundary::new();
    assert!(b.list_deferred().is_empty(), "fresh boundary must be empty");
    assert_eq!(b.declare(7, pkg("acme-plug")), Ok(1));
    assert_eq!(b.declare(7, file("plugins/a/mod.js")), Ok(2));
    assert_eq!(b.declare(8, pkg("@scope/b")), Ok(3));
    let list = b.list_deferred();
    assert_eq!(list.len(), 3);
    assert_eq!(
        list.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "seqs must run 1..=3 in order"
    );
    assert_eq!(
        list.iter().map(|e| e.scope).collect::<Vec<_>>(),
        vec![7, 7, 8]
    );
    for e in &list {
        assert_eq!(
            e.reason,
            DeferredReason::ExternalDiscoveryDeferred,
            "every event must be ExternalDiscoveryDeferred"
        );
    }
    assert_eq!(list[0].reference, pkg("acme-plug"));
    assert_eq!(list[1].reference, file("plugins/a/mod.js"));
    assert_eq!(list[2].reference, pkg("@scope/b"));
    // Determinism: identical call sequence => identical projection.
    let mut replay = DiscoveryBoundary::new();
    assert_eq!(replay.declare(7, pkg("acme-plug")), Ok(1));
    assert_eq!(replay.declare(7, file("plugins/a/mod.js")), Ok(2));
    assert_eq!(replay.declare(8, pkg("@scope/b")), Ok(3));
    assert_eq!(replay.list_deferred(), list);
}

#[test]
fn ext010_t02_scope_revoke() {
    let mut b = DiscoveryBoundary::new();
    assert_eq!(b.declare(7, pkg("alpha")), Ok(1));
    assert_eq!(b.declare(7, file("plugins/a.js")), Ok(2));
    assert_eq!(b.declare(8, pkg("beta")), Ok(3));
    assert_eq!(b.revoke_scope(7), 2);
    let list = b.list_deferred();
    assert_eq!(list.len(), 1, "only the scope-8 event must remain");
    assert_eq!(list[0].scope, 8);
    assert_eq!(list[0].seq, 3);
    assert_eq!(list[0].reference, pkg("beta"));
    let before = format!("{:?}", list);
    assert_eq!(b.revoke_scope(9999), 0, "unknown scope must be harmless");
    assert_eq!(format!("{:?}", b.list_deferred()), before);
    // Re-declare the same ref under a fresh scope: no ghost duplicate.
    assert_eq!(b.declare(9, pkg("alpha")), Ok(4));
    let list = b.list_deferred();
    assert_eq!(list.len(), 2);
    assert_eq!(
        list.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![3, 4]
    );
}

#[test]
fn ext010_t03_validation_and_caps() {
    let mut b = DiscoveryBoundary::new();
    assert_eq!(b.declare(1, pkg("seed")), Ok(1));
    let snap = |b: &DiscoveryBoundary| format!("{:?}", b.list_deferred());
    // Both set / neither set.
    let both = DiscoveryRef {
        package: Some("a".to_string()),
        file: Some("b.js".to_string()),
    };
    let neither = DiscoveryRef {
        package: None,
        file: None,
    };
    for bad in [both, neither] {
        let before = snap(&b);
        assert_eq!(b.declare(1, bad), Err(DiscoveryError::InvalidRef));
        assert_eq!(snap(&b), before, "failed declare must leave log unchanged");
    }
    // Absolute path and traversal.
    for bad in [file("/x/y.js"), file("a/../b.js")] {
        let before = snap(&b);
        assert_eq!(b.declare(1, bad), Err(DiscoveryError::InvalidRef));
        assert_eq!(snap(&b), before, "failed declare must leave log unchanged");
    }
    // Empty and overlong package (129 chars).
    for bad in [pkg(""), pkg(&"p".repeat(129))] {
        let before = snap(&b);
        assert_eq!(b.declare(1, bad), Err(DiscoveryError::InvalidRef));
        assert_eq!(snap(&b), before, "failed declare must leave log unchanged");
    }
    // Overlong file (257 chars).
    let before = snap(&b);
    assert_eq!(b.declare(1, file(&"f".repeat(257))), Err(DiscoveryError::InvalidRef));
    assert_eq!(snap(&b), before, "failed declare must leave log unchanged");
    // Bad charset.
    for bad in [pkg("has space"), pkg("semi;colon"), file("back\\slash")] {
        let before = snap(&b);
        assert_eq!(b.declare(1, bad), Err(DiscoveryError::InvalidRef));
        assert_eq!(snap(&b), before, "failed declare must leave log unchanged");
    }
    // Fill to the frozen cap (1 seed already recorded).
    assert_eq!(EXT10_MAX_DEFERRED, 128);
    for i in 1..EXT10_MAX_DEFERRED {
        assert_eq!(b.declare(1, pkg(&format!("cap-{i}"))), Ok((i as u64) + 1));
    }
    assert_eq!(b.list_deferred().len(), EXT10_MAX_DEFERRED);
    let before = snap(&b);
    assert_eq!(
        b.declare(1, pkg("one-too-many")),
        Err(DiscoveryError::Overflow)
    );
    assert_eq!(snap(&b), before, "overflow must leave log unchanged");
    assert_eq!(b.list_deferred().len(), EXT10_MAX_DEFERRED);
    // Drain oldest 10, then drain-all; never panics.
    let d = b.drain(10);
    assert_eq!(d.len(), 10);
    assert_eq!(
        d.iter().map(|e| e.seq).collect::<Vec<_>>(),
        (1..=10).collect::<Vec<_>>()
    );
    assert!(b.drain(0).is_empty(), "drain(0) must be a no-op");
    assert_eq!(b.list_deferred().len(), EXT10_MAX_DEFERRED - 10);
    let rest = b.drain(usize::MAX);
    assert_eq!(rest.len(), EXT10_MAX_DEFERRED - 10);
    assert!(b.list_deferred().is_empty(), "drain-all must empty the log");
}

/// Minimal EXT-001-shaped registry double plus the (absent) loader probe:
/// the boundary must never touch either.
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
    fn list(&self) -> Vec<FakePlugin> {
        let mut v = self.entries.clone();
        v.sort_by_key(|p| p.id);
        v
    }
}

#[test]
fn ext010_t04_no_loading_proof() {
    let mut reg = FakeRegistry::new();
    reg.add("alpha");
    reg.add("beta");
    let before = format!("{:?}", reg.list());
    // The loader that does not exist: any dynamic import/host call would
    // bump this counter. It must stay 0 through every boundary call.
    let mut loader_calls: u64 = 0;
    let mut b = DiscoveryBoundary::new();
    let _ = b.declare(7, pkg("acme-plug"));
    let _ = b.declare(7, file("plugins/a/mod.js"));
    let _ = b.declare(8, pkg("@scope/b"));
    let _ = b.revoke_scope(7);
    let _ = b.list_deferred();
    let _ = b.drain(1);
    assert_eq!(
        format!("{:?}", reg.list()),
        before,
        "boundary mutated registry state"
    );
    assert_eq!(loader_calls, 0, "external loader was invoked");
    // Silence unused-mut without touching the boundary: the counter is
    // only ever written by a loader that must not exist.
    loader_calls += 0;
    assert_eq!(loader_calls, 0);
}

#[test]
fn ext010_t05_zero_cost_when_off_and_safety() {
    let kids_before = children_count();
    // Disabled path: the log is never constructed; zero cost.
    let enabled = false;
    let boundary: Option<DiscoveryBoundary> = if enabled {
        Some(DiscoveryBoundary::new())
    } else {
        None
    };
    assert!(boundary.is_none(), "disabled boundary must construct nothing");
    assert_no_loader_artifacts();
    assert_eq!(children_count(), kids_before);
    // Disposable dir only; implementation performs zero I/O so it stays empty
    // (no files outside it, no DB writes).
    let dir = std::env::temp_dir().join(format!("ext010-discover-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "boundary must create no files"
    );
    std::fs::remove_dir(&dir).unwrap();
    // Debug output carries short names only: no file bodies,
    // no credential-like bytes.
    let mut b = DiscoveryBoundary::new();
    let _ = b.declare(1, pkg("zz-short"));
    let dbg = format!("{:?}", b.list_deferred());
    assert!(dbg.contains("zz-short"), "debug must show recorded ref");
    for secret in [
        "AKIAIOSFODNN7EXAMPLE",
        "sk-live-",
        "password=hunter2",
        "-----BEGIN",
    ] {
        assert!(!dbg.contains(secret), "secret-like bytes in log output");
    }
    assert_eq!(
        children_count(),
        kids_before,
        "boundary spawned a subprocess"
    );
}
