# FIX-NATIVE-DAEMON scratchpad

Claim: FIX-NATIVE-DAEMON, session ses_fix_natdaemon, HEAD 62f7eb1.
Owned file: crates/cli/src/tui_entry.rs ONLY.

## Source evidence
- tui_entry.rs:498-499 `run` delegates to `run_with_dir(args, None)`.
- tui_entry.rs:505-508 `run_with_dir(args, data_dir: Option<&Path>)` — explicit dir honored, None falls back.
- tui_entry.rs:512 `resolve_origin_bearer(origin, data_dir)` — bearer from validated descriptor.
- tui_entry.rs:548-556 piped-stdin guard: `!stdin().is_terminal()` → Err "refusing interactive TUI on piped stdin: pass --once, --follow, or run on a TTY".
- tui_entry.rs:564-583 `resolve_origin_bearer`: explicit `data_dir` wins; else `resolve_cli_data_dir()`; origin must match descriptor.http_origin; token must be wellformed; else None (fail-closed).
- main.rs:278-285 `Command::Tui` resolves data dir, sets OPENCODE_RK_HOME env hack, calls `tui_entry::run`. Typed channel (run_with_dir with &data) NOT wired — out of authority (main.rs forbidden). tui_entry side complete; wiring left to integrator.
- main.rs:225 tty probe, app_start plan; chat.rs spawn/probe untouched per boundary.

## Decision
No code change needed in owned file: both SUCCESS (1) and (2) already implemented. Verify only, do not duplicate.

## Tests
- Pending: `cargo check -p opencode-rk-cli --bins` and/or tui integration test.

## Unknowns
- Whether verifier expects main.rs to call run_with_dir directly. Cannot do (forbidden path). Noted for orchestrator.
