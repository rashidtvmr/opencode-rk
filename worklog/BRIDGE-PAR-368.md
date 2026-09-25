# BRIDGE-PAR-368 scratchpad (unclaimed: orchestrator owns claims.json, file-only lane)

- Claim: NOT claimed (per delegation: do not touch tasks/completion/claims.json).
- Task: one new file crates/opentui-bridge/src/ctx_runtime_full.rs, no lib.rs/Cargo.toml edits.
- Source evidence: packages/tui/src/context/runtime.tsx:21-32 provider pattern (explicit frozen state, no globals); crates/opentui-bridge/src/runtime_ctx.rs:1-55 style (forbid unsafe, explicit state, caps via reject/truncate).
- Target boundary: CtxRuntime { model cap 128, busy } + set_model/set_busy/status (cap 256). std-only, forbid(unsafe_code), <80 lines, >=3 tests.
- Tests: default_idle, model_caps, busy_status_caps (in-file #[cfg(test)]).
- Decisions: chars() caps (unicode-safe); status `"<model>:busy|idle"` truncated to 256; Default via new().
- Unknowns: none. Verification: rustfmt --check only (no cargo per scope).
