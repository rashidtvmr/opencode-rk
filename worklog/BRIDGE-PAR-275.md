# BRIDGE-PAR-275 scratchpad

Claim: BRIDGE-PAR-275 claimed ses_par275. File `crates/opentui-bridge/src/dialog_theme_list_full.rs`.
Source evidence:
- TS truth `packages/tui/src/component/dialog-theme-list.tsx:6` DialogThemeList, sorted options :9, live preview onMove :28, confirm onSelect :31, revert initial onCleanup :19-21.
- `crates/opentui-bridge/src/theme_picker.rs:7` ThemePicker {engine, index}, next :32, prev :39, current :29, apply_known :46.
- Style ref `dialog_message_full.rs` forbid unsafe, trunc bounds, open/close.
Target boundary: new file only, std-only, forbid unsafe, <110 lines, no lib.rs/Cargo.toml, no cargo/commit.
Tests: 5 in-file (new_cursor_matches, move_forward, move_backward_wrap, apply_confirms, locked_fails).
Decisions: cursor mirrors picker.index after every mutation; apply_current re-applies current name (locked -> false); with_picker ctor for host injection.
Verification: rustfmt --check FMT_OK, 109 lines.
