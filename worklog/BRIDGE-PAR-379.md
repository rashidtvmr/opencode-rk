# BRIDGE-PAR-379 scratchpad (UNCLAIMED - file-only, orchestrator owns ledger)

- Claim: skipped per task instruction (do NOT touch claims.json).
- Source: `packages/tui/src/context/exit.tsx:1-8` `Exit=(reason?: unknown)=>void`; prior art `crates/opentui-bridge/src/exit_ctx.rs` (richer ExitCtx w/ reason string), `ctx_theme_full.rs` style template.
- Boundary: new file only `crates/opentui-bridge/src/ctx_exit_full.rs`; no lib.rs/Cargo.toml edits; no cargo; no commit.
- Design: state machine Default->Asked->Taken; `ExitReq{code,asked}`, `ask` sets both, `take` one-shot resets asked, `pending` reads asked. std-only, forbid(unsafe_code).
- Tests: default_not_pending, ask_pends_take_once, reask_after_take (3 tests, inline).
- Verify: `rustfmt --check crates/opentui-bridge/src/ctx_exit_full.rs` PASS (exit 0), 63 lines < 70.
- Unknowns: none. Integration (mod wiring) left to orchestrator.
