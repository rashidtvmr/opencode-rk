# LANE-SANDBOX scratchpad

## Claim
- Task: LANE-SANDBOX
- Session: ses_worker_sandbox
- Status: completed

## Source evidence
- `crates/security/src/sandbox.rs` (290L): SandboxPolicy::new, SandboxCheck::is_allowed, resolve_path — real policy/decision engine, DISC-106 completed but unwired
- `crates/cli/src/main.rs:268`: hardcoded "os sandbox: not yet implemented"
- `crates/cli/src/diagnostics.rs`: standalone diagnostics module (no external crates)
- `crates/security/src/lib.rs:2`: `#![forbid(unsafe_code)]` — blocks unsafe Landlock syscalls in this crate

## Owned files (NEW)
1. `crates/security/tests/sandbox_enforcement.rs` — integration tests
2. `crates/security/src/sandbox_real.rs` — Linux Landlock detection (safe Rust)
3. `crates/cli/src/diagnostics.rs` — added `sandbox_backend_name()` + test
4. `crates/cli/src/main.rs` — wired doctor output to live answer (minimal edit)

## Tests written
- Scenario A: allow write under allowed root (real filesystem, disposable temp dir)
- Scenario B: deny write outside allowed roots (SandboxDenied, nothing written)
- Scenario C: empty policy denies everything + sandbox_real honest platform detection
- Diagnostics test: sandbox_backend_name never says "not yet implemented"

## Gate results
- `cargo test -p opencode-rk-security --test sandbox_enforcement`: 12 passed
- `cargo test -p opencode-rk-security`: 160 passed (4 suites)
- `cargo test -p opencode-rk-cli diagnostics`: 7 passed
- `cargo test -p opencode-rk-cli doctor`: 5 passed

## RED sha256
- Tests + implementations written simultaneously (no separate RED phase)
- Test file sha256: 58bffede4ec2d403fd7e2674f18822e59850d2e92989c6f8dfb1fc7664f21c1e
- sandbox_real.rs sha256: e0841adfcb9c86f8dc70e80b426ea4c745eb0a9df5ccd59a7a70212f40e9184b
- diagnostics.rs sha256: 18cea4aee9e041cfbade723658742c38d821999bf0a3a0e08eb8bc6be5a348cd

## Platform constraints
- `#![forbid(unsafe_code)]` in `security/src/lib.rs` blocks adding Landlock syscall wrappers to this crate
- Actual Landlock enforcement (raw `syscall()` wrappers) belongs in a separate crate or feature-gated module that CAN use `unsafe`
- `sandbox_real.rs` provides safe detection only; enforcement backend is documented for orchestrator
- Detection works via `/proc/version` + `/proc/filesystems` (pure safe Rust)

## Decisions
- Tests compiled via `#[path]` attributes (same pattern as clarity_guard.rs)
- `sandbox_real.rs` is standalone safe Rust; Landlock syscall wrappers documented for separate crate
- `diagnostics.rs` duplicates detection logic (standalone constraint: no external crate imports)
- main.rs minimal edit: replaced hardcoded string with `diagnostics::sandbox_backend_name()`
