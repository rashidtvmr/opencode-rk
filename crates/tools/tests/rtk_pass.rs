//! TOOL-021 frozen tests: RTK pass/chain/proxy/opt-out (RTK-PASS-T01..T05).
//!
//! Included via path so the owning module needs no shared-file wiring.
//! Frozen after RED: never edit to make code pass, fix the implementation.

#[path = "../src/rtk_pass.rs"]
mod rtk_pass;

use rtk_pass::{
    ENV_NO_FILTER, MARKER_MAX, PassKind, RtkConfig, Segment, TRUNC_MARKER_SUFFIX, env_flag_value,
    filter_chain, filter_pass, kind_of, proxy, reset_shrink_calls, shrink_calls, split_chain,
};

const ALL_KNOWN: [PassKind; 12] = PassKind::ALL_KNOWN;

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || (hay.len() >= needle.len() && hay.windows(needle.len()).any(|w| w == needle))
}

fn ends_with(hay: &[u8], needle: &[u8]) -> bool {
    hay.len() >= needle.len() && &hay[hay.len() - needle.len()..] == needle
}

fn has_marker(b: &[u8]) -> bool {
    b.len() >= 5 && b.windows(5).any(|w| w == b"[rtk:")
}

/// ANSI-wrapped lines, >100 KiB total.
fn ansi_big() -> Vec<u8> {
    let mut v = Vec::new();
    for i in 0..1600 {
        v.extend_from_slice(b"\x1b[32m");
        v.extend_from_slice(
            format!("line {i:05} padding padding padding padding padding padding\n").as_bytes(),
        );
        v.extend_from_slice(b"\x1b[0m");
    }
    v
}

/// Mixed noise + one failure block + verdict line (shrinkable by known kinds).
fn fail_log() -> Vec<u8> {
    let mut v = Vec::new();
    for i in 0..300 {
        v.extend_from_slice(
            format!("info: routine noise line {i:03} all systems nominal\n").as_bytes(),
        );
    }
    v.extend_from_slice(b"\x1b[31mERROR E0599 worker failed on item 000042\x1b[0m\n");
    v.extend_from_slice(b"info: trailing context one\ninfo: trailing context two\n");
    v.extend_from_slice(b"verdict: FAIL item 000042\n");
    v
}

/// 500 KiB all-error log with verdict at the very end.
fn fail_log_big() -> Vec<u8> {
    let mut v = Vec::new();
    for i in 0..8500 {
        v.extend_from_slice(
            format!("ERROR E1001 subsystem worker failed on item {i:06} retry later\n").as_bytes(),
        );
    }
    v.extend_from_slice(b"verdict: FAIL batch rejects\n");
    v
}

#[test]
fn rtk_pass_t01_unknown_passthrough_identical() {
    let input = ansi_big();
    assert!(
        input.len() > 100 * 1024,
        "fixture under 100 KiB: {}",
        input.len()
    );
    let stderr = b"\x1b[31mwarn boom\x1b[0m\n".to_vec();
    let out = filter_pass(PassKind::Unknown, &input, &stderr, 3, &RtkConfig::default());
    assert_eq!(out.stdout, input);
    assert_eq!(out.stderr, stderr);
    assert_eq!(out.code, 3);
    assert!(!out.truncated);
    // Unknown with nonzero exit never drops bytes, even over budget.
    let out2 = filter_pass(PassKind::Unknown, &input, b"", 1, &RtkConfig::default());
    assert_eq!(out2.stdout, input);
    assert!(!out2.truncated);
    assert!(!has_marker(&out2.stdout));
    // Empty unknown: passthrough, no marker.
    let out3 = filter_pass(PassKind::Unknown, b"", b"", 0, &RtkConfig::default());
    assert_eq!(out3.stdout, b"");
    assert!(!out3.truncated);
}

#[test]
fn rtk_pass_t02_chain_segments_independent() {
    // Quoted && must not split: exactly 3 segments.
    let segs = split_chain(r#"log ok && err "a && b" fail && mystery-cmd"#);
    assert_eq!(segs.len(), 3, "bad split: {segs:?}");
    assert_eq!(kind_of(&segs[0]), PassKind::Log);
    assert_eq!(kind_of(&segs[1]), PassKind::Err);
    assert_eq!(kind_of(&segs[2]), PassKind::Unknown);
    // Segment outputs: A ok-noise, B failure with id+verdict, C unknown bytes.
    let mut a_in = Vec::new();
    for i in 0..300 {
        a_in.extend_from_slice(format!("info: noise line {i:03} nominal\n").as_bytes());
    }
    let b_in = b"info: start\nFAIL-ID-7749 ERROR worker crashed\nverdict: FAIL 7749\n".to_vec();
    let c_in = b"\x1b[33mraw?\x1b[0m\n".to_vec();
    let chain = vec![
        Segment {
            kind: PassKind::Log,
            stdout: a_in.clone(),
            stderr: vec![],
            code: 0,
        },
        Segment {
            kind: PassKind::Err,
            stdout: b_in.clone(),
            stderr: b"kept\n".to_vec(),
            code: 2,
        },
        Segment {
            kind: PassKind::Unknown,
            stdout: c_in.clone(),
            stderr: vec![],
            code: 0,
        },
    ];
    let (outs, exit) = filter_chain(&chain, &RtkConfig::default());
    assert_eq!(outs.len(), 3);
    assert!(outs[0].stdout.len() < a_in.len(), "seg A not shrunk");
    assert!(
        contains(&outs[1].stdout, b"FAIL-ID-7749"),
        "seg B lost fail id"
    );
    assert!(contains(&outs[1].stdout, b"verdict"), "seg B lost verdict");
    assert_eq!(outs[1].stderr, b"kept\n", "seg B stderr altered");
    assert_eq!(outs[2].stdout, c_in, "seg C rewritten by sibling failure");
    assert_eq!(exit, 2);
    assert!(
        !contains(&outs[0].stdout, b"FAIL-ID-7749"),
        "failure leaked into seg A"
    );
}

#[test]
fn rtk_pass_t03_proxy_accounts_without_filtering() {
    let input = fail_log_big();
    assert!(
        input.len() > 100 * 1024,
        "fixture under 100 KiB: {}",
        input.len()
    );
    let stderr = b"proxy-err\n".to_vec();
    // Sanity: the plain path would truncate this input over the default budget.
    let plain = filter_pass(PassKind::Log, &input, &stderr, 1, &RtkConfig::default());
    assert!(
        plain.truncated,
        "fixture does not exceed budget, proxy test vacuous"
    );
    let mut accounted = 0usize;
    let out = proxy(&input, &stderr, 1, &mut accounted);
    assert_eq!(out.stdout, input);
    assert_eq!(out.stderr, stderr);
    assert_eq!(out.code, 1);
    assert!(!out.truncated);
    assert!(!has_marker(&out.stdout), "proxy emitted marker");
    assert_eq!(accounted, input.len() + stderr.len());
}

#[test]
fn rtk_pass_t04_opt_out_zero_overhead() {
    let input = fail_log();
    let stderr = b"e\n".to_vec();
    let cfgs = [
        RtkConfig {
            enabled: true,
            ..RtkConfig::default()
        },
        RtkConfig {
            enabled: true,
            no_filter: true,
            ..RtkConfig::default()
        },
        RtkConfig::disabled(),
        RtkConfig {
            no_filter: true,
            ..RtkConfig::disabled()
        },
    ];
    // Control first: enabled + no per-command flag really shrinks.
    reset_shrink_calls();
    let mut shrunk_any = false;
    for kind in ALL_KNOWN {
        let out = filter_pass(kind, &input, &stderr, 1, &cfgs[0]);
        if out.stdout != input {
            shrunk_any = true;
        }
    }
    assert!(
        shrunk_any,
        "control config does not shrink, opt-out test vacuous"
    );
    assert!(shrink_calls() > 0, "shrink counter dead");
    // Opt-out matrix: global OR per-command each suffices, zero shrink work.
    reset_shrink_calls();
    for kind in ALL_KNOWN {
        for cfg in [&cfgs[1], &cfgs[2], &cfgs[3]] {
            let out = filter_pass(kind, &input, &stderr, 1, cfg);
            assert_eq!(out.stdout, input, "filtered despite opt-out for {kind:?}");
            assert_eq!(out.stderr, stderr);
            assert_eq!(out.code, 1);
            assert!(!out.truncated);
        }
    }
    assert_eq!(shrink_calls(), 0, "shrink path taken while opted out");
    // Env flag mapping (pure, no process-env mutation).
    assert!(env_flag_value(Some(std::ffi::OsStr::new("1"))));
    assert!(env_flag_value(Some(std::ffi::OsStr::new("true"))));
    assert!(env_flag_value(Some(std::ffi::OsStr::new("YES"))));
    assert!(!env_flag_value(None));
    assert!(!env_flag_value(Some(std::ffi::OsStr::new("0"))));
    assert!(!env_flag_value(Some(std::ffi::OsStr::new(""))));
    let _ = ENV_NO_FILTER;
}

#[test]
fn rtk_pass_t05_budget_marker() {
    let input = fail_log_big();
    assert!(
        input.len() > 500 * 1024,
        "fixture under 500 KiB: {}",
        input.len()
    );
    let stderr = b"kept-stderr\n".to_vec();
    let cfg = RtkConfig {
        enabled: true,
        max_bytes: 65536,
        no_filter: false,
    };
    let out = filter_pass(PassKind::Err, &input, &stderr, 1, &cfg);
    assert!(out.truncated);
    assert!(
        out.stdout.len() <= 65536 + MARKER_MAX,
        "over budget: {}",
        out.stdout.len()
    );
    assert!(
        ends_with(&out.stdout, TRUNC_MARKER_SUFFIX.as_bytes()),
        "missing marker suffix"
    );
    assert!(
        contains(&out.stdout, b"verdict"),
        "verdict lost under budget"
    );
    assert_eq!(out.stderr, stderr, "stderr dropped");
    assert_eq!(out.code, 1, "code remapped");
}
