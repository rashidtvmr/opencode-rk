# BRIDGE-GAP-77 scratchpad

claim: BRIDGE-GAP-77 via ses_gap77, worklog/BRIDGE-GAP-77.md
source: /home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/entry.body.ts (entryBody, textBody/codeBody/markdownBody none-on-empty, userBody `> ` prefix)
target: crates/opentui-bridge/src/run_entry_body.rs only (no lib.rs/Cargo.toml edits, no cargo run, no commit)
tests: 6 in-file cfg(test): user_ok_prefixes_marker, assistant_ok_passthrough, unknown_role_errs, empty_text_errs, preview_truncates_long, preview_short_untouched
decisions: std-only, forbid(unsafe_code), 95 lines; role cap 16, text cap 64KiB chars, preview 200+...; body_text errs on empty/unknown/overlong role; system passthrough like assistant
unknowns: none; standalone module unreferenced by lib.rs per scope
verify: rustfmt --check crates/opentui-bridge/src/run_entry_body.rs -> FMT_OK
