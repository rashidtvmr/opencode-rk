# PROV-020 worklog (verify-only, dedicated)

## Claim
`crates/providers/src/auth_commands.rs` (805 lines) satisfies `tasks/PROV-020.md` (auth connector commands/status). Frozen suite `prov_020_auth_commands.rs` 5/5 GREEN. GREEN-on-first-run, no independent RED. Verifier decides.

## Source evidence
- Base rev `248f519`. `tasks/PROV-020.md:8-9` owns `auth_commands.rs` only.
- Impl: AuthCommand, AuthEffect, AuthStatus, AuthCommandError, dispatch, list, inspect. Pure planner: no Command/thread/IO/network.
- Wired `lib.rs:7`.

## Observed scenario
Passed first run vs existing code. No valid RED (bundle `PROV-017-024.md`).

## Target boundary
- Frozen test untracked, 5 #[test]; sha256 `8d74c4ff0429442e2b3bb714c2745f9f4e6c01c0c30cbafbd617fa14016b6896` (bundle `9489303a...`; drift noted, GREEN).
- Zero edits.

## Tests
- Rerun batch 017–020 → 20 passed, 0 failed. Serial JOBS=2 THREADS=2 timeout 120.

## Decisions
- Verify-only.

## Remaining unknowns
- Acceptance verifier-owned. ralph.json untouched.
