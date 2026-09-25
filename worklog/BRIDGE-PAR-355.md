# BRIDGE-PAR-355 (unclaimed, orchestrator owns ledger)

Claim: none taken per task (no claims.json touch). File-only lane.
Source: `packages/tui/.../prompt/move.tsx:18` usePromptMove (dialog-based session move; no cursor model in first 50 lines). Style: `crates/opentui-bridge/src/dialog_select.rs:1` forbid + docs + clamp.
Target: `crates/opentui-bridge/src/comp_prompt_move_full.rs` PromptMove {pos,len} + new + step(delta) clamp + pos. std-only, forbid(unsafe_code), <70 lines, >=3 tests.
Tests: 4 (zero, move, clamp both ends, empty). Verification: rustfmt --check only.
Status: file written, rustfmt pending.
