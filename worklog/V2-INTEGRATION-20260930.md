# V2 integration evidence — 2026-09-30

## Integrated revisions

`0759e01464a7a0dbed614163a14b6f60673e2412`, pushed to `origin/main-v2`.
Native lifecycle package: `425d617a3576c791e7027b2cc332a75e7441fd4e`.
Typed live writer/restart package: `31ea2b0d39e4cf23e8eae5f894763edf88678cb9`.
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

## V2-TYPED-HISTORY — exact integrated live writer/restart acceptance

- Gate/package: G3/G4 typed call/result admission, continuation, second turn,
  and settled-history restart replay.
- Product base: `fc2d201f450b6020b1dce12268a550053d65235d`.
- Product candidate: `6291cafbfbb16824c577f4d530e4371c96be7455`.
- Independent original-contract maintenance candidate:
  `88923f185975482c24ad2ef8d589200954eaea74`.
- Integration: product `588ab24`, contract maintenance `31ea2b0`; accepted
  integrated revision `31ea2b0d39e4cf23e8eae5f894763edf88678cb9`.
- Exact changed paths: `crates/providers/src/responses.rs`,
  `crates/server/src/lib.rs`, `crates/sessions/src/lib.rs`,
  `crates/storage/src/lib.rs`, `crates/storage/tests/typed_tool_history_http.rs`,
  and `worklog/V2-TYPED-CONTRACT-MAINTENANCE.md`. The already-integrated
  `crates/storage/tests/typed_history_http_restart.rs` retains its frozen bytes.
- Authority: G3/G4 in `docs/CONVERGENCE.md`; pinned OpenCode `95daf906`,
  `packages/core/src/session/runner/llm.ts:249-278` publishes the durable call
  before settlement and result publication;
  `packages/core/src/session/runner/to-llm-message.ts:70-112` lowers assistant
  calls before tool results. Historical `61d473ab` supersedes the obsolete
  non-201 late-fault assertion; security-905 requires non-disclosing output
  `write success`. Independent evaluator records these corrections without
  weakening identities, payload lengths, transactional absence, or side effects.
- Frozen restart target SHA-256:
  `1ca3559f49e7f85cb2f7d483372a7ed8104665d43577888a44fc55445d4ab928`.
- Corrected original target SHA-256:
  `8eb0b453cf5844d2d5956f1a56c3b481624c14507d27f2a54b9d2954aa79863f`.
- Independent candidate verification: Xkiro GPT-6 Luna reviewed the actual
  `fc2d201..6291caf` delta, independently checked source/binary hashes, and ran
  the frozen restart gate: **2/2 passed**. Independent corrected original gate
  produced baseline RED (`QueryReturnedNoRows`) then candidate GREEN **1/1**.
- Integrated binary was built from clean exact `31ea2b0` with:

  ```text
  rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo build --offline --locked -p opencode-rk-cli --bin oc2
  ```

  Installed into disposable fixture
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-typed-integrated-31ea2b0-oj1sr8d3/bin/oc2`;
  binary SHA-256
  `87715143372e8ae96163b4807f69fd07c7281c70b0d9192e497e51589ea0ae1e`.
  Profile is dev, native feature off; this is not a release/platform certificate.
- Commands rerun serially on exact integrated revision:

  ```text
  rtk env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 OC2_TEST_BINARY=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-typed-integrated-31ea2b0-oj1sr8d3/bin/oc2 /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-storage --test typed_history_http_restart --test typed_tool_history_http --test typed_history_boundary_regression --test typed_history_component
  rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test --offline --locked -p opencode-rk-server --test agent_loop_turns
  git diff 6291caf..31ea2b0 -- crates/providers/src/responses.rs crates/server/src/lib.rs crates/sessions/src/lib.rs crates/storage/src/lib.rs crates/storage/tests/typed_history_http_restart.rs
  ```

  Results: **15/15 storage gates**, including both installed-daemon targets;
  **1/1 agent-loop integration gate**. Product/restart paths match candidate
  byte-for-byte. Candidate focused regressions also passed: 19 storage tests,
  11 Responses protocol tests, 73 provider + 42 session + 205 server library tests.
- State: **ACCEPTED for this scoped typed writer/restart package on `31ea2b0`**.
  Interrupted incomplete rounds still fail closed and are never automatically
  re-executed. Explicit recovery/quarantine, large-history compaction, concurrent
  client admission, and full native/browser/platform golden journeys remain open.

## B1/G6 browser tool-stream transport

- Package: parse native tool events and reconcile server-persisted tool rows
  after a settled turn. Implementation base `89780a3121e8626b2d134adcda93dd523a2e53ff`
  (frozen component contract over `7fe4656`); candidates `44b0ac5` and `7e264cea`.
- Independent source review and runtime verifier: main session, separate from
  Xkiro GPT-6 Luna's implementation role. The first candidate omitted the settled
  transcript refresh; the follow-up repairs that path with controller ownership
  checks before and after the bounded refresh.
- Integration: frozen contract `e67b193`, parser `4b1e0bf`, settlement `25b6448`.
  Exact integrated component-gate revision:
  `25b64488cfc4282b867a924570ba267fbcd46c32`.
- Changed paths: `web/src/lib/api.ts`, `web/src/App.tsx`,
  `web/src/lib/api.tool-stream.test.mjs`,
  `worklog/V2-WEB-TOOL-STREAM-CONTRACT.md`, `worklog/V2-WEB-TOOL-STREAM.md`.
- Authority: the native daemon's `create_turn_stream` wire contract in
  `crates/server/src/lib.rs` emits string-valued `tool_call` and `tool_output`
  fields. Pinned upstream `95daf906` uses persisted message/tool parts rather
  than this rk NDJSON extension. The existing 256 KiB line and 2 MiB stream
  bounds, late-error propagation, and authenticated same-origin transport remain
  part of the frozen contract.
- Frozen tool-stream SHA-256:
  `2f0e598ee1c0a27511bfcec06d7165b96f02ea73302b2673f2f8139b1365e7fb`.
- Independently run on candidate `7e264cea` and exact integrated `25b6448`:

  ```text
  rtk /usr/bin/arch -arm64 node --experimental-strip-types --test web/src/lib/api.tool-stream.test.mjs web/src/lib/api.auth.test.mjs web/src/lib/api.direct-auth.test.mjs web/src/lib/api.refresh-auth.test.mjs
  rtk /usr/bin/arch -arm64 pnpm --dir web run typecheck
  rtk /usr/bin/arch -arm64 pnpm --dir web exec vitest run src/lib/canvas-model.test.ts --maxWorkers=1 --minWorkers=1
  ```

  Results: **five stream controls + six auth controls passed**, typecheck GREEN,
  **16 canvas tests passed**. The scoped client transport contract is **ACCEPTED
  on `25b6448`**; real-browser G6 acceptance remains pending.
- A mistakenly broadened Vitest invocation (`pnpm test -- ...`) discovered
  existing App fixture failures and tried to execute Node `.mjs` tests as Vitest
  suites. A correctly focused baseline run on unchanged `c185ab4` independently
  reproduces `App.stream.test.tsx:113`, missing the `Streaming turn` heading before
  any stream starts. Log:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-web-base-c185ab4-b8evx209/app-stream.log`.
  These observed fixture failures remain open; the focused contract does not
  imply the entire Web suite is GREEN.
- Embedded bundle rebuilt by
  `rtk /usr/bin/arch -arm64 pnpm --dir web run build` on the integrated source.
  Entry SHA-256 `fee548caa07c879fb02ab22690252dc542ddfec6a5dd109c51804268b7b8b560`;
  JS `index-BUFkk7Km.js` SHA-256
  `74cb8fca204ccf6ac427072f3e1307343bb73b1e81ca2686d3081752445a46ad`;
  CSS `index-CjLNe1JJ.css` SHA-256
  `16b43a730b810f08f3bfb521454484f9a71b3beea18cff9c15b801769261fc12`.
  Generated bundle changes include `crates/server/web_dist/index.html` and the
  two renamed index assets. Installed-binary/browser verification follows on a
  clean committed revision containing this bundle.

## Remaining observed product failures

- G2 persisted/in-app credentials are not connected to the outbound provider
  request; current turn execution reads environment credentials.
- G3/G4 settled typed history now passes the scoped live writer/restart gate;
  interruption recovery and second-client ownership still require golden-journey
  validation.
- Native renderer lifecycle is accepted as above; the complete native CLI golden
  journey still requires G1–G4 integration.
- Docker server is ready (`29.8.0`). Ubuntu 24.04 arm64 image acquired at digest
  `sha256:008173c23f95b170204355c12626cb5a965d779a7e1283b09e9cffbb1bf33ca3`.
  The frozen native installer closure test independently reproduces one failing
  test with three missing-library/atomicity assertions against `b323e2b`.
  Ubuntu packaging and its real native golden journey remain unrun.

These component acceptances are not G0–G8 release acceptance.
