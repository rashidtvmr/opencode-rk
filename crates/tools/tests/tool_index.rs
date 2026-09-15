//! TOOL-016 native codebase index/snapshot, frozen TOOL-016-T01..T05.
//!
//! RED: fails on missing behavior. Module included via `#[path]` so the lane
//! owns exactly `src/tool_index.rs` + this file; the integrator wires
//! `pub mod tool_index;` into `lib.rs` later.

#[path = "../src/tool_index.rs"]
mod tool_index;

use std::fs;
use std::sync::atomic::AtomicBool;

use tool_index::{
    IndexConfig, IndexError, build_snapshot, build_snapshot_cancel, hash_bytes, lookup_file,
    lookup_symbol, total_bytes,
};

fn write_tree(root: &std::path::Path, n: usize) {
    for i in 0..n {
        fs::write(root.join(format!("file{i:03}.txt")), format!("body-{i}\n")).unwrap();
    }
}

fn symbol_tree(root: &std::path::Path) {
    write_tree(root, 7);
    fs::write(
        root.join("a.rs"),
        "pub struct MyStruct { pub x: u32 }\n\
         pub fn alpha() {}\n\
         pub fn beta() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("b.rs"),
        "pub enum MyEnum { A, B }\npub trait MyTrait {}\n",
    )
    .unwrap();
    fs::write(
        root.join("c.rs"),
        "pub mod helpers {}\npub const LIMIT: u32 = 7;\n",
    )
    .unwrap();
}

#[test]
fn tool016_t01_full_scan_happy_path() {
    let dir = tempfile::tempdir().unwrap();
    symbol_tree(dir.path());
    let snap = build_snapshot(dir.path(), &IndexConfig::default()).unwrap();
    assert_eq!(snap.files.len(), 10);
    assert!(snap.symbols.len() >= 5);
    assert!(!snap.truncated);
    assert_eq!(lookup_symbol(&snap, "MyStruct").len(), 1);
    let entry = lookup_file(&snap, "a.rs").expect("a.rs indexed");
    let raw = fs::read(dir.path().join("a.rs")).unwrap();
    assert_eq!(entry.hash, hash_bytes(&raw));
    assert_eq!(entry.size, raw.len() as u64);
}

#[test]
fn tool016_t02_determinism_and_lookup_order() {
    let dir = tempfile::tempdir().unwrap();
    symbol_tree(dir.path());
    let cfg = IndexConfig::default();
    let snap1 = build_snapshot(dir.path(), &cfg).unwrap();
    let snap2 = build_snapshot(dir.path(), &cfg).unwrap();
    assert_eq!(snap1.files, snap2.files);
    assert_eq!(snap1.symbols, snap2.symbols);
    let hits = lookup_symbol(&snap1, "alpha");
    assert!(!hits.is_empty(), "alpha must be found");
    let keys: Vec<_> = hits
        .iter()
        .map(|s| (s.file.clone(), s.start_line))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

#[test]
fn tool016_t03_caps_truncate() {
    let dir = tempfile::tempdir().unwrap();
    symbol_tree(dir.path());
    let cfg = IndexConfig {
        max_files: 3,
        ..IndexConfig::default()
    };
    let snap = build_snapshot(dir.path(), &cfg).unwrap();
    assert!(snap.truncated);
    assert!(snap.files.len() <= 3);
    assert!(total_bytes(&snap) <= cfg.max_bytes);
    // Large generated tree completes bounded (no OOM, caps respected).
    let big = tempfile::tempdir().unwrap();
    for i in 0..10_000 {
        fs::write(big.path().join(format!("g{i:05}.txt")), b"x").unwrap();
    }
    let cap = IndexConfig {
        max_files: 100,
        ..IndexConfig::default()
    };
    let snap = build_snapshot(big.path(), &cap).unwrap();
    assert!(snap.truncated);
    assert!(snap.files.len() <= 100);
}

#[test]
fn tool016_t04_failure_states() {
    let missing = std::path::Path::new("/definitely/missing/tool-016-root-xyz");
    let err = build_snapshot(missing, &IndexConfig::default()).unwrap_err();
    assert_eq!(err, IndexError::RootUnreadable);
    // Broken symlink: single unreadable entry skipped, scan continues.
    let dir = tempfile::tempdir().unwrap();
    symbol_tree(dir.path());
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        dir.path().join("no-such-target.txt"),
        dir.path().join("broken.txt"),
    )
    .unwrap();
    let snap = build_snapshot(dir.path(), &IndexConfig::default()).unwrap();
    assert!(snap.skipped >= 1, "broken symlink must be skipped");
    assert_eq!(snap.files.len(), 10, "other files still indexed");
    // Pre-set cancel flag: prompt Cancelled.
    let cancel = AtomicBool::new(true);
    let err = build_snapshot_cancel(dir.path(), &IndexConfig::default(), &cancel).unwrap_err();
    assert_eq!(err, IndexError::Cancelled);
}

#[test]
fn tool016_t05_opt_out_zero_cost() {
    let missing = std::path::Path::new("/definitely/missing/tool-016-root-xyz");
    let snap = build_snapshot(missing, &IndexConfig::disabled()).unwrap();
    assert!(snap.files.is_empty());
    assert!(snap.symbols.is_empty());
    assert!(!snap.truncated);
    assert_eq!(snap.skipped, 0);
}
