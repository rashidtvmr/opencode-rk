# BRIDGE-GAP-111 scratchpad
claim: BRIDGE-GAP-111 via ses_gap111, scratchpad worklog/BRIDGE-GAP-111.md
source: crates/opentui-bridge/src/run_lifecycle.rs:1-126 (LifePhase, Lifecycle boot/ready/busy/idle/close); TS truth packages/opencode/src/cli/cmd/run/runtime.lifecycle.ts:1-120 (Lifecycle handle close/showExit, SIGINT exit path)
observed: Lifecycle.close() bool-once, no reason; need CloseReason kept for exit-splash/teardown labeling
target: APPEND only CloseReason/Lifecycle2/close_with/close_reason/reason_label + tests2; keep existing items byte-identical; std-only
tests: tests2 open_default, close_done_once, second_close_false, reason_kept, label_open/done/error/interrupted, error_cap_256
decisions: cap 256 chars (char-boundary safe); truncate inside close_with; Default+new() open
unknowns: none
