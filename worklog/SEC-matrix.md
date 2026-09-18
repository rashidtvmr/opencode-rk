# Worklog SEC-matrix (PAR-006 platform capability matrix types)

## Claim
Types-only per-OS capability matrix: fail-closed `UnsupportedSandbox`,
Linux/macOS/Windows rows as data, hook-cannot-grant marker, wildcard-proof
file authorization, deny-without-side-effect receipt. No OS sandbox claimed.

## Source evidence (HEAD 5af7884)
- `crates/security/src/sandbox.rs:1` — policy only, no Landlock syscalls.
- `docs/SECURITY.md` sections 1 (wildcard/hook rules), 3-4 (real backend +
  no-side-effect denial proof required).
- `docs/TDD.md` sections 3, 7 (denial asserts absence of side effects).
- PAR-006 card via `python3 tools/completion_plan.py --card PAR-006`:
  T01 wildcard-proof, T03 hooks cannot grant, T04 denial side-effect-free,
  T05 unsupported fails closed with honest limits.
- Sibling pattern: `crates/security/src/star_proof.rs:1`
  (`#![forbid(unsafe_code)]`), `crates/security/src/manual_only.rs`
  (destructive-instruction policy, no execution).

## Observed scenario
File `crates/security/src/platform_matrix.rs` did not exist (verified via
`ls` before creation). `crates/security/src/lib.rs` does not declare the
module; wiring left to integrator (owned-file-only lease, no other edits).

## Target boundary
OWNED FILE ONLY: `crates/security/src/platform_matrix.rs`. No `lib.rs`,
no Cargo manifest, no other crate edits. `std` only, `#![forbid(unsafe_code)]`,
no syscalls, no FS writes on deny path, no network, no wall-clock.

## Tests (in-file `#[cfg(test)]`, frozen at creation)
1. `unsupported_fails_closed` — `enforce()` errs for 3 OS x 2 backends,
   non-empty honest limit, `isolated == false` for all rows.
2. `wildcard_cannot_bypass_protected` — 7 protected paths deny under `"*"`;
   benign path allows under `"*"`, denies on non-matching pattern.
3. `hooks_cannot_grant` — policy deny + hook allow stays deny; hook refusal
   can only deny; allow+allow allows.
4. `denial_leaves_no_side_effect_marker` — denied write under disposable
   `temp_dir()/rk-pm-<pid>` leaves no file, receipt has
   `side_effect_performed=false`, `side_effect_marker=None`.
5. `matrix_covers_linux_macos_windows` — 3 rows, all `PlatformOptIn`,
   non-empty limits, `row_for` round-trips.

## Decisions
- `Backend::{None, PlatformOptIn}` both fail closed in `enforce()`; OptIn
  names the backend an integrator must supply, grants nothing today.
- `HOOK_CANNOT_GRANT_AUTHORITY: bool = true` + `debug_assert!` in
  `authorize_with_hook` so the invariant is greppable and enforced.
- `EnforcedSession` field-private; unconstructible outside module, never
  returned today.
- `is_protected_path` case-insensitive, covers `.env*` names, secret markers,
  system prefixes incl. Windows `c:/windows`, `c:/program files`.
- RED note: module file absent before creation = missing-behavior RED
  (compile-fail baseline); GREEN run below compiled the new file directly.
  No separate pre-impl RED suite hash frozen — recorded as deviation.

## Verification
- `rustc --edition 2021 --test crates/security/src/platform_matrix.rs
  -o /tmp/opencode/pm && /tmp/opencode/pm` → 5 passed, 0 failed.
- No `cargo build` run per lease. No `lib.rs` wiring (integrator owns).

## Remaining unknowns
- Integrator must add `pub mod platform_matrix;` to
  `crates/security/src/lib.rs` and run workspace gate.
- Real OS backend (Landlock/Seatbelt/Job-Object) + on-platform inherited-FD
  tests still missing; this module only types the matrix honestly.
