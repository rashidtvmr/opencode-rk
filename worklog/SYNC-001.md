# SYNC-001 worklog (verify-only, dedicated)

## Claim
`crates/server/src/sync_log.rs` (202 lines) satisfies `tasks/SYNC-001.md` (aggregate sync log). Frozen suite `crates/server/tests/sync_log.rs` 5/5 GREEN. Valid compiling-RED history (unresolved import pre-impl; bundle record). Verifier decides.

## Source evidence
- Base rev `248f519`. `tasks/SYNC-001.md:8-9` owns `sync_log.rs` only; lib.rs wiring integrator-owned.
- Impl: SyncEvent, SequencedEvent, SyncLog, SyncError, MAX_SYNC_EVENTS, MAX_PROJECTORS, append/register_projector/replay/freeze. Wired `server lib.rs:31`.
- Sibling split: SYNC-002 owns part-event classification; WEB-005 model mirrored.

## Observed scenario
New-module suite failed compiling RED on unresolved `sync_log` import; minimum native Rust impl → 5/5 GREEN (bundle `UI019-TOOL016-020-SYNC-RUN.md`).

## Target boundary
- Frozen test untracked, 5 #[test]; sha256 `a38c2e7256660b1819f22c972255dee0cf1a0a5e18c4d8871e2388f129f72a37` (bundle-frozen prefix `9384ffdc43892680`; full-hash drift noted, GREEN).
- Zero edits this lane (verify-only).

## Tests
- Rerun 2026-09-16: `cargo test -p opencode-rk-server --test sync_log` → 5 passed, 0 failed. Serial JOBS=2 THREADS=2 timeout 120.

## Decisions
- `replay` with `from_seq == len+1` on non-empty log → `Ok(0)`; beyond head → `BadCursor`; duplicates never re-emit.

## Remaining unknowns
- Acceptance verifier-owned. ralph.json `not-started` untouched.
