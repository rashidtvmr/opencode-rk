# BRIDGE-PAR-424 (unclaimed, file-only)

- Status: unclaimed (orchestrator owns claims.json; did not claim).
- Task: create `crates/opentui-bridge/src/side_footer_full.rs`, no lib.rs/Cargo.toml edits.
- Truth: `footer.tsx:19-30` splits dir text on `/` into parent/name.
- Design: `SideFoot { parent, name }`, `set` caps 128 chars each, `line` joins with `/` capped 256 chars.
- Tests: joins_parent_name, empty_parent_returns_name, caps_fields_and_line.
