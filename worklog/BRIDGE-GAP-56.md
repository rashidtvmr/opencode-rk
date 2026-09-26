# BRIDGE-GAP-56 scratchpad

Claim: BRIDGE-GAP-56 via ses_gap56, scratchpad worklog/BRIDGE-GAP-56.md.
Source evidence: crates/opentui-bridge/src/plugin_host.rs:1 (forbid unsafe, bounded registries, PluginCommand/PluginStatus/RouteRevision naming); TS refs adapters.tsx:173 + command-shim.ts:85 (untrusted, behavior mirror only).
Observed scenario: new file only; lib.rs/Cargo.toml/plugin_host.rs untouched.
Target boundary: ONE file crates/opentui-bridge/src/plugin_adapters.rs, std-only, forbid(unsafe_code), under 180 lines, in-file #[cfg(test)] >=5 tests.
Tests: adapters_cap_32, empty_names_skipped, shim_ok, shim_empty_command_errs (+handler), adapter_version_zero.
Decisions: truncate names/commands/handlers to 64 (fail-closed length, mirrors plugin_host MAX bounds style); create_adapters version 0; camelCase aliases for TS parity.
Remaining unknowns: none; no cargo run per task (rustfmt check only).
