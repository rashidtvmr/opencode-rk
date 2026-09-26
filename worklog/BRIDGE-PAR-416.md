# BRIDGE-PAR-416 (unclaimed, file-only, orchestrator owns ledger)

- Claim: skipped per prompt (no claims.json touch). Status: unclaimed.
- Source: `packages/tui/src/feature-plugins/system/notifications.ts:9-18` notify() fan-out; repo `crates/opentui-bridge/src/toast_single.rs` style (forbid unsafe, ponytail note, std-only).
- Boundary: ONE new file `crates/opentui-bridge/src/notif_sys_full.rs`. No lib.rs/Cargo.toml edit, no cargo, no commit.
- Impl: `NotifSys { items: Vec<String> }`, push (drop oldest at 32, chars().take(256)), clear, len + is_empty. 70 lines (<80).
- Tests: 3 (`push_and_len`, `truncates_to_256_chars`, `caps_at_32_and_clears`) inline `#[cfg(test)]`, std-only.
- Verify: `rustfmt --check crates/opentui-bridge/src/notif_sys_full.rs` exit 0. No cargo per scope.
- Unknowns: module wiring (lib.rs) left to orchestrator.
