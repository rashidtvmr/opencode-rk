# BRIDGE-PAR-387 (unclaimed, file-only per orchestrator)

Claim: none (orchestrator owns claims.json). File-only lane.
Evidence: TS packages/tui/src/context/thinking.ts:4 (show|hide), :24-27 (show->hide->show cycle), :36 (default hide). Sibling thinking_ctx.rs:1-119 (u8 level mirror, skipped reasoningSummary).
Scope: ONE new file crates/opentui-bridge/src/ctx_thinking_full.rs. No lib.rs/Cargo.toml edits.
Target: Thinking {on: bool, depth: u32} + enable + step (bump when on) + is_on.
Tests: 3 (new_is_off_zero, step_bumps_only_when_on, enable_is_idempotent).
Decisions: Default off/0 mirrors TS default hide. saturating_add for depth bound (ponytail: no disable/reset, add when TS gains toggle-off).
Status: done file-only, rustfmt --check PASS, 59 lines (<70). No cargo run per scope.
