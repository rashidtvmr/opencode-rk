# BRIDGE-PAR-236 scratchpad

claim: BRIDGE-PAR-236 via ses_par236 OK.
source: packages/tui/src/routes/session/sidebar.tsx:12-36 (Sidebar, Show when=session, width 42, overlay absolute/relative); crates/opentui-bridge/src/sidebar_screen.rs:37 (sidebar_lines, read-only).
target: ONE file crates/opentui-bridge/src/route_session_full.rs. SessionRoute{open,width} + toggle + set_width clamp 20..80 + render_hint cap 64. std-only, forbid(unsafe_code), <100 lines.
tests: default_closed_42, toggle_flips, clamp_bounds, hint_format_capped.
decisions: width default 42 mirrors TS; hint "open|closed:width".
evidence: rustfmt --check EXIT 0.
