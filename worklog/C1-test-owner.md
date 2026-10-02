# C1 independent test owner worklog

- Base: `be659402cbc99a4ffdedbfdc461ea754036a9f46`
- Candidate: `9ccfd85804789b90722b52a8cba7b98033d7996c`
- Scope: `crates/cli/tests/installed_default_entrypoint.rs` only (copied from `6dcabf83dbfb0e254b91ba4656562d91b3210d09`; no storage-trigger ancestry).
- Authority correction: pinned upstream `packages/tui/src/app.tsx:540-548` transitions empty-provider state to provider dialog; `packages/tui/src/component/dialog-provider.tsx:228-230` names it `Connect a provider`. Historical `provider setup` was superseded. Escape is required before `:q` in native input. Determinism masks only `, updated: ...`; transcript/assertion content stays exact.
- Compile gate: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test installed_default_entrypoint --no-run --locked --offline` — PASS.
- Focused gate: same target with `OC2_E2E_BIN=target/debug/oc2`, `OC2_E2E_REVISION=<candidate>` — 1/6 PASS, 5/6 RED. All five failures are before first native frame: executable emits `[offline] daemon unavailable ... start it manually ...`, then legacy `OpenCode RK` route. This is a product startup/daemon failure, not a test weakening.
- Frozen test SHA256: `sha256sum crates/cli/tests/installed_default_entrypoint.rs` to be recorded by verifier on this exact candidate.
- No daemon instrumentation was added; refusal proof is pending product-owned startup diagnosis.
