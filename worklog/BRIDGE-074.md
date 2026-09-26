# BRIDGE-074 win32_terminal scratchpad

Claim: plan/state for win32FlushInputBuffer + CtrlCGuard, no syscalls in lib.
Source: packages/tui/src/terminal-win32.ts:47 (FlushConsoleInputBuffer), :69-130 (guard: setRawMode hook + 100ms poll); crates/opentui-bridge/src/terminal.rs:48,64 (reuse win32_disable_processed_input, not redefined).
Target: crates/opentui-bridge/src/win32_terminal.rs only. lib.rs wiring left to orchestrator.
Tests: 7 (flush drains, flush idempotent, queue saturate, poll counts only when installed, install idempotent, enforce Unsupported off-windows, is_supported cfg). RED written first mentally; impl matches. No cargo run per lane scope (orchestrator gates).
Decisions: enforce() thin wrapper ties poll to real mode clear; saturating counters; const fns where possible.
Unknowns: host-side Windows syscall wiring deferred (needs windows-sys dep decision).
