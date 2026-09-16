//! EXT-009 skill/command namespacing boundary, frozen EXT-009-T01..T05.
//!
//! RED: fails on missing behavior. Module included via `#[path]` so the lane
//! owns exactly `src/plugin_namespace.rs` + this file; the integrator wires
//! `pub mod plugin_namespace;` into `lib.rs` later.

#[path = "../src/plugin_namespace.rs"]
mod plugin_namespace;

use plugin_namespace::{EntryKind, MAX_NAMES, NameDecl, Namespace, NamespaceError};

fn decl(kind: EntryKind, name: &str, source: u32) -> NameDecl {
    NameDecl {
        kind,
        name: name.to_string(),
        source,
    }
}

fn skill(name: &str, source: u32) -> NameDecl {
    decl(EntryKind::Skill, name, source)
}

fn command(name: &str, source: u32) -> NameDecl {
    decl(EntryKind::Command, name, source)
}

fn total(ns: &Namespace) -> usize {
    ns.list(EntryKind::Skill).len() + ns.list(EntryKind::Command).len()
}

fn snapshot(ns: &Namespace) -> (Vec<(String, u32)>, Vec<(String, u32)>) {
    let skills = ns
        .list(EntryKind::Skill)
        .into_iter()
        .map(|d| (d.name.clone(), d.source))
        .collect();
    let commands = ns
        .list(EntryKind::Command)
        .into_iter()
        .map(|d| (d.name.clone(), d.source))
        .collect();
    (skills, commands)
}

#[test]
fn ext009_t01_happy_path() {
    let mut ns = Namespace::new();
    assert_eq!(ns.register(skill("lint", 1)), Ok(true));
    assert_eq!(ns.register(skill("test", 1)), Ok(true));
    assert_eq!(ns.register(command("lint", 1)), Ok(true));
    let skills: Vec<&str> = ns
        .list(EntryKind::Skill)
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    assert_eq!(skills, ["lint", "test"]);
    assert_eq!(ns.list(EntryKind::Command).len(), 1);
    assert_eq!(
        ns.lookup(EntryKind::Skill, "lint").map(|d| d.source),
        Some(1)
    );
    // Same string coexists across kinds.
    assert!(ns.lookup(EntryKind::Command, "lint").is_some());
}

#[test]
fn ext009_t02_later_source_precedence_and_unregister() {
    let mut ns = Namespace::new();
    assert_eq!(ns.register(skill("lint", 1)), Ok(true));
    assert_eq!(ns.register(command("lint", 1)), Ok(true));
    // Later source wins.
    assert_eq!(ns.register(skill("lint", 2)), Ok(false));
    assert_eq!(
        ns.lookup(EntryKind::Skill, "lint").map(|d| d.source),
        Some(2)
    );
    // Stale source keeps existing.
    assert_eq!(ns.register(skill("lint", 1)), Ok(false));
    assert_eq!(
        ns.lookup(EntryKind::Skill, "lint").map(|d| d.source),
        Some(2)
    );
    // Unregister removes exactly that entry.
    assert!(ns.unregister(EntryKind::Skill, "lint"));
    assert_eq!(ns.lookup(EntryKind::Skill, "lint"), None);
    assert!(ns.lookup(EntryKind::Command, "lint").is_some());
    // Unknown unregister is harmless.
    let before = snapshot(&ns);
    assert!(!ns.unregister(EntryKind::Skill, "ghost"));
    assert_eq!(snapshot(&ns), before);
}

#[test]
fn ext009_t03_validation_and_overflow_registry_unchanged() {
    let mut ns = Namespace::new();
    assert_eq!(ns.register(skill("keep", 1)), Ok(true));
    let long65 = "a".repeat(65);
    let bad: Vec<String> = vec![
        String::new(),
        "a/b".to_string(),
        " a".to_string(),
        ".lead".to_string(),
        "trail ".to_string(),
        long65,
    ];
    for name in &bad {
        let before = snapshot(&ns);
        assert_eq!(
            ns.register(skill(name, 1)).unwrap_err(),
            NamespaceError::InvalidName,
            "name {name:?} must be InvalidName"
        );
        assert_eq!(snapshot(&ns), before, "failed register must not mutate");
    }
    // 64-char boundary name is valid.
    let ok64 = "a".repeat(64);
    let mut ns2 = Namespace::new();
    assert_eq!(ns2.register(skill(&ok64, 1)), Ok(true));
    // Fill to cap across both kinds (256 names x 2 kinds).
    let mut full = Namespace::new();
    for i in 0..256u32 {
        let n = format!("n{i:03}");
        assert_eq!(full.register(skill(&n, 1)), Ok(true));
        assert_eq!(full.register(command(&n, 1)), Ok(true));
    }
    assert_eq!(total(&full), MAX_NAMES);
    let before = snapshot(&full);
    assert_eq!(
        full.register(skill("zz-new", 9)).unwrap_err(),
        NamespaceError::Overflow
    );
    assert_eq!(snapshot(&full), before, "overflow must not mutate");
    assert_eq!(total(&full), MAX_NAMES);
}

#[test]
fn ext009_t04_determinism() {
    let build = || {
        let mut ns = Namespace::new();
        ns.register(skill("b", 1)).unwrap();
        ns.register(command("a", 2)).unwrap();
        ns.register(skill("a", 1)).unwrap();
        ns.register(command("b", 1)).unwrap();
        ns.register(skill("c", 3)).unwrap();
        ns
    };
    let (a, b) = (build(), build());
    assert_eq!(snapshot(&a), snapshot(&b));
    // Two sources racing on one name resolve to the larger source
    // regardless of arrival order.
    let mut x = Namespace::new();
    x.register(skill("race", 1)).unwrap();
    x.register(skill("race", 2)).unwrap();
    let mut y = Namespace::new();
    y.register(skill("race", 2)).unwrap();
    y.register(skill("race", 1)).unwrap();
    assert_eq!(
        x.lookup(EntryKind::Skill, "race").map(|d| d.source),
        Some(2)
    );
    assert_eq!(
        y.lookup(EntryKind::Skill, "race").map(|d| d.source),
        Some(2)
    );
    assert_eq!(snapshot(&x), snapshot(&y));
}

#[test]
fn ext009_t05_no_discovery_or_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let before: Vec<std::ffi::OsString> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    // Full matrix: register both kinds, collide, stale write, unregister.
    let mut ns = Namespace::new();
    ns.register(skill("lint", 1)).unwrap();
    ns.register(skill("test", 1)).unwrap();
    ns.register(command("lint", 1)).unwrap();
    ns.register(skill("lint", 2)).unwrap();
    ns.register(skill("lint", 1)).unwrap();
    let _ = ns.lookup(EntryKind::Skill, "lint");
    let _ = ns.lookup(EntryKind::Command, "missing");
    let _ = ns.list(EntryKind::Skill);
    let _ = ns.list(EntryKind::Command);
    assert!(ns.unregister(EntryKind::Skill, "test"));
    assert!(!ns.unregister(EntryKind::Skill, "ghost"));
    // No files created outside the disposable dir (dir itself untouched:
    // the registry never touches the filesystem).
    let after: Vec<std::ffi::OsString> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
    // Entries hold kind/name/source only: every stored name is one of the
    // short declared names, and debug output carries zero credential-like
    // bytes (canary never inserted, must never appear).
    for kind in [EntryKind::Skill, EntryKind::Command] {
        for d in ns.list(kind) {
            assert!(["lint"].contains(&d.name.as_str()), "unexpected {d:?}");
        }
    }
    let dbg = format!("{ns:?}");
    assert!(dbg.contains("lint"));
    for secret in [
        "supersecret-canary-xyz",
        "AKIA",
        "password",
        "BEGIN PRIVATE KEY",
    ] {
        assert!(!dbg.contains(secret), "leaked {secret:?}");
    }
}
