# BRG-ABI — build.rs provenance gate

## Claim
- Task: BRG-ABI, session ses_brg_abi, ledger claim OK (2026-09-23).
- Owned file ONLY: crates/opentui-bridge/build.rs (extend in place, keep non-native early-return).

## Source evidence
- crates/opentui-bridge/build.rs:1-66 — per-triple link gate, panics with `expected ...` string, NO provenance check; `env!("CARGO_MANIFEST_DIR")` inside `main()` only.
- crates/opentui-bridge/native/lib/: single entry `x86_64-unknown-linux-gnu/` containing only `libopentui.so` — no PROVENANCE file anywhere.
- `grep -r PROVENANCE crates/opentui-bridge/` → no hits (RED premise).

## Observed scenario (RED)
- Current build.rs has no `provenance`/`PROVENANCE`/`expected_artifact` symbols; artifact drift (wrong hash, rebuilt flags) links silently.
- RED check: `cp build.rs /tmp/opencode/build_check.rs && rustc --edition 2021 --test ... && ./brg_abi_test` on the CURRENT file → compiles, runs 0 tests (no harness), and `grep -c provenance` → 0. Provenance validation missing → fail.

## Target boundary
- Provenance contract: `native/lib/<triple>/PROVENANCE`, `key=value` lines: `zig_commit`, `build_flags`, `source_pin`, `sha256` (64 lowercase hex).
- build.rs validates it, fails closed (panic) listing `expected_artifact`.
- Constraints: keep non-native early-return; pure logic as fns; `#[cfg(not(test))]` on `main()`; harness via `rustc --test`; markers `provenance`, `PROVENANCE`, `expected_artifact`; ≥80 lines, ≥5 tests.

## Tests (frozen harness in build.rs `#[cfg(test)] mod tests`)
1. select static macos → Static
2. missing windows runtime → None
3. expected_artifact per os/env
4. parse_provenance ok
5. parse_provenance missing field errs
6. parse_provenance bad sha256 errs
7. drift_message contains triple + expected_artifact + reason
8. validate_provenance_file missing file fails closed naming expected_artifact

## Decisions
- `LinkKind { Static, Dylib }` + `select_link_kind` pure fn over bools; `expected_artifact` extracted from existing match.
- `parse_provenance(&str)` pure; `validate_provenance_file(&Path, expected_artifact)` does IO + maps every error to string containing expected_artifact (fail closed).
- `drift_message` pure formatter; `main()` panics with it on both link-gate miss and provenance error.

## Remaining unknowns
- None for lane. Upstream artifact rebuild/pinning out of scope; only gate + contract.

## Verification (GREEN)
- `cp crates/opentui-bridge/build.rs /tmp/opencode/build_check.rs && rustc --edition 2021 --test /tmp/opencode/build_check.rs -o /tmp/opencode/brg_abi_test && /tmp/opencode/brg_abi_test` → 8 passed, 0 failed.
- File: 264 lines (≥80); 8 tests (≥5); markers: provenance/PROVENANCE (11+4 hits), expected_artifact (11 hits).
- `cargo check --manifest-path crates/opentui-bridge/Cargo.toml` → build script compiles+runs (early-return warning printed); crate itself fails on PRE-EXISTING `src/input.rs:427` unterminated string, untouched by this lane (git status shows only build.rs modified).
