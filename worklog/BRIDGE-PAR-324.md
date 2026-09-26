# BRIDGE-PAR-324 scratchpad

Claim: ExportDialog format picker, session ses_par324.
Source: packages/tui/src/ui/dialog-export-options.tsx:1-50 (DialogExportOptions solid store, tab-order option cycle, confirm/cancel).
Target: crates/opentui-bridge/src/dialog_export_full.rs, std-only, forbid(unsafe_code), <100 lines.
API: ExportDialog { formats: Vec<String> cap 8 each 32, cursor: usize } + push(&mut String,&str)->bool + move_cursor(delta:isize) + selected->Option<&str>.
Tests: 4 in-file (push-select, cap/reject, wrap, empty-none).
Decisions: wrapping cursor via rem_euclid; reject empty/>32/at-cap-8.
Unknowns: none.
