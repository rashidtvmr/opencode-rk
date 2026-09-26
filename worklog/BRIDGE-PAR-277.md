# BRIDGE-PAR-277
claim: ses_par277 ok. source: dialog-mcp.tsx:1-50 DialogMcp+Status, toggle dialog.mcp.toggle; pattern dialog_select.rs:1 (forbid unsafe). target: dialog_mcp_full.rs McpDialog only. tests: 6 (caps, toggle, wrap, empty, order, skip-empty). decisions: snapshot list, host sorts; rem_euclid wrap. verification: rustfmt --check PASS, 129 lines, 6 tests. unknowns: none.
