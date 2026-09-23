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

## Verify pass ses_f384fee27ffe1QKjki0oZQsHrQ @0a3ea6c
- Claim: PROV-020 in-progress, session ses_f384fee27ffe1QKjki0oZQsHrQ (no collision).
- HEAD 0a3ea6c. Frozen test sha256 8d74c4ff0429442e2b3bb714c2745f9f4e6c01c0c30cbafbd617fa14016b6896. Impl sha256 d2e8e34f407f7a346d6151324587e72109c9c92a4c6d23f2c08008d4f205477a (805 lines, zero diff, no edits).
- Focused suite GREEN first run, already GREEN: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-providers --test prov_020_auth_commands -- --test-threads=1` => 5 passed 0 failed (T01 connect+login consent-gated, grant applies expiry; T02 sorted list+inspect, unknown=>UnknownProvider; T03 refresh/logout/NotLoggedIn; T04 redaction sk-/source, empty=>EmptyProvider; T05 32-cap+truncated, byte-identical, disposable dir empty).
- No historical RED available in-tree (impl+tests landed together); recorded honestly, no RED fabricated.
- Static scans: 0 todo!/unimplemented!; no env/fs/net/process/thread/SystemTime/Command usage in owned file; forbid(unsafe_code); Error/Debug/serde static or safe_text-redacted; Import source hardcoded <redacted> in Debug+serde.
- Bounds: MAX_AUTH_PROVIDERS 128, provider 128B, label 256B, metadata 128B, list cap 32, caller-supplied now_ms only.
- lane_gate.py is storage-wave scoped (no PROV-020 lane) => not run, not guessed.
- Integration seam: `pub mod auth_commands` already at crates/providers/src/lib.rs:7 (integrator-owned, pre-existing); no caller wiring by this lane.
- Resource: single lightweight test target run; no broad builds.
