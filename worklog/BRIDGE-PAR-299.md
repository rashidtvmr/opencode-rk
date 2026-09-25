# BRIDGE-PAR-299 scratchpad

Claim: BRIDGE-PAR-299 via cc.claim, session ses_par299. OK.
Source: packages/tui/src/util/tool-display.ts (13 lines): webSearchProviderLabel + toolDisplayMetadata (pending/structured rules).
Sibling: crates/opentui-bridge/src/tool_display.rs:1-89 (provider labels, truncate_title, ToolDisplay pending pair).
Target: ONE new file crates/opentui-bridge/src/tool_display_full.rs. No lib.rs/Cargo.toml edits.
API: tool_label(name)->String cap 64; tool_line(name,status)->String cap 256; is_running(status)->bool.
Decisions: trim+default "Tool"; char-based truncation; is_running matches running|pending|in_progress|in-progress|loading (case-insensitive); tool_line = "label [status]" or label alone.
Tests: 4 tests (label trim/default, label cap, running states, line combine/cap).
Verify: rustfmt --check only. No cargo, no commit.
