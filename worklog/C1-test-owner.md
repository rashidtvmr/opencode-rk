# C1 independent test owner worklog

- Base: `e80ce6802f09c7e909a5d6123a5910ab14e2488f` (preserved)
- Candidate: `7130b03ed1e355d4661bb1eb4f9b1c6392ee664b`
- Scope: `crates/cli/tests/installed_default_entrypoint.rs` only (mechanical correction; worklog updated afterward).
- Authority correction: pinned upstream `packages/tui/src/app.tsx:540-548` transitions empty-provider state to provider dialog; `packages/tui/src/component/dialog-provider.tsx:228-230` names it `Connect a provider`. Historical `provider setup` was superseded. Escape is required before `:q` in native input. Determinism masks only `, updated: ...`; transcript/assertion content stays exact.
- Compile gate: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test installed_default_entrypoint --no-run --locked --offline` — PASS.
- Correction: `Root::new` now allocates an exclusive six-character suffix under the approved temp directory, uses bounded `create_dir`, and asserts socket path bytes including NUL are <=104. The obsolete `ctrl-p` route-marker assertion now checks the native frame marker; the renderer test retains the separate alternate-screen contract.
- Native build: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo build --release --features native --bin oc2 --locked --offline` — PASS (cached; no whole-worktree rebuild). Artifact SHA256 `3e546ac82ebe24d4665c58bafe47f60b793b69afb29ada8de63d98fd3faf9366`.
- Focused gate: `OC2_E2E_BIN=target/release/oc2 OC2_E2E_REVISION=7130b03... cargo test ... --offline -- --nocapture` — **1/6 PASS, 5/6 RED**, 60.13s. All five fail at `wait_for` before native frame (line 249): product daemon/startup remains unavailable; no fixture retry or product assertion weakening.
- Frozen test SHA256 on candidate: `29bf97e9fdcbc73c23e52866938bb04f044612acb60816b8c8e2fbfa9f4e2459`.
- Direct-serve source attestation: native artifact `serve --listen 127.0.0.1:0` with minimal environment was attempted for long and short roots. Both exited 1 with `daemon io: path must be shorter than SUN_LEN`; no descriptor/health was published. This confirms the path precondition, while the test candidate now stays within the asserted bound (production's own socket suffix may still exceed it).
- N6 faithful control (fresh `/private/var/.../C1-native-audit/n6_fresh_c1_rerun.py`, output `n6-c1-faithful-rerun.jsonl`): A=3/3 exit0, 0 bytes before post-reap read, EOF; B=3/3 full 24-byte payload while child alive, then exit0/EOF; C=3/3 exit0, 24-byte payload post-reap, EOF. Each row contains PID/TTY/write count and payload hex. The original audit was not overwritten.
