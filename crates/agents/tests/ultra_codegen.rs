#[path = "../src/ultra_codegen.rs"]
mod ultra_codegen;

use ultra_codegen::{
    CacheKey, CodegenConfig, CompileResult, DenyReason, ExecutionBounds,
    UltraCodegen, UltraState,
};

fn default_config() -> CodegenConfig {
    CodegenConfig::default()
}

fn custom_config(max_attempts: usize, max_source_bytes: usize) -> CodegenConfig {
    CodegenConfig {
        max_attempts,
        max_source_bytes,
        ..CodegenConfig::default()
    }
}

// ── T01: lifecycle starts in Requested state ──

#[test]
fn t01_initial_state_is_requested() {
    let gen = UltraCodegen::new(default_config());
    assert_eq!(gen.state(), &UltraState::Requested);
}

// ── T02: emit_draft transitions to DraftEmitted with content hash ──

#[test]
fn t02_emit_draft_produces_hash() {
    let mut gen = UltraCodegen::new(default_config());
    let source = r#"fn main() { println!("hello"); }"#;
    let draft = gen.emit_draft(source).unwrap();
    assert!(matches!(gen.state(), UltraState::DraftEmitted(_)));
    assert!(!draft.source_hash.is_empty());
    assert_eq!(draft.source, source);
}

// ── T03: same source produces same cache key (determinism) ──

#[test]
fn t03_same_source_same_cache_key() {
    let mut g1 = UltraCodegen::new(default_config());
    let mut g2 = UltraCodegen::new(default_config());
    let source = r#"fn main() {}"#;
    let d1 = g1.emit_draft(source).unwrap();
    let d2 = g2.emit_draft(source).unwrap();
    assert_eq!(d1.cache_key(), d2.cache_key());
}

// ── T04: different source produces different cache key ──

#[test]
fn t04_different_source_different_cache_key() {
    let mut g1 = UltraCodegen::new(default_config());
    let mut g2 = UltraCodegen::new(default_config());
    let d1 = g1.emit_draft("fn a() {}").unwrap();
    let d2 = g2.emit_draft("fn b() {}").unwrap();
    assert_ne!(d1.cache_key(), d2.cache_key());
}

// ── T05: compile_success transitions DraftEmitted -> Compiled ──

#[test]
fn t05_compile_success_transitions_to_compiled() {
    let mut gen = UltraCodegen::new(default_config());
    gen.emit_draft("fn main() {}").unwrap();
    gen.request_compile();
    assert!(matches!(gen.state(), UltraState::CompileRequested));
    gen.compile_success(CompileResult {
        binary_path: "/tmp/test-binary".into(),
    });
    assert!(matches!(gen.state(), UltraState::Compiled { .. }));
}

// ── T06: compile_failure increments attempt counter ──

#[test]
fn t06_compile_failure_increments_attempt() {
    let mut gen = UltraCodegen::new(custom_config(3, 1024));
    gen.emit_draft("bad code").unwrap();
    gen.request_compile();
    gen.compile_failure("syntax error");
    match gen.state() {
        UltraState::CompileFailed { attempt, .. } => assert_eq!(*attempt, 1),
        other => panic!("expected CompileFailed, got {other:?}"),
    }
}

// ── T07: repeated failures trigger fallback after max_attempts ──

#[test]
fn t07_fallback_after_max_attempts() {
    let mut gen = UltraCodegen::new(custom_config(2, 1024));
    gen.emit_draft("bad").unwrap();

    gen.request_compile();
    gen.compile_failure("err1");
    assert!(matches!(gen.state(), UltraState::CompileFailed { attempt: 1, .. }));

    gen.request_compile();
    gen.compile_failure("err2");
    assert!(matches!(gen.state(), UltraState::Fallback { .. }));
}

// ── T08: denied for unsafe_without_approval ──

#[test]
fn t08_deny_unsafe_without_approval() {
    let mut gen = UltraCodegen::new(default_config());
    gen.emit_draft("fn main() {}").unwrap();
    gen.deny(DenyReason::UnsafeWithoutApproval);
    assert!(matches!(gen.state(), UltraState::Denied(DenyReason::UnsafeWithoutApproval)));
}

// ── T09: denied for forbidden_api_hit ──

#[test]
fn t09_deny_forbidden_api() {
    let mut gen = UltraCodegen::new(default_config());
    gen.emit_draft("fn main() {}").unwrap();
    gen.deny(DenyReason::ForbiddenApiHit { pattern: "std::process::Command".into() });
    match gen.state() {
        UltraState::Denied(DenyReason::ForbiddenApiHit { pattern }) => {
            assert_eq!(pattern, "std::process::Command");
        }
        other => panic!("expected Denied(ForbiddenApiHit), got {other:?}"),
    }
}

// ── T10: denied for bounds_exceeded ──

#[test]
fn t10_deny_bounds_exceeded() {
    let mut gen = UltraCodegen::new(default_config());
    gen.emit_draft("fn main() {}").unwrap();
    gen.deny(DenyReason::BoundsExceeded { bytes: 999_999, limit: 1024 });
    match gen.state() {
        UltraState::Denied(DenyReason::BoundsExceeded { bytes, limit }) => {
            assert_eq!(*bytes, 999_999);
            assert_eq!(*limit, 1024);
        }
        other => panic!("expected Denied(BoundsExceeded), got {other:?}"),
    }
}

// ── T11: denied for build_failure_n ──

#[test]
fn t11_deny_build_failure_n() {
    let mut gen = UltraCodegen::new(default_config());
    gen.emit_draft("fn main() {}").unwrap();
    gen.deny(DenyReason::BuildFailureN { attempts: 5 });
    match gen.state() {
        UltraState::Denied(DenyReason::BuildFailureN { attempts }) => {
            assert_eq!(*attempts, 5);
        }
        other => panic!("expected Denied(BuildFailureN), got {other:?}"),
    }
}

// ── T12: source bounds enforcement on emit ──

#[test]
fn t12_source_too_large_is_denied() {
    let mut gen = UltraCodegen::new(custom_config(3, 10));
    let big_source = "x".repeat(20);
    let result = gen.emit_draft(&big_source);
    assert!(result.is_err());
    match gen.state() {
        UltraState::Denied(DenyReason::BoundsExceeded { bytes, limit }) => {
            assert_eq!(*bytes, 20);
            assert_eq!(*limit, 10);
        }
        other => panic!("expected Denied(BoundsExceeded), got {other:?}"),
    }
}

// ── T13: cache hit skips compile (simulated) ──

#[test]
fn t13_cache_hit_skips_compile() {
    let mut gen = UltraCodegen::new(default_config());
    gen.emit_draft("fn main() {}").unwrap();
    let cache_key = gen.cache_key().unwrap().clone();
    // Simulate cache hit: another instance with same source
    let mut gen2 = UltraCodegen::new(default_config());
    gen2.emit_draft("fn main() {}").unwrap();
    let hit = gen2.cache_lookup(&cache_key);
    assert!(hit, "cache should hit for identical source");
}

// ── T14: cache miss does not match ──

#[test]
fn t14_cache_miss_no_match() {
    let mut gen = UltraCodegen::new(default_config());
    gen.emit_draft("fn main() {}").unwrap();
    let wrong_key = CacheKey("deadbeef".into());
    let hit = gen.cache_lookup(&wrong_key);
    assert!(!hit, "cache should miss for different key");
}

// ── T15: denylist checks draft text for forbidden patterns ──

#[test]
fn t15_denylist_rejects_forbidden_api_in_source() {
    let mut gen = UltraCodegen::new(default_config());
    let result = gen.emit_draft("use std::process::Command; fn main() {}");
    assert!(result.is_err());
    match gen.state() {
        UltraState::Denied(DenyReason::ForbiddenApiHit { pattern }) => {
            assert!(pattern.contains("Command"));
        }
        other => panic!("expected Denied(ForbiddenApiHit), got {other:?}"),
    }
}

// ── T16: safe source passes denylist ──

#[test]
fn t16_denylist_allows_safe_source() {
    let mut gen = UltraCodegen::new(default_config());
    let result = gen.emit_draft("fn add(a: i32, b: i32) -> i32 { a + b }");
    assert!(result.is_ok());
    assert!(matches!(gen.state(), UltraState::DraftEmitted(_)));
}

// ── T17: execution bounds recorded after execute ──

#[test]
fn t17_execute_records_bounds() {
    let mut gen = UltraCodegen::new(default_config());
    gen.emit_draft("fn main() {}").unwrap();
    gen.request_compile();
    gen.compile_success(CompileResult {
        binary_path: "/tmp/bin".into(),
    });
    gen.execute(ExecutionBounds {
        max_bytes: 1024,
        max_steps: 500,
    });
    assert!(matches!(gen.state(), UltraState::Executed { .. }));
}

// ── T18: emitted manifest requires forbid(unsafe_code) ──

#[test]
fn t18_generated_manifest_requires_forbid_unsafe() {
    let gen = UltraCodegen::new(default_config());
    let manifest = gen.generate_manifest("test_crate");
    assert!(
        manifest.contains("forbid(unsafe_code)"),
        "generated Cargo.toml must include #![forbid(unsafe_code]): {manifest}"
    );
}

// ── T19: multiple deny reasons don't stack state ──

#[test]
fn t19_deny_overwrites_previous() {
    let mut gen = UltraCodegen::new(default_config());
    gen.emit_draft("fn main() {}").unwrap();
    gen.deny(DenyReason::UnsafeWithoutApproval);
    gen.deny(DenyReason::BoundsExceeded { bytes: 100, limit: 50 });
    match gen.state() {
        UltraState::Denied(DenyReason::BoundsExceeded { bytes, limit }) => {
            assert_eq!(*bytes, 100);
            assert_eq!(*limit, 50);
        }
        other => panic!("expected Denied(BoundsExceeded), got {other:?}"),
    }
}

// ── T20: compile_failure then re-request compile ──

#[test]
fn t20_retry_after_failure() {
    let mut gen = UltraCodegen::new(custom_config(3, 1024));
    gen.emit_draft("bad").unwrap();
    gen.request_compile();
    gen.compile_failure("err1");
    gen.request_compile();
    assert!(matches!(gen.state(), UltraState::CompileRequested));
    gen.compile_success(CompileResult {
        binary_path: "/tmp/bin".into(),
    });
    assert!(matches!(gen.state(), UltraState::Compiled { .. }));
}
