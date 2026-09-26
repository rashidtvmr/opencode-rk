# BRIDGE-GAP-51 scratchpad

Claim: BRIDGE-GAP-51, session ses_gap51, status in-progress.
Source evidence: crates/opentui-bridge/src/tui_utils.rs:1 (`#![forbid(unsafe_code)]` lane pattern).
Target boundary: ONE new file crates/opentui-bridge/src/run_runtime.rs only. No lib.rs, no Cargo.toml, no cargo run, no commit.
Tests: in-file #[cfg(test)] 6 tests (boot_starts_in_boot_phase, enqueue_rejects_past_cap, drain_clears_queue, stdin_modes_resolve, shutdown_is_terminal, enter_interactive_records_stdin).
Decisions: std-only Vec queue + mem::take drain; cap const PROMPT_QUEUE_CAP=64; tty wins in resolve_stdin. 158 lines (<220).
Unknowns: none. TS refs taken from task prompt (runtime.lifecycle.ts, runPromptQueue, resolveInteractiveStdin).
