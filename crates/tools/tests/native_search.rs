#[path = "../src/native_search.rs"]
mod native_search;

mod file_ops {
    pub use opencode_rk_tools::file_ops::*;
}

use native_search::search;
use opencode_rk_security::{PermissionBroker, SecurityPolicy};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn broker(root: &std::path::Path) -> PermissionBroker {
    PermissionBroker::new(SecurityPolicy::lean_default(root))
}

#[test]
fn grep_is_literal_bounded_and_honors_hidden() {
    let d = tempdir().unwrap();
    fs::write(d.path().join("a.rs"), "hello world\n").unwrap();
    fs::write(d.path().join(".secret"), "hello hidden\n").unwrap();
    let out = search(
        &json!({"query":"hello.*", "literal":true}),
        d.path(),
        &broker(d.path()),
        "grep",
    )
    .unwrap();
    assert!(out.contains("result") && !out.contains(".secret"));
}

#[test]
fn glob_supports_recursive_include_and_hidden_switch() {
    let d = tempdir().unwrap();
    fs::create_dir(d.path().join("src")).unwrap();
    fs::write(d.path().join("src/lib.rs"), "x").unwrap();
    fs::write(d.path().join(".x.rs"), "x").unwrap();
    let out = search(
        &json!({"query":"src/**/*.rs"}),
        d.path(),
        &broker(d.path()),
        "glob",
    )
    .unwrap();
    assert!(out.contains("src/lib.rs") && !out.contains(".x.rs"));
}

#[test]
fn symlink_is_not_followed() {
    let d = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("x.rs"), "x").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), d.path().join("link")).unwrap();
    let out = search(
        &json!({"query":"**/*.rs"}),
        d.path(),
        &broker(d.path()),
        "glob",
    )
    .unwrap();
    assert!(!out.contains("x.rs"));
}

#[test]
fn limit_and_include_are_honored_for_grep() {
    let d = tempdir().unwrap();
    fs::write(d.path().join("a.rs"), "alpha needle one\n").unwrap();
    fs::write(d.path().join("b.rs"), "beta needle two\n").unwrap();
    fs::write(d.path().join("c.txt"), "gamma needle three\n").unwrap();
    let out = search(
        &json!({"query":"needle", "include":"*.rs", "limit":1}),
        d.path(),
        &broker(d.path()),
        "grep",
    )
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let results = v["result"].as_array().unwrap();
    assert_eq!(results.len(), 1, "limit must cap results: {out}");
    let path = results[0]["path"].as_str().unwrap();
    assert!(path.ends_with(".rs"), "include must filter to *.rs: {path}");
    assert!(results[0]["line"].as_str().unwrap().contains("needle"));
    assert!(
        v["output"].as_str().unwrap().contains("needle"),
        "output carries line content"
    );
    assert!(!out.contains("c.txt"), "include must exclude c.txt");
}

#[test]
fn visit_budget_counts_nonmatching_entries() {
    let d = tempdir().unwrap();
    for name in ["a", "b", "c", "d"] {
        fs::write(d.path().join(name), "nothing here\n").unwrap();
    }
    let out = search(
        &json!({"query":"absent", "maxEntries":2}),
        d.path(),
        &broker(d.path()),
        "grep",
    )
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["entries"].as_u64(),
        Some(2),
        "visits count entries even without matches: {out}"
    );
    assert!(v["result"].as_array().unwrap().is_empty());
    assert_eq!(v["truncated"], true, "visit budget must truncate the walk");
}

#[test]
fn traversal_and_symlink_escape_are_rejected_and_bounds_validated() {
    let d = tempdir().unwrap();
    assert!(
        search(
            &json!({"query":"x", "path":"../"}),
            d.path(),
            &broker(d.path()),
            "glob"
        )
        .is_err()
    );

    let outside = tempdir().unwrap();
    fs::write(outside.path().join("secret.rs"), "x").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), d.path().join("escape")).unwrap();
    assert!(
        search(
            &json!({"query":"**/*.rs", "path":"escape"}),
            d.path(),
            &broker(d.path()),
            "glob"
        )
        .is_err()
    );
    assert!(
        search(
            &json!({"query":"x", "path":outside.path().to_string_lossy()}),
            d.path(),
            &broker(d.path()),
            "glob",
        )
        .is_err()
    );

    let long_query = "a".repeat(257);
    assert!(
        search(
            &json!({"query":long_query}),
            d.path(),
            &broker(d.path()),
            "grep"
        )
        .is_err()
    );
    assert!(
        search(
            &json!({"query":"x", "literal":false}),
            d.path(),
            &broker(d.path()),
            "grep"
        )
        .is_err()
    );
}
