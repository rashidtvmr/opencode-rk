# BRIDGE-PAR-354 (unclaimed, file-only per spawn orders; ledger untouched)

Claim: `crates/opentui-bridge/src/comp_prompt_cwd_full.rs`. No ledger claim (orchestrator owns claims.json).
Source: TS truth `packages/tui/.../prompt/cwd.ts` EMPTY (0 bytes). Real logic: `runtime.tsx:3` `abbreviateHome`, `context/directory.ts:13`, `feature-plugins/sidebar/footer.tsx:22-30` (parent/name split on "/").
Observed: `workspace_label_full.rs:27-51` already implements tilde+cap+basename pattern; mirrored minimal.
Boundary: std-only, `forbid(unsafe_code)`, `cwd_label(cwd,home)->String` cap 128 + `cwd_base(cwd)->String` cap 64, no lib.rs/Cargo.toml edits.
Tests: 3 (`label_home_itself`, `label_sub_outside_cap`, `base_split_cap`).
Decisions: char-based `cap` (not byte); `norm` strips trailing `/\`; `~` only on exact/prefix match, empty home = passthrough.
Unknowns: none.
Verification: `rustfmt --check .../comp_prompt_cwd_full.rs` = FMT-OK (73 lines, under 80). No cargo per orders.
