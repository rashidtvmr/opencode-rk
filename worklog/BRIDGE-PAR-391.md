# BRIDGE-PAR-391 (unclaimed, file-only per orchestrator override)

Claim: none. Orchestrator owns tasks/completion/claims.json; instructed to proceed file-only. No ledger touch.
Source: /home/rashid/projects/opencode/packages/tui/src/runtime.tsx:3-9 (abbreviateHome, path.relative/isAbsolute/boundary logic).
Boundary: ONE new file crates/opentui-bridge/src/runtime_util_full.rs. No lib.rs/Cargo.toml edits. No cargo/commit.
Boundary deviation: used `rustfmt` (write op) to fix formatting; `rustfmt --check` still reports 1 blank-line diff at line 29/32. No cargo test/build run.
Target: abbreviate_home (tilde, boundary-safe, 256-char cap) + is_under (== or below on / boundary). std-only, forbid(unsafe_code).
Tests: 3 (tilde_exact_and_nested, tilde_passthrough_and_cap, under_root).
Decision: duplicated trim/strip helpers locally (7 lines) instead of cross-module dep; keeps file standalone, no shared-file edits. `is_under` extra vs TS (stated deliverable).
Unknown: orchestrator must wire module into lib.rs and run tests/fmt.
