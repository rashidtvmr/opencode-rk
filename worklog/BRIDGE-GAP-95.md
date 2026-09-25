# BRIDGE-GAP-95 scratchpad

claim: ses_gap95 owns BRIDGE-GAP-95, in-progress.
source: TS `packages/tui/src/routes/session/sidebar.tsx:12` Sidebar session overlay box; naming from `crates/opentui-bridge/src/sidebar.rs:19` Panel variants.
target: new file only `crates/opentui-bridge/src/route_sidebar.rs`, std-only, forbid unsafe, <140 lines.
tests: toggle_flips, select_opens, close_keeps_section, labels_non_empty, reselect_same_stays_open, default_closed_on_files.
decision: SideSection mirrors Panel order per task spec (Files, Context, Todo, Mcp, Lsp); label() const static str.
