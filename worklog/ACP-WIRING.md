# ACP-WIRING worklog

Status: IMPLEMENTED / NOT ACCEPTED (verifier decides).

## Claim
Integrator wiring only: expose `acp_bridge` + `acp_files` via `lib.rs`.
No impl edits.

## Source evidence
- `crates/server/src/acp_bridge.rs` (ACP-001 codec + session map, untracked lane file).
- `crates/server/src/acp_files.rs` (ACP-002 broker traits, untracked lane file).
- Tests use `#[path]` include: `crates/server/tests/acp_bridge.rs:4-5`,
  `crates/server/tests/acp_files.rs:7-8`.
- Lane ownership: ACP-001/ACP-002 lanes own impl + tests; this lane owns
  `lib.rs` mod lines only. Did NOT edit `acp_files.rs` / `acp_bridge.rs` impl.

## Target boundary
`crates/server/src/lib.rs` lines 3-4:
`pub mod acp_bridge;` + `pub mod acp_files;` (additive, alphabetical).

## Tests
- `cargo check -p opencode-rk-server` => exit 0 (0 errors, pre-existing warnings only).
- `cargo test -p opencode-rk-server --test acp_bridge --test acp_files -- --test-threads=2`
  => acp_bridge: 5 passed, 0 failed; acp_files: 5 passed, 0 failed.
- JOBS=2 THREADS=2, timeout 120, scoped to server crate (8 GiB budget).

## Decisions
Alphabetical mod order. No `mod` rename, no Cargo.toml change, no impl touch.
Pre-existing working-tree lib.rs diff (WEB lanes) left intact.

## Remaining unknowns
None for wiring. Acceptance via verifier; DISC-003 reconciliation outstanding.
