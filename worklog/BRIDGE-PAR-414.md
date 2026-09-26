# BRIDGE-PAR-414 (unclaimed, file-only per orchestrator)

Claim: skipped ledger (orchestrator owns claims.json). Proceeded file-only.
Source: bg-pulse-render.ts:1-50 (PERIOD loop), crate::bg_pulse (frame wrap, forbid unsafe).
Target: crates/opentui-bridge/src/bg_render_full.rs only. No lib.rs/Cargo.toml edits.
Tests: 3 unit tests (init zero, wrap at max, zero-max stays zero).
Verify: `rustfmt --check` PASS, 57 lines (<70), std-only, forbid(unsafe_code).
Unknowns: wiring into lib.rs left to orchestrator.
