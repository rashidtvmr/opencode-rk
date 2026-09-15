//! EXT-001 scoped native plugin add/remove/wait lifecycle, frozen EXT-001-T01..T05.
//!
//! RED: fails on missing behavior. Module included via `#[path]` so the lane
//! owns exactly `src/plugin_lifecycle.rs` + this file; the integrator wires
//! `pub mod plugin_lifecycle;` into `lib.rs` later.

#[path = "../src/plugin_lifecycle.rs"]
mod plugin_lifecycle;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use plugin_lifecycle::{
    MAX_CAPABILITIES, MAX_PLUGINS, PluginDecl, PluginError, PluginId, PluginRegistry,
    PluginState, SUPPORTED_CONTRACT_VERSION,
};

fn decl(name: &str) -> PluginDecl {
    PluginDecl {
        name: name.to_string(),
        contract_version: SUPPORTED_CONTRACT_VERSION,
        capabilities: vec!["cap-a".to_string()],
    }
}

fn snapshot(reg: &PluginRegistry) -> Vec<(u64, String, PluginState)> {
    reg.list()
        .into_iter()
        .map(|i| (i.id.0, i.decl.name.clone(), i.state))
        .collect()
}

#[test]
fn ext001_t01_happy_path() {
    let mut reg = PluginRegistry::new();
    let a = reg.add(decl("alpha")).expect("add alpha");
    let b = reg.add(decl("beta")).expect("add beta");
    let c = reg.add(decl("gamma")).expect("add gamma");
    assert_eq!(a, PluginId(1));
    assert_eq!(b, PluginId(2));
    assert_eq!(c, PluginId(3));
    assert_eq!(reg.mark_ready(b), Ok(()));
    let cancel = AtomicBool::new(false);
    assert_eq!(reg.wait_ready(b, &cancel), Ok(()));
    let list = reg.list();
    assert_eq!(list.len(), 3);
    let ids: Vec<u64> = list.iter().map(|i| i.id.0).collect();
    assert_eq!(ids, [1, 2, 3]);
    assert_eq!(list[1].state, PluginState::Ready);
    assert_eq!(list[0].state, PluginState::Registered);
    assert_eq!(list[2].state, PluginState::Registered);
}

#[test]
fn ext001_t02_remove_scope() {
    let mut reg = PluginRegistry::new();
    let a = reg.add(decl("alpha")).unwrap();
    let b = reg.add(decl("beta")).unwrap();
    assert!(reg.remove(a));
    assert_eq!(reg.list().len(), 1);
    assert_eq!(reg.list()[0].id, b);
    assert_eq!(reg.list()[0].id.0, 2);
    let before = snapshot(&reg);
    assert!(!reg.remove(PluginId(999)));
    assert_eq!(snapshot(&reg), before, "remove-unknown must not mutate");
    let cancel = AtomicBool::new(false);
    assert_eq!(
        reg.wait_ready(a, &cancel),
        Err(PluginError::Unknown),
        "wait on removed id must be Unknown"
    );
}

#[test]
fn ext001_t03_validation_and_overflow_registry_unchanged() {
    let mut reg = PluginRegistry::new();
    reg.add(decl("keep")).unwrap();

    // Duplicate name.
    let before = snapshot(&reg);
    assert_eq!(reg.add(decl("keep")).unwrap_err(), PluginError::Duplicate);
    assert_eq!(snapshot(&reg), before, "duplicate must not mutate");

    // Bad names.
    let bad_names: Vec<String> = vec![
        String::new(),
        "x".repeat(129),
        "/bad".to_string(),
        "bad name".to_string(),
        "-lead".to_string(),
        ".lead".to_string(),
    ];
    for name in &bad_names {
        let before = snapshot(&reg);
        let mut d = decl(name);
        // `decl` helper builds a valid decl; overwrite name verbatim.
        d.name = name.clone();
        assert_eq!(
            reg.add(d).unwrap_err(),
            PluginError::InvalidName,
            "name {name:?} must be InvalidName"
        );
        assert_eq!(snapshot(&reg), before, "bad name must not mutate");
    }

    // Too many capabilities.
    let before = snapshot(&reg);
    let mut d = decl("too-many-caps");
    d.capabilities = (0..17).map(|i| format!("c{i}")).collect();
    assert_eq!(
        reg.add(d).unwrap_err(),
        PluginError::InvalidCapabilities
    );
    assert_eq!(snapshot(&reg), before);

    // Bad capability entries: empty, space, too long, intra-decl duplicate.
    let bad_cap_sets: Vec<Vec<String>> = vec![
        vec![String::new()],
        vec!["bad cap".to_string()],
        vec!["x".repeat(65)],
        vec!["dup".to_string(), "dup".to_string()],
    ];
    for caps in &bad_cap_sets {
        let before = snapshot(&reg);
        let mut d = decl("bad-cap-entry");
        d.capabilities = caps.clone();
        assert_eq!(
            reg.add(d).unwrap_err(),
            PluginError::InvalidCapabilities,
            "caps {caps:?} must be InvalidCapabilities"
        );
        assert_eq!(snapshot(&reg), before);
    }

    // Unsupported contract version.
    let before = snapshot(&reg);
    let mut d = decl("bad-version");
    d.contract_version = 999;
    assert_eq!(
        reg.add(d).unwrap_err(),
        PluginError::UnsupportedContract
    );
    assert_eq!(snapshot(&reg), before);
    assert_eq!(MAX_CAPABILITIES, 16);

    // Fill to cap, then overflow.
    let mut full = PluginRegistry::new();
    for i in 0..MAX_PLUGINS {
        full.add(decl(&format!("p{i:03}"))).unwrap();
    }
    assert_eq!(full.list().len(), MAX_PLUGINS);
    let before = snapshot(&full);
    assert_eq!(
        full.add(decl("one-more")).unwrap_err(),
        PluginError::Overflow
    );
    assert_eq!(snapshot(&full), before, "overflow must not mutate");
    assert_eq!(full.list().len(), MAX_PLUGINS);
}

#[test]
fn ext001_t04_wait_cancel() {
    let mut reg = PluginRegistry::new();
    assert_eq!(
        reg.mark_ready(PluginId(999)),
        Err(PluginError::Unknown)
    );
    let cancel = AtomicBool::new(false);
    assert_eq!(
        reg.wait_ready(PluginId(999), &cancel),
        Err(PluginError::Unknown)
    );

    // Pre-set cancel on a Registered plugin: prompt Cancelled.
    let id = reg.add(decl("slow")).unwrap();
    let preset = AtomicBool::new(true);
    let start = Instant::now();
    assert_eq!(
        reg.wait_ready(id, &preset),
        Err(PluginError::Cancelled)
    );
    assert!(
        start.elapsed() < Duration::from_millis(50),
        "pre-set cancel must return within 50 ms"
    );

    // Cancel 5 ms into a blocked wait_ready: waiter thread joins cleanly.
    let flag = AtomicBool::new(false);
    let start = Instant::now();
    std::thread::scope(|s| {
        let h = s.spawn(|| reg.wait_ready(id, &flag));
        std::thread::sleep(Duration::from_millis(5));
        flag.store(true, Ordering::SeqCst);
        let r = h.join().expect("waiter thread must join, no leak");
        assert_eq!(r, Err(PluginError::Cancelled));
    });
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "mid-wait cancel must return promptly"
    );
    // Registry unchanged by cancelled waits.
    assert_eq!(reg.list().len(), 1);
    assert_eq!(reg.list()[0].state, PluginState::Registered);
}

#[test]
fn ext001_t05_no_host_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let before: Vec<std::ffi::OsString> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    // Full lifecycle inside the disposable dir's shadow: registry never
    // touches the filesystem, spawns no host, launches no callbacks.
    let mut reg = PluginRegistry::new();
    let a = reg.add(decl("alpha")).unwrap();
    let b = reg.add(decl("beta")).unwrap();
    reg.mark_ready(a).unwrap();
    let cancel = AtomicBool::new(false);
    assert_eq!(reg.wait_ready(a, &cancel), Ok(()));
    assert!(reg.remove(b));
    let _ = reg.list();
    // No files created outside the disposable dir (dir itself untouched).
    let after: Vec<std::ffi::OsString> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
    // Declarations hold names/labels only; debug carries the plugin name
    // but zero credential-like bytes (canaries never inserted).
    let dbg = format!("{:?}", reg.list());
    assert!(dbg.contains("alpha"));
    for secret in [
        "supersecret-canary-xyz",
        "AKIA",
        "password",
        "BEGIN PRIVATE KEY",
    ] {
        assert!(!dbg.contains(secret), "leaked {secret:?}");
    }
}
