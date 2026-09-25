# BRIDGE-PAR-194 scratchpad (ses_par194)

Claim: ledger `claim` ok (no output, exit 0).
Source evidence:
- crates/opentui-bridge/src/keymap_default.rs:11 `default_keymap`, :30 `describe`
- crates/cli/src/tui_entry.rs:552 `NativePage::Help` arm (7 help strings)
- crates/sessions/src/tui_state.rs:386 `keybinding_help`
Target boundary: ONE new file crates/opentui-bridge/src/help_screen.rs; no lib.rs/Cargo.toml/keymap_default.rs edits; no cargo/commit.
Pattern: cloned home_screen.rs frame idiom (clip char-safe, height.max(1), footer-last, pad/trunc).
Tests: 5 tests (title/exact-height, describe rows, footer-last+tiny, pad, width-clip).
Decisions: footer "Esc back"; body = "Help" + describe rows; std-only, forbid(unsafe_code).
Verification: `rustfmt --check` PASS; `wc -l` 85 (<110).
Unknowns: none. lib.rs wiring explicitly out of scope per task.
