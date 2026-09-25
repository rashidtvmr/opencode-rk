# BRIDGE-PAR-164 - footer_menu_full.rs

Claim: BRIDGE-PAR-164 via ses_par164. Owned file: crates/opentui-bridge/src/footer_menu_full.rs.

Source evidence:
- TS truth: packages/opencode/src/cli/cmd/run/footer.menu.tsx:57-74 (createFooterMenuState selected/offset, reveal clamp, reset).
- Sibling pattern: crates/opentui-bridge/src/run_footer_menu.rs:34-97 (FooterMenu capped items, wrapping cursor via rem_euclid, open/close gate, chosen None when closed).

Observed scenario: sibling FooterMenu owns label/action rows (cap 16x64); this lane needs plain string list variant.

Target boundary: ONE new file footer_menu_full.rs. No lib.rs/Cargo.toml/run_footer_menu.rs edits. No cargo.

Tests: 6 in-file #[cfg(test)] (open flag, open empty, close gates, wrap both ways, selected none, cap 32 + truncate 128).

Decisions: close_menu keeps items (reopenable) unlike sibling clear; ponytail: no offset/scroll window, add when limit>visible rows.

Unknowns: none.
