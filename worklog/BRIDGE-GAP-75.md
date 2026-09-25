# BRIDGE-GAP-75 scratchpad

claim: BRIDGE-GAP-75 ses_gap75 worklog/BRIDGE-GAP-75.md (in-progress)
source: runtime.stdin.ts:15-37 resolveInteractiveStdin (isTTY wins else /dev/tty)
source: crates/opentui-bridge/src/run_runtime.rs:17-22 StdinMode, :46-54 resolve_stdin
observed: run_runtime.rs owns StdinMode+resolve_stdin; companion needed, no redefine
target: crates/opentui-bridge/src/run_runtime_stdin.rs only; no lib.rs/Cargo/run_runtime edits; no cargo; rustfmt --check only
tests: tty_wins_over_bytes, piped_on_bytes, closed_when_empty, probe_zero_is_closed, label_tty, label_piped, label_closed (7)
decisions: reuse crate::run_runtime::{StdinMode,resolve_stdin}; probe maps pending_bytes>0 to has_bytes; std-only forbid(unsafe_code)
unknowns: none; lib.rs wiring left to orchestrator
