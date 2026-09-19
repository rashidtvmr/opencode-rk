//! LANE-SANDBOX integration tests: sandbox policy engine enforces real filesystem
//! decisions. Three scenarios:
//!   A (allow): write under allowed root is permitted
//!   B (deny):  write outside allowed root is blocked (SandboxDenied)
//!   C (deny-by-default + platform honesty): empty policy denies everything;
//!     sandbox_real detects the platform honestly (no "not yet implemented")
//!
//! Uses #[path] to compile against crate modules directly (no lib.rs wiring needed).
#![forbid(unsafe_code)]

#[path = "../src/sandbox.rs"]
mod sandbox;
#[path = "../src/sandbox_real.rs"]
mod sandbox_real;

use sandbox::{FileAction, SandboxCheck, SandboxPolicy};
use std::fs;
use std::path::PathBuf;

fn tmp_base(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rk-sandbox-enforce-{}-{tag}",
        std::process::id()
    ))
}

// ── Scenario A (allow): write under <tmp>/work is permitted ───────────────

#[test]
fn scenario_a_allow_write_under_allowed_root() {
    let base = tmp_base("allow");
    let _ = fs::remove_dir_all(&base);
    let work = base.join("work");
    fs::create_dir_all(&work).expect("create allowed work dir");

    let policy = SandboxPolicy::new(
        vec![base.clone()],
        vec![work.clone()],
        Vec::new(),
    );
    let checker = SandboxCheck::new(policy);

    let file = work.join("output.txt");
    // is_allowed must permit the write
    checker
        .is_allowed(&file, FileAction::Write)
        .expect("write under allowed root must be permitted");

    // Real filesystem write must succeed
    fs::write(&file, b"allowed content").expect("real write must succeed");
    assert_eq!(fs::read(&file).unwrap(), b"allowed content");

    let _ = fs::remove_dir_all(&base);
}

// ── Scenario B (deny): write outside allowed roots returns SandboxDenied ─

#[test]
fn scenario_b_deny_write_outside_allowed_roots() {
    let base = tmp_base("deny");
    let _ = fs::remove_dir_all(&base);
    let work = base.join("work");
    let outside = base.join("outside");
    fs::create_dir_all(&work).expect("create work dir");
    fs::create_dir_all(&outside).expect("create outside dir");

    let policy = SandboxPolicy::new(
        vec![base.clone()],
        vec![work.clone()],
        Vec::new(),
    );
    let checker = SandboxCheck::new(policy);

    let outside_file = outside.join("secret.txt");

    // is_allowed must deny the write
    let err = checker
        .is_allowed(&outside_file, FileAction::Write)
        .expect_err("write outside allowed roots must be denied");

    assert_eq!(err.path, outside_file);
    assert_eq!(err.action, FileAction::Write);
    assert!(
        err.reason.contains("outside the sandbox"),
        "denial reason must explain the cause: {}",
        err.reason
    );

    // Nothing must be written at the denied path
    assert!(
        !outside_file.exists(),
        "file must not exist after denied write"
    );

    let _ = fs::remove_dir_all(&base);
}

// ── Scenario C (deny-by-default + platform honesty) ──────────────────────

#[test]
fn scenario_c_empty_policy_denies_everything() {
    let policy = SandboxPolicy::new(Vec::new(), Vec::new(), Vec::new());
    let checker = SandboxCheck::new(policy);

    let paths = [
        "/tmp/some-file.txt",
        "/etc/hosts",
        "/home/user/data.csv",
    ];
    for path_str in &paths {
        let err = checker
            .is_allowed(path_str, FileAction::Read)
            .expect_err("empty policy must deny all reads");
        assert_eq!(err.action, FileAction::Read);

        let err = checker
            .is_allowed(path_str, FileAction::Write)
            .expect_err("empty policy must deny all writes");
        assert_eq!(err.action, FileAction::Write);
    }
}

#[test]
fn scenario_c_sandbox_real_honest_platform_detection() {
    // sandbox_real must detect the current platform honestly.
    // The old doctor line claimed "not yet implemented" — that lie must be gone.
    let name = sandbox_real::backend_name();
    assert!(
        name != "not yet implemented",
        "backend_name must not return the old hardcoded lie"
    );
    assert!(
        !name.is_empty(),
        "backend_name must return a non-empty string"
    );
    let avail = sandbox_real::is_available();
    if avail {
        assert_eq!(name, "landlock");
    } else {
        assert_eq!(name, "unavailable on this platform");
    }
}
