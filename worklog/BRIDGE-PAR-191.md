# BRIDGE-PAR-191 scratchpad

Claim: BRIDGE-PAR-191, session ses_par191, 2026-09-25.
Evidence:
- crates/opentui-bridge/src/sidebar_panels.rs:1-80 (SidebarPanels, panel_label, SidePanel2, summary, count_of) read-only truth.
- packages/tui/src/routes/session/sidebar.tsx:1-103 (title/content/footer slots, width 42 box).
- crates/opentui-bridge/src/route_sidebar.rs:1-70 (SideSection labels, open flag; row content lives in crate::sidebar).
- Pattern: page_adapter.rs clip/pad/truncate-to-height; text.rs clip_chars.
Target: NEW file crates/opentui-bridge/src/sidebar_screen.rs only. sidebar_lines(panels,width,height)->Vec<String>: summary first + 5 "label: N" rows, width char-clip + space-pad, height truncate/pad. std-only, forbid(unsafe_code).
Tests: 5 (order/content, width-clip, height-trunc, zero-dims, missing-zero).
Decisions: char-count clip (matches text.rs clip_chars); ponytail: no unicode-width.
Verification: rustfmt --check PASS (ran rustfmt fix + re-check FMT_OK). No cargo per task scope. 112 lines < 120.
Unknowns: none.
