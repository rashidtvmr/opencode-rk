# V2 integration evidence — 2026-09-30

## Integrated revisions

`0759e01464a7a0dbed614163a14b6f60673e2412`, pushed to `origin/main-v2`.
Native lifecycle package: `425d617a3576c791e7027b2cc332a75e7441fd4e`.
Integration writer: the main session in `/Users/mymac/Projects/opencode-rk-main-v2`.

## V2-SALVAGE-PRESERVATION

- Candidate: `3fd4b81`; branch base `fc2d201`, inventory context `0088fbf`.
- Independent verifier/test owner: Xkiro DeepSeek V4.1 Flash Free.
- Changed paths: `tools/convergence_v2_salvage.py`,
  `tests/bootstrap/test_convergence_v2_preservation.py`,
  `docs/convergence-v2-salvage.json`, `docs/CONVERGENCE_V2_SALVAGE.md`,
  `worklog/V2-SALVAGE-PRESERVATION.md`.
- Current-requirement authority: AGENTS.md non-destructive salvage and bounded
  resource policy; G0 in `docs/CONVERGENCE.md`. This is an rk preservation tool,
  not an upstream product-parity claim.
- Frozen test SHA-256:
  `9295af1d780b8865aca44b5b59518b32c14b07904cb0626742dd14d8a898a08b`.
- Compiling RED: all eight tests fail when loading the preserved `81dadbb` tool.
- Independent candidate GREEN: eight tests pass; source SHA-256
  `6098baac892ece3fdc5c87b692acd28193f5fe52c598d122c9f1534b6d3822d5`.
- Integrated command:

  ```text
  rtk env OC2_SALVAGE_TMP=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode /usr/bin/arch -arm64 /usr/bin/python3 -m unittest tests.bootstrap.test_convergence_v2_preservation tests.bootstrap.test_convergence_v2_backpressure tests.bootstrap.test_convergence_v2_controller
  ```

  Result: **19 passed**, including eight preservation and eleven controller tests.
- Inventory/preservation rerun on the exact integrated SHA:

  ```text
  rtk /usr/bin/arch -arm64 /usr/bin/python3 tools/convergence_v2_salvage.py --base-sha 0759e01464a7a0dbed614163a14b6f60673e2412 --base-ref main-v2 --frozen-root /Users/mymac/Projects/opencode-rk-pre-v2-20260930-1130 --preserve-dirty --snapshot-dir /Users/mymac/Projects/opencode-rk-pre-v2-20260930-1130/v2-worktree-preservation-20260930-integrated-0759e01 --json /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-preservation-integrated-0759e01.json --markdown /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-preservation-integrated-0759e01.md
  ```

  Result: 659 refs = 331 tips + 328 aliases; 45/45 detached anchors valid;
  112/112 worktrees present; 18 dirty including two staged-only; zero
  analysis/status failures. Fresh snapshot created without overwriting earlier
  snapshots. Full G0 remains pending: sensitive live `opencode.json` is left in
  place, and historical product/test intake still requires content disposition.
- State: **ACCEPTED for the preservation correctness package on `0759e01`**.

## V2-WEB-MECHANICAL-BUILD

- Candidate: `78f585c`, base `fc2d201`; cherry-picked as `0759e01`.
- Independent source review: Muse Spark; runtime preverification/integration:
  main session.
- Changed paths: `web/src/lib/canvas-model.test.ts`,
  `worklog/V2-WEB-MECHANICAL-BUILD.md`.
- Authority: independent mechanical unused-import maintenance in AGENTS.md.
  Removes five unused imports; test bodies independently compared byte-for-byte.
- Candidate and exact integrated revision both passed:

  ```text
  rtk /usr/bin/arch -arm64 pnpm --dir web run typecheck
  rtk /usr/bin/arch -arm64 pnpm --dir web exec vitest run src/lib/canvas-model.test.ts --maxWorkers=1
  rtk /usr/bin/arch -arm64 node --experimental-strip-types --test web/src/lib/api.auth.test.mjs web/src/lib/api.direct-auth.test.mjs web/src/lib/api.refresh-auth.test.mjs
  ```

  Results: typecheck GREEN, **16 canvas tests passed**, **six auth tests passed**.
- `git diff --check`: passed on the integrated tree.
- State: **ACCEPTED for the mechanical build package on `0759e01`**. The real
  browser tool/second-turn/reload journey G6 has not been run.

## V2-NATIVE-LIFECYCLE — exact integrated renderer acceptance

- Raw-input repair base: `3692743b0c2fd717efcabd7b491a4e48b7d46c1e`;
  original selective-salvage base: `fc2d201`.
- Candidate: `6959f88d1e94dc489a545e51390d00de56c7257b`, pushed on
  `v2/native-lifecycle-salvage`.
- Integration: known deltas `2391c50`, `3692743`, `6959f88` cherry-picked as
  `fb0a584`, `3197598`, `425d617`. All six package paths were independently
  compared byte-for-byte between candidate and exact integrated revision:
  `crates/opentui-bridge/src/safe_renderer.rs`,
  `crates/opentui-bridge/Cargo.toml`, `Cargo.lock`,
  `crates/opentui-bridge/tests/native_terminal_lifecycle.rs`,
  `crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib`,
  `worklog/V2-NATIVE-LIFECYCLE.md`.
- Authority/evidence: pinned OpenTUI `c01292fd0837bafd07ce458c74416b2b375a41ab`,
  `packages/core/src/renderer.ts` host raw-input lifecycle at 3592–3593,
  4295–4296, 4303–4304, 4470–4472; current G5 real-PTY contract.
- Final independently repaired PTY freeze SHA-256:
  `3799fb3444d0ac20f3b20c40404543c93bf85790aaa8060f053a8b678c011940`.
- Native dylib SHA-256:
  `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`.
- Exact final fixture produces four clean raw-input RED failures on preserved
  baseline `3692743`. Xkiro GPT-6 Luna independently reviewed and ran the committed
  candidate: all focused gates passed, tracked tree remained clean.
- Main reran these commands **serially on exact integrated `425d617`**:

  ```text
  rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 TUI015_NATIVE_LIB_DIR=/Users/mymac/Projects/opencode-rk-main-v2/crates/opentui-bridge/native/lib/aarch64-apple-darwin DYLD_LIBRARY_PATH=/Users/mymac/Projects/opencode-rk-main-v2/crates/opentui-bridge/native/lib/aarch64-apple-darwin cargo test --offline --locked -p opencode-rk-opentui-bridge --features native --test native_terminal_lifecycle -- --nocapture
  rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 DYLD_LIBRARY_PATH=/Users/mymac/Projects/opencode-rk-main-v2/crates/opentui-bridge/native/lib/aarch64-apple-darwin cargo test --offline --locked -p opencode-rk-opentui-bridge --features native --lib
  rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test --offline --locked -p opencode-rk-opentui-bridge --lib
  git diff --check
  ```

  Results: **10/10 PTY-target tests, 73/73 native library tests, 72/72 default
  library tests passed**; diff check passed. Five real PTY scenarios exercise raw
  input, explicit restoration, close/drop, unwinding, resize/closed handles,
  input protocol modes, suspend/resume and singleton reacquisition.
- State: **ACCEPTED for the native renderer lifecycle package on `425d617`**.
  Full release-built CLI/provider/session golden journey, G5 and G8 remain pending.

## Remaining observed product failures

- G2 persisted/in-app credentials are not connected to the outbound provider
  request; current turn execution reads environment credentials.
- G3/G4 typed tool persistence is not called by the live server, and second-turn
  history rejects Tool messages. The independent durability fixture is being
  frozen before the isolated source candidate is validated.
- Native renderer lifecycle is accepted as above; the complete native CLI golden
  journey still requires G1–G4 integration.
- Docker server is ready (`29.8.0`). Ubuntu 24.04 arm64 image acquired at digest
  `sha256:008173c23f95b170204355c12626cb5a965d779a7e1283b09e9cffbb1bf33ca3`.
  The frozen native installer closure test independently reproduces one failing
  test with three missing-library/atomicity assertions against `b323e2b`.
  Ubuntu packaging and its real native golden journey remain unrun.

These component acceptances are not G0–G8 release acceptance.
