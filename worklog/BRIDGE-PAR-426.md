# BRIDGE-PAR-426 scratchpad (UNCLAIMED: orchestrator owns claims.json, no claim made)

- Claim: unclaimed per task orders; file-only work, ledger untouched.
- Source: `packages/tui/src/feature-plugins/system/plugins.tsx:9` id `internal:plugin-manager`; sibling `crates/opentui-bridge/src/system_plugins.rs:18-24` already defines that id.
- Observed: no `sys_plugins_full.rs` existed; `system_plugins.rs` covers notifications/which-key/tips/footer, not a bounded name registry.
- Target boundary: ONE new file `crates/opentui-bridge/src/sys_plugins_full.rs`; lib.rs, Cargo.toml, plugin_system_full.rs untouched.
- Tests: 3 unit tests in-file (register_has_len, rejects_bad_names, caps_at_32).
- Decisions: std-only, forbid(unsafe_code), dup register idempotent Ok, byte-len bound 64, cap 32; 78 lines.
- Unknowns: whether orchestrator will wire module in lib.rs (out of scope).
