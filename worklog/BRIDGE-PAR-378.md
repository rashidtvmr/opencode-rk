# BRIDGE-PAR-378 scratchpad (UNCLAIMED: orchestrator owns claims.json, file-only per task)

- Claim: skipped ledger (instructed NOT to touch tasks/completion/claims.json); status unclaimed.
- Source: packages/tui/src/context/event.ts:12-19 subscribe payload-kind flow; sibling crates/opentui-bridge/src/event_ctx.rs:1-68 bounded registry pattern.
- Target: crates/opentui-bridge/src/ctx_event_full.rs only; lib.rs/Cargo.toml untouched.
- Tests: emit_bumps_seq, kind_capped_64, default_empty_zero (in-file, 3 tests).
- Decisions: overlong kind truncates on char boundary (pop loop, no panic); seq via wrapping_add (no overflow panic); std-only, forbid(unsafe_code).
- Unknowns: none; wiring into lib.rs left to orchestrator.
