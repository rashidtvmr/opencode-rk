# BRIDGE-PAR-365 (unclaimed, file-only, no ledger touch per task override)

- Claim: SKIPPED per orchestrator instruction (do NOT touch tasks/completion/claims.json); proceeding file-only.
- Source evidence:
  - `/home/rashid/projects/opencode/packages/tui/src/context/data.tsx:37-48` (Data record stores, flat get/set pattern via solid store)
  - `/home/rashid/projects/opencode-rk/crates/opentui-bridge/src/kv_ctx.rs:1-69` (sibling bounded KV, bool fail-closed pattern reused)
  - `/home/rashid/projects/opencode-rk/crates/opentui-bridge/src/context_kv.rs:51-69` (richer Result API sibling, not reused)
- Observed: no `ctx_data_full.rs` in `crates/opentui-bridge/src/` (dir listing).
- Target boundary: ONE new file `crates/opentui-bridge/src/ctx_data_full.rs`. No lib.rs / Cargo.toml edits. No cargo. No commit.
- Tests: 4 unit tests in-file (roundtrip, overwrite, caps, full).
- Decisions: MAX_PAIRS=64, single MAX_LEN=512 for both key/value (spec "cap 64 each 512"); bool fail-closed like kv_ctx; update-in-place allowed when full; std-only; forbid(unsafe_code); 91 lines.
- Unknowns: none. lib.rs wiring left to orchestrator.
