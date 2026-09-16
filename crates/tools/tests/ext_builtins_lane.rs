//! EXT-002 built-in plugin wiring, frozen EXT-002-T01..T05.
//!
//! RED: fails on missing behavior. Module included via `#[path]` so the lane
//! owns exactly `src/ext_builtins_lane.rs` + this file; the integrator wires
//! `pub mod ext_builtins_lane;` into `lib.rs` later.

#[path = "../src/ext_builtins_lane.rs"]
mod ext_builtins_lane;

use std::sync::atomic::AtomicBool;

use ext_builtins_lane::{
    BUILTINS, MAX_PLUGINS, PluginDecl, PluginError, PluginId, PluginRegistry, PluginState,
    SUPPORTED_CONTRACT_VERSION, is_builtin, register_builtins, unregister_builtins,
};

fn dynamic(name: &str) -> PluginDecl {
    PluginDecl {
        name: name.to_string(),
        contract_version: SUPPORTED_CONTRACT_VERSION,
        capabilities: Vec::new(),
    }
}

#[test]
fn ext002_t01_happy_path() {
    assert_eq!(BUILTINS.len(), 2);
    assert_eq!(BUILTINS[0], ("core.commands", &[] as &[&str]));
    assert_eq!(BUILTINS[1], ("core.skills", &["skill.list"] as &[&str]));
    let mut reg = PluginRegistry::new();
    let ids = register_builtins(&mut reg, SUPPORTED_CONTRACT_VERSION).expect("register builtins");
    assert_eq!(ids.len(), 2);
    let list = reg.list();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, ids[0]);
    assert_eq!(list[1].id, ids[1]);
    assert_eq!(list[0].decl.name, "core.commands");
    assert_eq!(list[1].decl.name, "core.skills");
    assert!(list.iter().all(|i| i.state == PluginState::Ready));
    assert!(list[0].decl.capabilities.is_empty());
    assert_eq!(list[1].decl.capabilities, vec!["skill.list".to_string()]);
    assert!(is_builtin("core.commands"));
    assert!(is_builtin("core.skills"));
    assert!(!is_builtin("evil"));
    assert!(!is_builtin("core.commands "));
    assert!(!is_builtin(""));
}

#[test]
fn ext002_t02_unregister_clean() {
    let mut reg = PluginRegistry::new();
    let ids = register_builtins(&mut reg, SUPPORTED_CONTRACT_VERSION).unwrap();
    assert_eq!(unregister_builtins(&mut reg, &ids), 2);
    assert!(reg.list().is_empty());
    // Unknown ids only: harmless, count 0, registry unchanged.
    let before = reg.list();
    assert_eq!(
        unregister_builtins(&mut reg, &[PluginId(9999), PluginId(10000)]),
        0
    );
    assert_eq!(reg.list().len(), before.len());
    // Empty slice removes nothing.
    assert_eq!(unregister_builtins(&mut reg, &[]), 0);
    // register -> unregister -> register yields fresh larger ids in table order.
    let ids2 = register_builtins(&mut reg, SUPPORTED_CONTRACT_VERSION).unwrap();
    assert_eq!(ids2.len(), 2);
    assert!(
        ids2[0].0 > ids[1].0,
        "ids must be fresh, got {ids2:?} after {ids:?}"
    );
    let list = reg.list();
    assert_eq!(list[0].id, ids2[0]);
    assert_eq!(list[1].id, ids2[1]);
    assert_eq!(list[0].decl.name, "core.commands");
    assert_eq!(list[1].decl.name, "core.skills");
    assert!(list.iter().all(|i| i.state == PluginState::Ready));
}

#[test]
fn ext002_t03_atomicity() {
    // Collision with a pre-existing dynamic same-named plugin: no partial add.
    let mut reg = PluginRegistry::new();
    let dyn_id = reg.add(dynamic("core.skills")).unwrap();
    assert_eq!(
        register_builtins(&mut reg, SUPPORTED_CONTRACT_VERSION),
        Err(PluginError::Duplicate)
    );
    let after = reg.list();
    assert_eq!(after.len(), 1, "partial builtin must be rolled back");
    assert_eq!(after[0].id, dyn_id);
    assert_eq!(after[0].decl.name, "core.skills");
    // Double registration without unregister: Duplicate, registry unchanged.
    let mut reg2 = PluginRegistry::new();
    register_builtins(&mut reg2, SUPPORTED_CONTRACT_VERSION).unwrap();
    assert_eq!(
        register_builtins(&mut reg2, SUPPORTED_CONTRACT_VERSION),
        Err(PluginError::Duplicate)
    );
    assert_eq!(reg2.list().len(), 2);
    // Wrong contract version: UnsupportedContract, registry unchanged.
    let mut reg3 = PluginRegistry::new();
    assert_eq!(
        register_builtins(&mut reg3, 999),
        Err(PluginError::UnsupportedContract)
    );
    assert!(reg3.list().is_empty());
    // Registry full: Overflow, registry unchanged.
    let mut full = PluginRegistry::new();
    for i in 0..MAX_PLUGINS {
        full.add(dynamic(&format!("dyn-{i:03}"))).unwrap();
    }
    assert_eq!(full.list().len(), MAX_PLUGINS);
    assert_eq!(
        register_builtins(&mut full, SUPPORTED_CONTRACT_VERSION),
        Err(PluginError::Overflow)
    );
    assert_eq!(full.list().len(), MAX_PLUGINS);
}

#[test]
fn ext002_t04_no_lifecycle_redefinition() {
    let mut reg = PluginRegistry::new();
    let ids = register_builtins(&mut reg, SUPPORTED_CONTRACT_VERSION).unwrap();
    // EXT-001 wait contract works directly on a builtin id.
    let unset = AtomicBool::new(false);
    assert_eq!(reg.wait_ready(ids[0], &unset), Ok(()));
    // Removing one builtin via the registry leaves the other Ready and visible.
    assert!(reg.remove(ids[0]));
    let list = reg.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, ids[1]);
    assert_eq!(list[0].state, PluginState::Ready);
    // Re-registering after partial removal hits the still-present name
    // with no partial add.
    assert_eq!(
        register_builtins(&mut reg, SUPPORTED_CONTRACT_VERSION),
        Err(PluginError::Duplicate)
    );
    let after = reg.list();
    assert_eq!(after.len(), 1, "failed re-register must not partially add");
    assert_eq!(after[0].id, ids[1]);
    assert_eq!(after[0].state, PluginState::Ready);
}

fn task_count() -> Option<usize> {
    std::fs::read_dir("/proc/self/task").ok().map(|d| d.count())
}

#[test]
fn ext002_t05_zero_cost_when_off_and_safety() {
    let dir = tempfile::tempdir().unwrap();
    let before: Vec<std::ffi::OsString> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let t0 = task_count();
    // Feature off: register_builtins never called; registry stays empty.
    let reg = PluginRegistry::new();
    assert!(reg.list().is_empty());
    let dbg = format!("{reg:?}");
    // Zero builtin strings and zero credential-like bytes in any output.
    for secret in [
        "core.commands",
        "core.skills",
        "skill.list",
        "supersecret-canary-xyz",
        "AKIA",
        "password",
        "BEGIN PRIVATE KEY",
    ] {
        assert!(!dbg.contains(secret), "disabled registry leaks {secret:?}");
    }
    let t1 = task_count();
    if let (Some(a), Some(b)) = (t0, t1) {
        assert_eq!(a, b, "no threads may be spawned when builtins are off");
    }
    let after: Vec<std::ffi::OsString> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        before, after,
        "no files may be created outside the test dir"
    );
}
