# BRIDGE-075 dialog_select

Claim: filterable select state + export format.
Source: packages/tui/src/ui/dialog-select.tsx:80 (generic DialogSelect, ref :486-498, fuzzysort :165-170, move wrap :290-297); dialog-export-options.tsx:24; routes/session/index.tsx:956 (`.md` default); tips-view.tsx:185 (Markdown only).
Target: crates/opentui-bridge/src/dialog_select.rs only. Reuses crate::dialog, no redefine. No cargo run (scope ban).
Tests: 10 (item bounds, overflow 256, empty stable, case-insensitive, hint, non-fuzzy, wrap+empty noop, clamp+long query, move_to_value, export pick).
Decisions: String T-erasure documented; substring filter, fuzzysort divergence noted; single-variant Markdown format; hint=description.
Unknowns: none.
