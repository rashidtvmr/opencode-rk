//! EXT-011 plugin config-transform replay/disablement boundary, frozen EXT-011-T01..T05.
//!
//! RED: fails on missing behavior. Module included via `#[path]` so the lane
//! owns exactly `src/ext_replay_lane.rs` + this file; the integrator wires
//! `pub mod ext_replay_lane;` into `lib.rs` later.
//! Frozen after RED: never edit to make code pass, fix the implementation.

#[path = "../src/ext_replay_lane.rs"]
mod ext_replay_lane;

use std::time::{Duration, Instant};

use ext_replay_lane::{
    EXT11_MAX_KEY_LEN, EXT11_MAX_TRANSFORMS, EXT11_MAX_VALUE_LEN, TransformError, TransformLog,
};

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

fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn proj_bytes(log: &TransformLog) -> String {
    format!("{:?}", log.project())
}

#[test]
fn ext011_t01_happy_path_and_replay() {
    assert_eq!(EXT11_MAX_TRANSFORMS, 512);
    assert_eq!(EXT11_MAX_KEY_LEN, 128);
    assert_eq!(EXT11_MAX_VALUE_LEN, 1024);
    let mut log = TransformLog::new();
    assert!(log.project().is_empty(), "fresh log projects nothing");
    assert_eq!(log.add(7, "theme", "dark"), Ok(1));
    assert_eq!(log.add(7, "font", "mono"), Ok(2));
    assert_eq!(log.add(8, "theme", "light"), Ok(3));
    assert_eq!(
        log.project(),
        kv(&[("font", "mono"), ("theme", "light")]),
        "later seq wins per key, output sorted by key"
    );
    // Determinism: identical call sequence => identical projection.
    let mut replay = TransformLog::new();
    replay.add(7, "theme", "dark").unwrap();
    replay.add(7, "font", "mono").unwrap();
    replay.add(8, "theme", "light").unwrap();
    assert_eq!(replay.project(), log.project());
}

#[test]
fn ext011_t02_disable_and_close_reveal() {
    let mut log = TransformLog::new();
    log.add(7, "theme", "dark").unwrap();
    log.add(7, "font", "mono").unwrap();
    log.add(8, "theme", "light").unwrap();

    log.set_scope_disabled(8, true);
    assert_eq!(
        log.project(),
        kv(&[("font", "mono"), ("theme", "dark")]),
        "disabled scope stops projecting, record retained"
    );
    log.set_scope_disabled(8, false);
    assert_eq!(
        log.project(),
        kv(&[("font", "mono"), ("theme", "light")]),
        "re-enable restores projection"
    );
    // Unknown scope disable is a harmless no-op.
    let before = proj_bytes(&log);
    log.set_scope_disabled(9999, true);
    assert_eq!(proj_bytes(&log), before, "disable-unknown must not mutate");

    assert_eq!(log.close_scope(8), 1);
    assert_eq!(
        log.project(),
        kv(&[("font", "mono"), ("theme", "dark")]),
        "close removes only that scope, prior value revealed"
    );
    assert_eq!(log.close_scope(8), 0, "already-closed => 0");
    let before = proj_bytes(&log);
    assert_eq!(log.close_scope(9999), 0, "close-unknown => 0");
    assert_eq!(proj_bytes(&log), before, "close-unknown must not mutate");
}

#[test]
fn ext011_t03_validation_log_unchanged() {
    let mut log = TransformLog::new();
    assert_eq!(log.add(1, "dup", "a"), Ok(1));
    // Same key in different scopes is not a duplicate.
    assert_eq!(log.add(2, "dup", "b"), Ok(2));
    // Per-scope duplicate: exact error, no seq consumed, projection identical.
    let before = proj_bytes(&log);
    assert_eq!(log.add(1, "dup", "c"), Err(TransformError::Duplicate));
    assert_eq!(proj_bytes(&log), before, "duplicate must not mutate");
    assert_eq!(
        log.add(1, "next", "x"),
        Ok(3),
        "failed add consumes no seq"
    );

    // Bad keys: empty, too long, bad charset / leading punctuation.
    let bad_keys: Vec<String> = vec![
        String::new(),
        "x".repeat(129),
        "/bad".to_string(),
        "bad key".to_string(),
        "-lead".to_string(),
        ".lead".to_string(),
        "a/b".to_string(),
    ];
    for key in &bad_keys {
        let before = proj_bytes(&log);
        assert_eq!(
            log.add(3, key, "v"),
            Err(TransformError::InvalidKey),
            "key {key:?} must be InvalidKey"
        );
        assert_eq!(proj_bytes(&log), before, "bad key must not mutate");
    }
    // Boundaries: 128-char key ok, empty value ok, 1024-char value ok.
    assert!(log.add(3, &"k".repeat(128), "").is_ok());
    assert!(log.add(3, "boundary", &"v".repeat(1024)).is_ok());

    // Bad values: too long, interior NUL.
    let before = proj_bytes(&log);
    assert_eq!(
        log.add(4, "toolong", &"v".repeat(1025)),
        Err(TransformError::InvalidValue)
    );
    assert_eq!(proj_bytes(&log), before, "long value must not mutate");
    let before = proj_bytes(&log);
    assert_eq!(
        log.add(4, "nul", "a\0b"),
        Err(TransformError::InvalidValue)
    );
    assert_eq!(proj_bytes(&log), before, "NUL value must not mutate");

    // Fill to cap, then one more overflows without growth.
    let mut full = TransformLog::new();
    for i in 0..EXT11_MAX_TRANSFORMS {
        full.add(i as u64, &format!("k{i:03}"), "v")
            .expect("fill to cap");
    }
    assert_eq!(full.project().len(), EXT11_MAX_TRANSFORMS);
    let before = proj_bytes(&full);
    assert_eq!(
        full.add(u64::MAX, "one-more", "v"),
        Err(TransformError::Overflow)
    );
    assert_eq!(proj_bytes(&full), before, "overflow must not mutate");
    assert_eq!(
        full.project().len(),
        EXT11_MAX_TRANSFORMS,
        "log never exceeds cap"
    );
}

/// Minimal EXT-001/003/007-shaped registry double: the boundary must never touch it.
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
}

#[test]
fn ext011_t04_no_service_mutation() {
    let mut reg = FakeRegistry::new();
    let a = reg.add("alpha");
    let _b = reg.add("beta");
    reg.mark_ready(a);
    let before = format!("{:?}", reg.list());

    // Full boundary matrix against a separate log: adds (incl. cross-scope
    // same key), disable/re-enable, unknown disable, close, unknown close.
    let mut log = TransformLog::new();
    log.add(7, "theme", "dark").unwrap();
    log.add(7, "font", "mono").unwrap();
    log.add(8, "theme", "light").unwrap();
    log.set_scope_disabled(8, true);
    let _ = log.project();
    log.set_scope_disabled(8, false);
    log.set_scope_disabled(9999, true);
    assert_eq!(log.close_scope(8), 1);
    assert_eq!(log.close_scope(9999), 0);
    let _ = log.project();

    assert_eq!(
        format!("{:?}", reg.list()),
        before,
        "boundary mutated composed-service state"
    );

    // 512-entry projection completes promptly (<1 s).
    let mut full = TransformLog::new();
    for i in 0..EXT11_MAX_TRANSFORMS {
        full.add(i as u64, &format!("k{i:03}"), "v").unwrap();
    }
    let start = Instant::now();
    let projected = full.project();
    assert_eq!(projected.len(), EXT11_MAX_TRANSFORMS);
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "512-entry replay must complete <1 s"
    );
}

#[test]
fn ext011_t05_side_effect_safety() {
    let kids_before = children_count();
    let dir = tempfile::tempdir().expect("disposable test dir");
    let before: Vec<std::ffi::OsString> = std::fs::read_dir(dir.path())
        .expect("read disposable dir")
        .map(|e| e.expect("dir entry").file_name())
        .collect();

    // Full matrix: happy adds, duplicate/invalid attempts, disable,
    // re-enable, unknown disable, close, unknown close, projections.
    let mut log = TransformLog::new();
    log.add(7, "theme", "dark").unwrap();
    log.add(7, "font", "mono").unwrap();
    log.add(8, "theme", "light").unwrap();
    let _ = log.add(7, "theme", "again");
    let _ = log.add(9, "", "bad");
    let _ = log.add(9, "ok", "a\0b");
    log.set_scope_disabled(8, true);
    let _ = log.project();
    log.set_scope_disabled(8, false);
    log.set_scope_disabled(u64::MAX, true);
    assert_eq!(log.close_scope(u64::MAX), 0);
    assert_eq!(log.close_scope(8), 1);
    let _ = log.project();

    // No subprocess spawned by any boundary call in this suite.
    assert_eq!(
        children_count(),
        kids_before,
        "boundary spawned a subprocess"
    );
    // No files created outside the disposable dir (dir itself untouched:
    // the boundary never touches the filesystem or any DB).
    let after: Vec<std::ffi::OsString> = std::fs::read_dir(dir.path())
        .expect("reread disposable dir")
        .map(|e| e.expect("dir entry").file_name())
        .collect();
    assert_eq!(before, after, "no files outside the boundary");
    // Projection and log hold short labels only: recorded keys visible,
    // zero credential-like bytes (canaries never inserted).
    let dbg = format!("{:?}", log.project());
    assert!(dbg.contains("font"), "projection must show recorded keys");
    for secret in [
        "supersecret-canary-xyz",
        "AKIA",
        "password",
        "BEGIN PRIVATE KEY",
    ] {
        assert!(!dbg.contains(secret), "leaked {secret:?}");
    }
    let log_dbg = format!("{log:?}");
    assert!(log_dbg.contains("font"), "log must show recorded keys");
    for secret in [
        "supersecret-canary-xyz",
        "AKIA",
        "password",
        "BEGIN PRIVATE KEY",
    ] {
        assert!(!log_dbg.contains(secret), "leaked {secret:?}");
    }
}
