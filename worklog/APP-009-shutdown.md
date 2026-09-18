# APP-009-shutdown

Task: APP-009 slice — shutdown/terminal-restore types.
Owned file: `crates/cli/src/shutdown.rs` only. No other edits made.

## Claim
RAII terminal-restore guard + shutdown taxonomy, std-only, no syscalls.

## Source evidence
- HEAD at start: `5af7884` (per lease; `git status` showed diverging local
  modifications incl. `docs/ADAPTER_PROTOCOL.md` + untracked files — untouched).
- APP-009 card (`tasks/completion/local.json:12`): service controls/shutdown/
  terminal restoration; SIGINT/SIGTERM + recoverable panics restore
  raw/alternate-screen/mouse state; suspend/resume keeps input usable.
- No pre-existing `crates/cli/src/shutdown.rs`; lease creates it.
- `crates/cli/Cargo.toml`: package `opencode-rk-cli`.

## Observed scenario (TDD RED then GREEN)
- RED: `Drop` impl empty. `rustc --edition 2021 --test ... -o /tmp/opencode/sd`:
  4 passed, 2 failed (`drop_restores`, `panic_path_restores_on_unwind`).
  Hash at RED: `74b65d4...`.
- GREEN: implemented `Drop` running `restore.take()` once + full API.
  Result: `6 passed; 0 failed`. Hash at GREEN: `2c1e06a4...`.

## Target boundary
- Types: `ShutdownReason` (Sigint/Sigterm/Panic/Explicit) with
  `is_signal`/`as_str`/`Display`; `RestoreReceipt` (reason/restored/suspends);
  `RestoreGuard<F: FnOnce()>` (new/commit/disarm/suspend/is_armed/reason);
  `SuspendMarker` (suspend/resume/is_suspended/count).
- Caller supplies all side effects; module does no syscalls, installs no
  handlers, owns no FDs/processes. `#![forbid(unsafe_code)]`, std only.
- Panic path (documented in module docs + `Drop` comment): restores during
  unwinding; cannot run on panic=abort/SIGKILL/power loss (userspace ceiling);
  restore closure must not panic (Drop-panic aborts).

## Tests
6 tests in-file: `drop_restores`, `disarm_suppresses_double`,
`commit_suppresses_drop_restore`, `panic_path_restores_on_unwind`,
`suspend_marker_tracks_state`, `reason_classification`.
Frozen RED tests never edited to pass — only the `Drop` impl changed RED→GREEN.

## Decisions
- `FnOnce` closure, no trait objects: zero-cost, forbid-unsafe clean.
- `commit`/`disarm` both return `suppressed` receipt (aliases; disarm reads
  better in guard contexts).
- `suspends` saturating counter, no timestamps (no wall-clock dependence).

## Remaining unknowns
- Integration: caller wiring (signal handler → reason, TUI teardown closure,
  `mod shutdown` declaration in crate root) belongs to APP-009 service-commands
  slice / integrator, outside this leased file.
- Gate `tools/lane_gate.py` has no APP-009 lane; verification per lease command.
