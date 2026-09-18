# NET-010-rfile — remote file workflow validation

## Claim
`crates/server/src/remote_files.rs` implements pure validation for NET-010:
version-preconditioned edits, traversal/symlink/secret/unknown-workspace
denies without leakage, upload/download size+quota+cancellation caps with
cancel-cleanup ledger, artifact-uses-same-approval-path marker (remote
bypass denied), remote view version/conflict consistency.

## Source evidence (HEAD 5af7884)
- `crates/server/src/remote_sync.rs:1-21` — existing bounded-queue/cap idiom
  reused for cap constants (new requirement, no shared helper extracted;
  one-file lane).
- `crates/server/src/remote_ledger.rs:41-97` — pure in-memory ledger idiom
  reused for `apply_cancel` zeroing (new requirement).
- `crates/server/src/acp_files.rs:91-111` — `check_rel` traversal rules
  mirrored and extended (secret basenames, NUL, symlink flag, workspace
  allowlist); separate type so no shared-contract edit needed.
- `crates/server/src/lib.rs:28-29` — lane did NOT add `pub mod remote_files`;
  integrator pre-wires shared file per AGENTS.md lane gating.

## Observed scenario
- RED (permissive stubs, hash
  `3d3349176bacfe3984147288ef4a73bc0bc7c17edd0265f659da05bdea53bd5b`):
  `rustc --edition 2021 --test crates/server/src/remote_files.rs
  -o /tmp/opencode/rf && /tmp/opencode/rf` → 1 passed, 8 failed.
  Failing: traversal, symlink, unknown-workspace, version, over-cap,
  cancel, artifact, conflict-mismatch. Compiled, failed for missing
  behavior (not compile failure).
- GREEN iteration 1: 8 passed, 1 failed — empty-rel `contains("")`
  assertion vacuous (`""` is substring of every message incl. "empty
  path"). Fixed TEST (pre-freeze, RED suite not yet frozen) to assert
  deny + fixed message for `""`.
- GREEN final: 9 passed, 0 failed.

## Target boundary
- Owned file only: `crates/server/src/remote_files.rs` (408 lines).
  No other repo edits. `git status --short` shows only that file +
  this worklog (verify before submit).
- `#![forbid(unsafe_code)]`, std only (`std::fmt`, `std::error::Error`
  via full paths), no `std::fs`/`std::net`/threads/globals/statics,
  no FS IO (pure validation over caller-owned traits/buffers).
- Errors carry fixed variant names only; traversal test asserts denied
  messages contain neither the rejected rel nor `passwd`/`secret`.

## Tests (frozen GREEN suite, 9 tests in-file)
- T01 views: `allowed_view_preserves_version_conflict`,
  `view_conflict_flag_mismatch_rejected`.
- T02 denies: `traversal_denied_without_leakage`,
  `symlink_escape_denied`, `unknown_workspace_denied`.
- T03 edits: `concurrent_edit_needs_version` (None→VersionRequired,
  stale→Mismatch, exact→Ok(cur+1)).
- T04 transfers: `upload_over_cap_rejected` (TooLarge, QuotaExceeded,
  ok-at-boundary), `cancel_cleans_up` (Cancelled + held→0).
- T05 artifacts: `artifact_path_same_as_local` (SamePath→LocalFileEdit,
  Local→Local, Bypass→denied).

## Decisions
- Cancellation checked before size/quota: deterministic stop wins.
- Workspace allowlist checked before path: unknown id reveals nothing
  about path validity.
- `RemoteArtifactSamePath` normalizes to `LocalFileEdit` (marker type,
  no separate route); `RemoteBypass` denied.
- `check_view` contradiction → `Err("conflict flag mismatch")` fixed
  string, no versions echoed (version ints stay in caller struct).
- Saturating arithmetic on version bump and quota add (no overflow).

## Remaining unknowns
- Wiring into transport/permission broker is integrator's (lib.rs mod
  line, route approval plumbing) — out of lane scope.
- Secret-basename list (6 entries) is a floor; integrator may extend
  via discovery proposal.

## Frozen hash
- `db6a1437136e662ddf6dabcc402a5ecc6a2354128eebf2a79e36faa7eb0781a6`
  (`sha256sum crates/server/src/remote_files.rs`, 408 lines).
- Verify cmd: `rustc --edition 2021 --test
  crates/server/src/remote_files.rs -o /tmp/opencode/rf &&
  /tmp/opencode/rf` → 9 passed, 0 failed.
