# APP-001 app-start lane (owned file: crates/cli/src/app_start.rs)

## Claim
Default-launch orchestrator types: TTY-gated `LaunchMode` (NativeTui/Headless/Error),
redirected-stdio headless path that never enters raw mode, RAII terminal-restore
guard, RAII pending-descriptor guard (no partial `backend.json` on failure).
Unwired module (integrator binds to main.rs); no TTY syscalls/FS/process control
inside — probes and cleanup are caller-provided.

## Source evidence (HEAD 5af7884)
- `crates/cli/src/main.rs:160-188` — no-subcommand arm calls `chat::run(&data)`;
  `Tui` arm calls `tui_entry::run(args)`. No TTY gate at dispatch (current code).
- `crates/cli/src/chat.rs:24,67-71` — line-based stdin loop; owned daemon child
  killed/waited on exit (lifecycle pattern the descriptor guard mirrors).
- `crates/cli/src/tui_entry.rs:11-16` — line-based stdio transport, no TTY
  detection; `--once` fails closed on dead origin (fail-closed precedent).
- `crates/server/src/daemon.rs:151-172` — atomic descriptor publish via
  temp file + rename (`backend.json.<pid>.tmp`); guard models the pending side.
- Card: `tasks/completion/local.json:4` (APP-001 tests: redirected stdio
  headless/error path never hangs in raw mode; startup failure restores
  terminal, leaves no partial descriptor/child).

## Observed scenario
`cargo check -p opencode-rk-cli` passes but module is not referenced by
`main.rs` (no `mod app_start`): compiles as part of crate only via
`rustc --test` standalone + metadata emit; wiring is integrator-owned per task.

## Target boundary
ONLY `crates/cli/src/app_start.rs`. No edits to main.rs, chat.rs,
tui_entry.rs, daemon.rs. `terminal_host.rs` in same dir is another lane's
file — untouched.

## Tests (#[cfg(test)] in owned file, 8 tests, frozen)
1. both_ttys_launch_native_tui
2. redirected_stdin_routes_headless_without_raw_mode
3. redirected_stdout_routes_headless_without_raw_mode
4. both_redirected_reports_both_without_raw_mode
5. unknown_probe_fails_closed_to_error_without_raw_mode (5 probe combos)
6. restore_guard_runs_on_drop
7. restore_guard_disarmed_runs_nothing
8. pending_descriptor_cleans_temp_on_drop_and_not_after_commit

## Decisions
- Probe inputs are `Option<bool>` (unknown fails closed to Error), not raw
  libc `isatty` — keeps module side-effect-free/testable; integrator supplies
  real probes. No new deps (libc not in workspace).
- `HeadlessReason` per-stream so the message tells the user which fd is
  redirected; exit code 2 distinct from success.
- Guards use `Box<dyn FnOnce() + Send>`; `commit(self)` consumes to prevent
  reuse after rename.
- `#![forbid(unsafe_code)]`, no `todo!/unimplemented!`, no statics/threads/FS.

## Remaining unknowns
- Integrator must add `mod app_start;` + dispatch gate in main.rs and wire
  real TTY probes (e.g. `std::io::IsTerminal`), headless command, real
  terminal raw-mode enter/restore + daemon spawn/kill cleanup.
- Missing-provider-credentials in-app setup test belongs to APP-005 slice,
  not this module.
