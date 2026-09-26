# BRIDGE-PAR-284 scratchpad

claim: BRIDGE-PAR-284 ses_par284 in-progress (ledger claim ok).
source: packages/tui/src/ui/link.tsx:1-34 (Link href/displayText/open on click).
sibling style: crates/opentui-bridge/src/home_footer_full.rs (forbid unsafe, 71L, in-file tests).
target: crates/opentui-bridge/src/link_ui_full.rs only. No lib.rs/Cargo.toml.
impl: link_label (strip http/https, chars take 256), is_url (http/https prefix), open_cmd (xdg-open+url cap 2). std-only, forbid(unsafe_code), 5 tests.
verify: rustfmt --check only.
