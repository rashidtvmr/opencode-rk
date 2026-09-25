# BRIDGE-GAP-47 scratchpad
- claim: BRIDGE-GAP-47 ses_gap47 worklog/BRIDGE-GAP-47.md OK
- evidence: crates/opentui-bridge/src/solid_p0.rs (Signal/Memo/Effect P0 core, Rc<RefCell>, forbid unsafe)
- target: crates/opentui-bridge/src/solid_for_show.rs only; no edits to lib.rs/Cargo.toml/solid_host.rs/solid_p0.rs
- tests: in-file #[cfg(test)] 8 tests (for render order, empty, update changed/length/no-change, show some/none/toggle)
- decisions: index-wise PartialEq diff only, no keyed moves; std-only; forbid(unsafe_code); 147 lines
- verification: rustfmt --check PASS
