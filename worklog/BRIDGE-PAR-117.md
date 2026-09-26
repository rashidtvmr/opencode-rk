# BRIDGE-PAR-117 scratchpad

Claim: BRIDGE-PAR-117 via ses_par117. Owned file: crates/opentui-bridge/src/home_route.rs.
Source evidence:
- packages/tui/src/routes/home.tsx:22 (`Home`, prompt-first layout, slots home_logo/home_prompt/home_bottom/home_footer)
- packages/tui/src/runtime.tsx:3 (`abbreviateHome`: relative to home, `~` idiom)
- crates/opentui-bridge/src/home_destination.rs:8 (MAX_ID_LEN 64 cap idiom, trunc on char boundary)
- crates/opentui-bridge/src/tui_runtime.rs:4 (abbreviateHome mirror note)
Target boundary: HomeRoute struct + set_cwd/add_recent/select_dest/title. std-only, forbid(unsafe_code), <170 lines.
Tests: 6 in-file (empty cwd, cwd trunc, dup front, evict, dest select/clear, title tilde).
Decisions: trunc helper char-boundary like home_destination; abbrev uses $HOME prefix `~/`; new("") falls back ".".
Verification: rustfmt --check PASS (FMT_OK), 157 lines. No cargo per scope.
