# BRIDGE-PAR-363 scratchpad (unclaimed: orchestrator owns claims.json, no claim made)

- Task: new file `crates/opentui-bridge/src/route_sess_subfoot_full.rs`
- TS truth: `/home/rashid/projects/opencode/packages/tui/src/routes/session/subagent-footer.tsx:17-31` (label from `@(\w+) subagent` titlecase else "Subagent"; index/total from siblings with same parentID)
- Evidence: `crates/opentui-bridge/src/subagent_footer.rs:46-53` (SubagentFooter buffer pattern), `subagent_footer_full.rs:35-43` (capped summary pattern)
- Target: `SessSubfoot { label: String cap 128, active: u32 }` + `set_label` + `bump` + `line cap 256`, std-only, forbid(unsafe_code), <80 lines, >=3 tests
- Decision: minimal struct, char-wise trunc, saturating bump, `line()` = `"{label} ({active})"`
- Status: file-only, lib.rs/Cargo.toml untouched, no cargo, no commit
- Verify: `rustfmt --check crates/opentui-bridge/src/route_sess_subfoot_full.rs` PASS, 73 lines (<80)
