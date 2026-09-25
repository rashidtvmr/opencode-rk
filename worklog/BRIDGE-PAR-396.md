# BRIDGE-PAR-396 (unclaimed, file-only per spawn orders)

- Task: crates/opentui-bridge/src/plugin_api_full.rs, no ledger claim, no lib.rs/Cargo.toml edits.
- TS truth: packages/tui/src/plugin/api.ts:1-40 `createPluginRoutes` (register/get/revision), :42 `createTuiApi` identity+lifecycle. Port: minimal handle only.
- Boundary: PluginApi {name cap 64, calls u32} + call/name_of/calls. std-only, forbid(unsafe_code).
- Tests: 4 (init, increment, 64-cap, saturate).
- Decisions: ponytail - skipped route map/dispose signal; add when plugin host wires routes.
- Unknowns: none.
