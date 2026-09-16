//! TOOL-019 frozen tests: inbuilt RTK core filters (mirror RTK-CORE-T01..T05).
//!
//! Included via path so the owning module needs no shared-file wiring;
//! the integrator re-exports it from `lib.rs` per PLAN.md section 5.
//! Frozen after RED: never edit to make code pass, fix the implementation.

#[path = "../src/rtk_core.rs"]
mod rtk_core;

use rtk_core::{FilterKind, MARKER_MAX, RtkConfig, TRUNC_MARKER_SUFFIX, filter, raw};

const ALL_KINDS: [FilterKind; 8] = [
    FilterKind::GitStatus,
    FilterKind::GitDiff,
    FilterKind::GitLog,
    FilterKind::Ls,
    FilterKind::Read,
    FilterKind::Grep,
    FilterKind::Find,
    FilterKind::Diff,
];

/// Fixture `git-status-long`: >50 lines, every line wrapped in ANSI CSI color.
fn git_status_long() -> Vec<u8> {
    let mut v = Vec::new();
    for i in 0..60 {
        v.extend_from_slice(b"\x1b[32m");
        v.extend_from_slice(format!(" M src/file_{i:03}.rs\n").as_bytes());
        // NOTE: reset appended after newline keeps exact line count at 60.
        v.extend_from_slice(b"\x1b[0m");
    }
    v
}

fn has_marker(b: &[u8]) -> bool {
    b.len() >= 5 && b.windows(5).any(|w| w == b"[rtk:")
}

fn ends_with(hay: &[u8], needle: &[u8]) -> bool {
    hay.len() >= needle.len() && &hay[hay.len() - needle.len()..] == needle
}

#[test]
fn tool019_t01_filter_shrinks() {
    let input = git_status_long();
    assert!(
        input.iter().filter(|b| **b == b'\n').count() > 50,
        "fixture must exceed 50 lines"
    );
    let stderr = b"stderr kept verbatim\n";
    let cfg = RtkConfig::default();
    let out = filter(FilterKind::GitStatus, &input, stderr, 0, &cfg);
    assert!(
        out.stdout.len() < input.len(),
        "no shrink: {} vs {}",
        out.stdout.len(),
        input.len()
    );
    assert!(
        !out.stdout.iter().any(|b| *b == 0x1b),
        "ANSI CSI leaked: {:?}",
        &out.stdout[..out.stdout.len().min(120)]
    );
    assert_eq!(out.stderr, stderr);
    let again = filter(FilterKind::GitStatus, &input, stderr, 0, &cfg);
    assert_eq!(out, again, "filter not deterministic");
}

#[test]
fn tool019_t02_exit_code_preserved() {
    let stdout = b"hello\n";
    let stderr = b"warn\n";
    for kind in ALL_KINDS {
        for code in [0, 1, 128] {
            let out = filter(kind, stdout, stderr, code, &RtkConfig::default());
            assert_eq!(out.code, code, "code remapped for {kind:?}");
            assert_eq!(out.stderr, stderr, "stderr altered for {kind:?}");
        }
    }
}

#[test]
fn tool019_t03_empty_passthrough() {
    for kind in ALL_KINDS {
        let out = filter(kind, b"", b"", 0, &RtkConfig::default());
        assert_eq!(out.stdout, b"", "non-empty stdout for {kind:?}");
        assert!(!out.truncated, "truncated set on empty input");
        assert!(!has_marker(&out.stdout), "marker on empty input");
    }
}

#[test]
fn tool019_t04_overbudget_marker() {
    let mut input = Vec::new();
    for i in 0..2000 {
        input.extend_from_slice(
            format!("line {i:05} padding padding padding padding padding padding\n").as_bytes(),
        );
    }
    assert!(
        input.len() > 100 * 1024,
        "fixture under 100 KiB: {}",
        input.len()
    );
    let cfg = RtkConfig {
        enabled: true,
        max_bytes: 1024,
    };
    let out = filter(FilterKind::Read, &input, b"", 0, &cfg);
    assert!(out.truncated, "truncated flag not set over budget");
    assert!(
        ends_with(&out.stdout, TRUNC_MARKER_SUFFIX.as_bytes()),
        "missing marker suffix: {:?}",
        &out.stdout[out.stdout.len().saturating_sub(80)..]
    );
    assert!(
        out.stdout.len() <= 1024 + MARKER_MAX,
        "over budget: {}",
        out.stdout.len()
    );
    let again = filter(FilterKind::Read, &input, b"", 0, &cfg);
    assert_eq!(out, again, "filter not deterministic");
}

#[test]
fn tool019_t05_raw_identical() {
    let input = git_status_long();
    let stderr = b"\x1b[31merr\x1b[0m";
    for kind in ALL_KINDS {
        for code in [0, 1, 128] {
            let out = raw(&input, stderr, code);
            assert_eq!(out.stdout, input, "raw stdout altered for {kind:?}");
            assert_eq!(out.stderr, stderr, "raw stderr altered for {kind:?}");
            assert_eq!(out.code, code, "raw code altered for {kind:?}");
            assert!(!out.truncated, "raw sets truncated for {kind:?}");
        }
    }
}
