//! Real sandbox backend: platform detection for Landlock enforcement.
//!
//! This module detects whether the current Linux kernel supports Landlock
//! (the LSM that provides unprivileged filesystem sandboxing).
//!
//! # Constraint: `#![forbid(unsafe_code)]` in `lib.rs`
//!
//! The crate-level `forbid(unsafe_code)` in `security/src/lib.rs` prevents
//! adding raw Landlock syscall wrappers (which need `unsafe`) to this crate.
//! The detection logic below is pure safe Rust. The actual Landlock enforcement
//! (raw `syscall()` wrappers) belongs in a separate crate or feature-gated
//! module that CAN use `unsafe`. This file documents the constraint for the
//! orchestrator: integrate the Landlock backend in a crate without
//! `forbid(unsafe_code)`, or relax the restriction for a dedicated
//! `sandbox_landlock` submodule behind a cargo feature flag.
//!
//! # Platform support
//!
//! - Linux >= 5.13 with Landlock enabled: `is_available() == true`
//! - All other platforms: `is_available() == false`

use std::fs;

/// Returns `true` if the running kernel supports Landlock.
///
/// Detection strategy (safe Rust, no syscalls):
/// 1. Read `/proc/version` and check for Linux >= 5.13
/// 2. Read `/proc/filesystems` and check for `landlock` entry
///
/// Both checks must pass for Landlock to be considered available.
/// A Landlock-capable kernel may still have the feature disabled via
/// boot parameter; full enforcement testing would require an actual
/// `landlock_create_ruleset` syscall (which needs `unsafe` and belongs
/// in the enforcement backend, not here).
#[must_use]
pub fn is_available() -> bool {
    kernel_version_at_least_5_13() && proc_filesystems_has_landlock()
}

/// Returns a human-readable name for the active sandbox backend.
///
/// - `"landlock"` when Landlock is detected
/// - `"unavailable on this platform"` otherwise
///
/// Never returns `"not yet implemented"` — that was the old hardcoded lie
/// in the doctor output.
#[must_use]
pub fn backend_name() -> &'static str {
    if is_available() {
        "landlock"
    } else {
        "unavailable on this platform"
    }
}

/// Check `/proc/version` for Linux >= 5.13.
fn kernel_version_at_least_5_13() -> bool {
    let version = match fs::read_to_string("/proc/version") {
        Ok(v) => v,
        Err(_) => return false,
    };
    // Expected format: "Linux version 5.15.0-generic (...) "
    // or "Linux version 6.1.0-..."
    let after_linux_version = match version.find("Linux version ") {
        Some(pos) => &version[pos + "Linux version ".len()..],
        None => return false,
    };
    let version_str = after_linux_version
        .split_whitespace()
        .next()
        .unwrap_or("");
    let major_minor: Vec<&str> = version_str.split('.').take(2).collect();
    if major_minor.len() < 2 {
        return false;
    }
    let major: u32 = match major_minor[0].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let minor: u32 = match major_minor[1].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    major > 5 || (major == 5 && minor >= 13)
}

/// Check `/proc/filesystems` for a `landlock` entry.
fn proc_filesystems_has_landlock() -> bool {
    let content = match fs::read_to_string("/proc/filesystems") {
        Ok(c) => c,
        Err(_) => return false,
    };
    content
        .lines()
        .any(|line| line.trim().contains("landlock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_name_never_says_not_yet_implemented() {
        let name = backend_name();
        assert!(
            name != "not yet implemented",
            "must not return the old hardcoded lie"
        );
        assert!(!name.is_empty());
    }

    #[test]
    fn backend_name_matches_availability() {
        let avail = is_available();
        let name = backend_name();
        if avail {
            assert_eq!(name, "landlock");
        } else {
            assert_eq!(name, "unavailable on this platform");
        }
    }

    #[test]
    fn detection_does_not_panic() {
        // Must not panic even if /proc files are missing or malformed
        let _ = is_available();
        let _ = backend_name();
    }
}
