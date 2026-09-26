# BRIDGE-PAR-406 scratchpad (UNCLAIMED: orchestrator owns claims.json, no ledger write per task)

- Claim: not claimed (per instruction, file-only; claims.json untouched).
- Source: `/home/rashid/projects/opencode/packages/tui/src/context/route.tsx:17-21` PluginRoute `{type:"plugin", id:string}`; `initialRoute` plugin branch lines 50-52.
- Boundary: ONE new file `crates/opentui-bridge/src/route_plugin_full.rs`; lib.rs/Cargo.toml untouched; no cargo/commit.
- Target: `PluginRoute {pub id: String cap 128}` + `set(&mut,&str)` + `id_of()->&str` + `is_set()->bool`; std-only; forbid unsafe; <60 lines (49).
- Tests: empty_by_default, set_roundtrip, set_caps_128, set_clears (4).
- Decisions: char-based 128 cap mirrors ctx_route_full.rs; `ponytail:` no data payload (TS `data?` out of scope, add when plugin-data lane needs it).
- Unknowns: none; wiring into lib.rs left to orchestrator.
