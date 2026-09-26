# BRIDGE-GAP-72 scratchpad

claim: BRIDGE-GAP-72 ses_gap72 in-progress
source: TS truth packages/opencode/src/cli/cmd/run/scrollback.shared.ts (entry syntax/look/color fns over StreamCommit+theme; no line store, port is fresh line contract)
target: crates/opentui-bridge/src/run_scrollback_shared.rs, std-only, forbid(unsafe_code), <170 lines
tests: push_evicts_oldest, sticky_filter, tail_n, tail_over_len, cap_truncates, push_returns_true
decisions: LINE_CAP 4KiB char-boundary truncate; CAP 2000 evict oldest via remove(0); push->bool always true; tail(n) saturating; sticky_lines clones texts
