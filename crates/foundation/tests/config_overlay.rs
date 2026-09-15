//! OPS-005 frozen tests T01..T05: pure location-scoped config overlay merge.
//!
//! Integration target `config_overlay`. The overlay module is included by
//! path until the integrator wires `pub mod config_overlay` in the crate
//! root; this test file is frozen after RED and must not be edited to
//! make code pass.

#[path = "../src/config_overlay.rs"]
mod config_overlay;

use config_overlay::{merge_overlay, ConfigDoc, OverlayError, Scope};
use std::fs;

fn doc(scope: Scope, pairs: &[(&str, &str)]) -> ConfigDoc {
    ConfigDoc {
        scope,
        keys: pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
    }
}

#[test]
fn ops005_t01_happy_path_overlay() {
    let docs = vec![
        doc(Scope::Global, &[("a", "1"), ("b", "1")]),
        doc(Scope::Project, &[("b", "2")]),
        doc(Scope::Local, &[("c", "3")]),
    ];
    let cfg = merge_overlay(&docs).unwrap();
    assert_eq!(cfg.get("a"), Some("1"));
    assert_eq!(cfg.get("b"), Some("2"));
    assert_eq!(cfg.get("c"), Some("3"));
    assert_eq!(
        cfg.winner,
        vec![
            ("a".to_string(), Scope::Global),
            ("b".to_string(), Scope::Project),
            ("c".to_string(), Scope::Local),
        ]
    );
}

#[test]
fn ops005_t02_determinism_and_order() {
    // Shuffled input doc order; explicit scopes still decide winners.
    let docs = vec![
        doc(Scope::Local, &[("c", "3")]),
        doc(Scope::Global, &[("a", "1"), ("b", "1")]),
        doc(Scope::Project, &[("b", "2")]),
    ];
    let run1 = merge_overlay(&docs).unwrap();
    let run2 = merge_overlay(&docs).unwrap();
    assert_eq!(run1.keys, run2.keys);
    assert!(run1.keys.windows(2).all(|w| w[0] <= w[1]));
    let mut rev = docs.clone();
    rev.reverse();
    let run3 = merge_overlay(&rev).unwrap();
    assert_eq!(run1.keys, run3.keys);
    assert_eq!(run1.winner, run3.winner);
}

#[test]
fn ops005_t03_caps() {
    let big = ConfigDoc {
        scope: Scope::Global,
        keys: (0..4097)
            .map(|i| (format!("k{i:05}"), "v".to_string()))
            .collect(),
    };
    assert_eq!(merge_overlay(&[big]).unwrap_err(), OverlayError::TooMany);
    let huge_value = "x".repeat(64 * 1024 + 1);
    assert_eq!(
        merge_overlay(&[doc(Scope::Global, &[("k", huge_value.as_str())])]).unwrap_err(),
        OverlayError::BadEntry
    );
    let long_key = "k".repeat(257);
    assert_eq!(
        merge_overlay(&[doc(Scope::Global, &[(long_key.as_str(), "v")])]).unwrap_err(),
        OverlayError::BadEntry
    );
}

#[test]
fn ops005_t04_failure_states() {
    assert_eq!(merge_overlay(&[]).unwrap_err(), OverlayError::NoDocs);
    assert_eq!(
        merge_overlay(&[doc(Scope::Global, &[("dup", "1"), ("dup", "2")])]).unwrap_err(),
        OverlayError::DupKey
    );
    assert_eq!(
        merge_overlay(&[doc(Scope::Global, &[("", "v")])]).unwrap_err(),
        OverlayError::BadEntry
    );
}

#[test]
fn ops005_t05_purity_and_safety() {
    // Disposable fixture dir: the merge itself takes no fs/env handles.
    let dir = std::env::temp_dir().join(format!("ops005-purity-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let before = fs::read_dir(&dir).unwrap().count();
    let docs = vec![
        doc(Scope::Global, &[("ka", "val-alpha-secret")]),
        doc(Scope::Local, &[("kb", "val-beta-secret")]),
    ];
    let cfg = merge_overlay(&docs).unwrap();
    let after = fs::read_dir(&dir).unwrap().count();
    let fs_calls = after - before;
    assert_eq!(fs_calls, 0);
    // Env-insensitive output: no env reads can affect the pure merge.
    std::env::set_var("OPS005_PROBE", "one");
    let r1 = merge_overlay(&docs).unwrap();
    std::env::set_var("OPS005_PROBE", "two");
    let r2 = merge_overlay(&docs).unwrap();
    std::env::remove_var("OPS005_PROBE");
    let env_reads = if r1 == r2 { 0 } else { 1 };
    assert_eq!(env_reads, 0);
    // Key-names-only logging: the only log lines the lane emits name keys.
    let logs: Vec<String> = cfg
        .winner
        .iter()
        .map(|(k, s)| format!("key={k} scope={s:?}"))
        .collect();
    let blob = logs.join("\n");
    assert!(blob.contains("ka"));
    assert!(!blob.contains("val-alpha-secret"));
    assert!(!blob.contains("val-beta-secret"));
    // Error rendering never carries value bytes either.
    let secret_value = format!("s3cr3t-{}-tail", "x".repeat(64 * 1024));
    let err = merge_overlay(&[doc(Scope::Global, &[("k", secret_value.as_str())])]).unwrap_err();
    assert_eq!(err, OverlayError::BadEntry);
    let rendered = format!("{err} {err:?}");
    assert!(!rendered.contains("s3cr3t"));
    // Fixture dir cleaned; nothing retained outside it.
    fs::remove_dir_all(&dir).unwrap();
    assert!(!dir.exists());
}
