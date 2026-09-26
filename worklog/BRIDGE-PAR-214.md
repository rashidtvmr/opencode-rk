# BRIDGE-PAR-214: run_command_full.rs
Claim: ledger in-progress, session ses_par214, scratchpad worklog/BRIDGE-PAR-214.md.
Source evidence:
- TS truth: packages/opencode/src/cli/cmd/run/footer.command.tsx:193-199 match(query,entries) fuzzysort filter; :354/469 query signal + filtered CommandEntry; confirm = highlighted row.
- Menu API: crates/opentui-bridge/src/footer_menu_full.rs:15-19 FooterMenuFull {items,cursor,open} items private; :73-78 selected()->Option<&str> gated on open.
Target boundary: ONE new file crates/opentui-bridge/src/run_command_full.rs. No lib.rs/Cargo.toml/footer_menu_full.rs/command_shim.rs edits.
Decisions:
- matches() filters selected row only (items private to sibling module, can't enumerate). Empty query trivially matches selected. Cap 32 holds (max 1 row).
- set_query truncates to 128 chars (mirrors MAX_ITEM_LEN). confirm() clones selected when open.
Tests: 7 unit tests in-file. Verify: rustfmt --check only (no cargo per task).
Unknowns: none. Wiring into lib.rs left to orchestrator (out of scope).
