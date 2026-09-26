# BRIDGE-PAR-192 palette_screen.rs

Claim: ledger in-progress ses_par192.
Source evidence:
- crates/opentui-bridge/src/footer_menu_full.rs:15 FooterMenuFull (items private, read via selected()/len()/cursor())
- packages/opencode/src/cli/cmd/run/footer.command.tsx:1-60 palette panel, fuzzysort-ranked, cursor highlight
- crates/opentui-bridge/src/dialog_select.rs filter() substring precedent, :104-118
- crates/opentui-bridge/src/run_footer_menu.rs cursor wrap precedent
Target boundary: ONE new file crates/opentui-bridge/src/palette_screen.rs. No lib.rs/Cargo.toml edits. No cargo per scope.
Tests: 7 in-file (title pad, prefix mark, closed placeholder, empty placeholder, width-clip/height-cap/zero-guard, filter case-insensitive+empty+miss, filter closed empty).
Decisions:
- palette_lines: title row + single highlighted row (items field private, only selected() readable). Divergence noted in docs.
- filter_items: substring over highlighted row only, cap 32. Same privacy constraint.
- fit() char-based pad/trunc, std-only, forbid(unsafe_code), 116 lines.
Remaining: lib.rs prewire (pub mod palette_screen) left to integrator. Tests written, unrun per scope (no cargo allowed).
Verification: rustfmt --check EXIT 0, wc 116 lines.
