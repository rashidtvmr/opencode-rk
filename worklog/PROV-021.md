# PROV-021 worklog (verify-only, dedicated)

## Claim
`crates/providers/src/usage_status.rs` (310 lines) satisfies `tasks/PROV-021.md` (usage/limits telemetry). Frozen suite `prov_021_usage_status.rs` 5/5 GREEN. GREEN-on-first-run, no independent RED. Verifier decides.

## Source evidence
- Base rev `248f519`. `tasks/PROV-021.md:8-9` owns `usage_status.rs` only. Wired `lib.rs:56`.
- Recorded deviation (bundle): module requires provider pre-registration (`UnknownProvider` on unregistered record) while card text omits it; tests register first, isolate single remaining new key as `Overflow` at 256 entries. Saturating arithmetic at `u64::MAX`.

## Observed scenario
First run GREEN vs existing code. No valid RED.

## Target boundary
- Frozen test untracked, 5 #[test]; sha256 `5881c079922d647d0bc9990ec75ad20641dc1e52aecab4cfd6789cae3d169d4f` (bundle `4f883c3f...`; drift noted, GREEN).
- Zero edits.

## Tests
- Rerun 2026-09-16 batch 021–024 → 20 passed, 0 failed. Serial JOBS=2 THREADS=2 timeout 120.

## Decisions
- Verify-only. Deviation (pre-registration) kept as tested behavior, not weakened.

## Remaining unknowns
- Acceptance verifier-owned. ralph.json untouched.
