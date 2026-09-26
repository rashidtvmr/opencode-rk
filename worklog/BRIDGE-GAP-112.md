# BRIDGE-GAP-112 — tui wiring (stdin probe + session footer)

Claim: BRIDGE-GAP-112, session ses_gap112 (orchestrator-held; subagent ses_f27b7428fffet716KLb9r7nD0b hit step cap before ledger flip + scratchpad write).
Source evidence:
- TS truth: /home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/runtime.stdin.ts (resolveInteractiveStdin: tty wins else /dev/tty else throw).
- Bridge API: crates/opentui-bridge/src/run_runtime_stdin.rs (StdinProbe {is_tty, pending_bytes} + new/closed, resolve_probe, probe_label -> "tty"/"piped"/"closed").
- Target: crates/cli/src/tui_entry.rs native_page_lines (~464), native_interactive_loop (~590).
Observed scenario: native loop had no stdin-mode awareness; offline banner generic; Chat page had no session status footer.
Target boundary: two body-only extensions, zero signature changes, no new files/tests.
Tests: none in caller file (no test mod; not added). Verification: rustfmt --check shows only 2 pre-existing hunks (lines 15, 22, HEAD-baseline confirmed); cargo check -p opencode-rk-cli --features native green (E0308 fixed: &'static str literal -> .to_string()).
Decisions: reuse bridge stdin adapter (no duplication); footer id truncated to 8 chars (matches route_session title idiom); offline title + transcript seed both carry stdin mode.
Remaining: flip ledger completed via orchestrator (subagent session expired); this scratchpad written by orchestrator to close the gap.
