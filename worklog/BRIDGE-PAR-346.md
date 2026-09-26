# BRIDGE-PAR-346 scratchpad (UNCLAIMED - ledger overflow, orchestrator owns claims)

- Task: BRIDGE-PAR-346, one file `crates/opentui-bridge/src/builtins_config_full.rs`.
- No ledger claim made per delegation override (claims.json overflow; orchestrator owns it).
- Source evidence: `packages/tui/src/feature-plugins/builtins.ts:21-36` (`createBuiltinPlugins`, 12 entries); sibling style `crates/opentui-bridge/src/clip_line.rs:1-10` (`forbid(unsafe_code)`).
- Target: `Builtins { names: Vec<String> }` + `register(&mut self, &str) -> bool` + `has(&self, &str) -> bool` + `len(&self) -> usize`; cap 32, each 1..=64 bytes; std-only, `forbid(unsafe_code)`, <90 lines.
- Tests: 5 (register_and_has, rejects_duplicate, rejects_empty_and_long, enforces_cap_32, missing_is_absent).
- Decisions: byte-len check (ASCII ids); duplicate + cap reject false; added `is_empty` for `len` idiom; `Default` derived.
- Verification: `rustfmt --check` only (no cargo, no lib.rs/Cargo.toml edits, no commit).
