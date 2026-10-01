# V2 integration evidence — 2026-09-30

## Integrated revisions

`0759e01464a7a0dbed614163a14b6f60673e2412`, pushed to `origin/main-v2`.
Native lifecycle package: `425d617a3576c791e7027b2cc332a75e7441fd4e`.
Typed live writer/restart package: `31ea2b0d39e4cf23e8eae5f894763edf88678cb9`.
Browser transport: `25b64488cfc4282b867a924570ba267fbcd46c32`.
Persisted API auth and rooted tool continuation: `f5cfb012369f3a4187cfa4f50ad01eb4051081c6`.
Installer native closure/transaction: `14ff5fb4d93b25ab1c8e6ccc64603b2dbf3ea9a9`.
Ubuntu/Mac installer directory-layout portability: `96694bd23d97041647fe4f6308ed9fe53f2362d7`.
Native entrypoint discovery/raw/render/restore: `6c52b419d4e4029f1baf15e4b25f289872cd0bb8`.
Paired Mac/Ubuntu installed native release checks: `85c2547f20cfcb34014d80f5ae72fc25702659bc`.
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

## G2 persisted API auth and rooted streaming continuation

- Implementation base `c185ab489e2781343aa7629299a594bea79816aa`, frozen original
  contract imported as `59f54d0`, product candidates `821d12d`, `3de1d2f`, and
  final source candidate `f9eb056a2edccc12f4fb0898f905b829ce26b4ab`.
- Main independently reviewed and ran the installed candidate. Review required
  bounded file reads and absolute credential roots, then correct inline-source
  fallback and complete Api metadata schema. The independent source-selection
  contract produced three semantic RED and three GREEN controls against installed
  `3de1d2f` after mechanical HTTP-framing repair; no implementation worker changed
  the frozen assertions.
- Authority inspected at full pinned OpenCode `95daf906`: `Auth.Api` and
  `Auth.all` in `packages/opencode/src/auth/index.ts:23–27,58–71`, and persisted
  API precedence after ambient loading in
  `packages/opencode/src/provider/provider.ts:1582–1606`. Bounded auth bytes,
  records, keys, and absolute-root lookup implement the approved resource/security
  contract. Streaming retains one resolved client through continuation; relative
  file authorization uses the captured project root.
- Changed paths: `crates/providers/src/lib.rs`,
  `crates/providers/src/persisted_auth.rs`, `crates/providers/src/responses.rs`,
  `crates/server/src/lib.rs`, `crates/tools/src/file_ops.rs`,
  `crates/server/tests/prov_025_request_auth.rs`,
  `crates/server/tests/prov_030_auth_sources.rs`,
  `worklog/V2-PERSISTED-PROVIDER-CONTRACT.md`,
  `worklog/V2-PROVIDER-AUTH-SOURCES-CONTRACT.md`,
  `worklog/V2-PERSISTED-PROVIDER-IMPL.md`.
- Frozen original contract SHA-256:
  `674f09ee78ffb062265e9f33d98b1fadabbd8ad2b619d722592e62643be83344`;
  inline/schema contract SHA-256:
  `abd8a3e023ddbc607aff8367b8cb7b07ca8660780ec4fbadf935199419c4a676`.
- Candidate `f9eb056` independently passed both auth targets (**12/12**),
  `agent_loop_turns` (**1/1**), all four scoped typed-history targets (**15/15**),
  and focused file operations (**17/17**).
- Integrated through `e581589`, with mechanical removal of the unused mutable
  reader binding at `f5cfb01`. Exact verification SHA:
  `f5cfb012369f3a4187cfa4f50ad01eb4051081c6`.
- Clean integrated dev build, native off, installed at
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-integrated-f5cfb01-np94vwxp/bin/oc2`.
  Binary SHA-256:
  `1870d7627875ae450d2dcea2691d7b00e2127a30860726efeb2b43cdab434321`;
  sibling `build.json` records source and build identity. Commands on that exact
  integrated SHA, serially:

  ```text
  rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo build --offline --locked -p opencode-rk-cli --bin oc2
  rtk env TMPDIR=/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 OC2_TEST_BINARY=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-integrated-f5cfb01-np94vwxp/bin/oc2 /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-server --test prov_025_request_auth --test prov_030_auth_sources --test agent_loop_turns
  rtk env TMPDIR=/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 OC2_TEST_BINARY=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-integrated-f5cfb01-np94vwxp/bin/oc2 /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-storage --test typed_history_http_restart --test typed_tool_history_http --test typed_history_boundary_regression --test typed_history_component
  rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test --offline --locked -p opencode-rk-tools --lib file_ops
  ```

  Results: **12 auth + one agent-loop + 15 typed-history + 17 file-security checks
  passed**. State: **ACCEPTED on integrated `f5cfb01` for persisted API auth and
  rooted continuation**. In-app setup, OAuth/provider breadth, real-browser
  journey, and release/platform acceptance remain open.

## G6 real-browser settled tool/reopen/restart journey

- Exact installed product revision: `f5cfb012369f3a4187cfa4f50ad01eb4051081c6`;
  binary SHA-256 `1870d7627875ae450d2dcea2691d7b00e2127a30860726efeb2b43cdab434321`.
  Build/installation provenance is the G2 fixture above (dev, native off).
- Fixture candidates `ebd319b` and `c665fd6` required parent repairs before use:
  settled assistant-history accounting, exact typed ordering/model checks, bounded
  HTTP framing, an offline catalog, and descriptor PID/authenticated API readiness.
  Parent correction `81fddef` is selectively integrated as `289f361`. The fixture's
  `--self-check` passed loopback framing, credentials, typed pair, and settled
  history. Launch from the fixture worktree:

  ```text
  /usr/bin/arch -arm64 python3 tests/e2e/web_tool_journey_fixture.py --binary /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-integrated-f5cfb01-np94vwxp/bin/oc2 --artifact-dir /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/g6-integrated-f5cfb01-yt__rzd4 --build-json /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-integrated-f5cfb01-np94vwxp/build.json
  ```

- Main drove a real Playwright browser through authenticated launch, chat creation,
  `write the browser marker`, visible `[write] write success`, settled assistant
  output, and `confirm the browser marker`. Bare reload correctly demanded
  re-authentication. A fresh-document authenticated relaunch restored the same
  chat, tool result and both assistant replies. A fragment-only same-document
  navigation does not reinitialize authentication; the verified relaunch uses
  `about:blank` followed by the authenticated launch URL.
- Fixture-owned daemon PID 72371 was stopped/reaped. A fresh authenticated
  descriptor for PID 74191 had a new port/token. The browser attached, reopened
  the same chat, and settled `resume after restart` successfully.
- Bounded authenticated HTTP transcript verification found exactly seven unique
  messages in user/tool/assistant/user/assistant/user/assistant order. Read-only
  inspection of the disposable SQLite fixture found one tool message and exactly
  two typed records: ordered call/output for `g6_write_1`, unchanged relative-path
  arguments and `write success`. Exactly four provider requests carried expected
  settled assistant and user history. Marker bytes were exactly
  `G6 browser fixture marker\n`, SHA-256
  `09e053899d28ee2f132b9671ffe2378a9d9b0d48986152752dd0047da2c8ac1b`.
- Evidence root:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/g6-integrated-f5cfb01-yt__rzd4`.
  `provider-requests.json` SHA-256
  `1dd5f4387067ce259b76597e61725291d72dd46704f3dbe73181f89c60859e5f`;
  `typed-records.json` SHA-256
  `82e43a2bcf44b35852af8ff2dc73dc1c0737c4978d82f0de68c995eac410f486`;
  `http-transcript.json` SHA-256
  `a76d371fc9fffc0d1a02621882be331a4cd06c5c55a6f2550c38f7bc41da5fd9`.
  Screenshots and accessibility snapshots are retained there. Fixture exited 0;
  final evidence reports no provider failure, four requests, one restart.
- State: **ACCEPTED on integrated `f5cfb01` for the scoped real-browser settled
  tool/second-turn/authenticated-reopen/restart journey**. The real broker admitted
  write under the fixture's explicit operator allowlist. Interactive permission
  requests, denial/interruption, concurrent second-client admission and release
  profile/platform journeys remain open; this is not full G6/G8 acceptance.

## G7 installer native closure and paired rollback

- Frozen contract base `13616a440808d40947192126454140d955161986`;
  main-owned source candidate `7a06cc89feb8a694063356dcfdfe34798deee353`.
  Source/contracts were selectively integrated as `349e4da`, `9592556`, `fc47763`,
  and `14ff5fb`. Changed paths: `scripts/install-oc2.sh`, the three
  `tests/bootstrap/test_tui011_installer_{native_closure,rollback,signal_layout}.py`
  modules, `worklog/V2-UBUNTU-INSTALLER-CONTRACT.md`, and
  `worklog/V2-UBUNTU-INSTALLER.md`. Authority: paired replacement failures and
  historical producer layout `2f1692987ed0e27d928b290d8f2168ea37b81185`.
- Independent verifier reproduced both immutable-baseline signal/layout REDs
  against script SHA-256
  `6962df6e9998dcf86c7b7ba2793f4e9d6fe3289e36833483ed6e5757246bdf97`,
  then passed eight candidate controls. Source-independent frozen expectations
  remain identical. On exact integrated
  `14ff5fb4d93b25ab1c8e6ccc64603b2dbf3ea9a9`, independently confirmed before and
  after the gate:

  ```text
  rtk /usr/bin/arch -arm64 /usr/bin/python3 -m unittest tests.bootstrap.test_tui011_installer_native_closure tests.bootstrap.test_tui011_installer_rollback tests.bootstrap.test_tui011_installer_signal_layout -v
  /bin/sh -n scripts/install-oc2.sh
  git diff --check
  ```

  Results: **8/8 passed**, shell syntax/diff checks GREEN, source/tests byte-identical
  to candidate/frozen contracts. Script SHA-256
  `92525d449da6b5f2737e26708851d268c6741280e4bf7ce6e814697c5cdc08fe`;
  signal/layout SHA-256
  `db1b32d5472cf5c2b23bac079cf7eca7ad622667fe21b80335e3f94efefb237a`.
  Independent log: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/g7-integrated-verify/integrated-gate.log`.
- State: **ACCEPTED on integrated `14ff5fb` for the host-profile installer gate**.
  Real release-built Ubuntu installation and native golden journey remain open.

## APP-010 packaged executable identity

- Frozen contract imported through `0638c58`, SHA-256
  `44b00a50a307e4a7e039b39e84ee783670c7670d125e2dedbaca6563df054e97`.
  Main independently reran against installed `f5cfb01`: one semantic RED
  (`--version` reports `opencode-rk 0.1.0-alpha.1`) and one help/side-effect control
  GREEN. Exact command:

  ```text
  rtk env TMPDIR=/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp OC2_TEST_BINARY=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-integrated-f5cfb01-np94vwxp/bin/oc2 /usr/bin/arch -arm64 python3 -m unittest tests.bootstrap.test_app010_packaged_cli_identity
  ```

- Only the one-line `Cli` command-name correction in `crates/cli/src/main.rs` from
  historical `3e8dc0e328c07a0937a4e20c970fc1854649085f` was salvaged as `b899afc`.
  Authority is the approved `oc2` package name, installer gate, and
  `install_commands::BINARY_NAME`. Native release/installed identity and independent
  verification are pending; status is **CANDIDATE**, not acceptance.

### Installed Mac native release at `289f361`

- Serial build succeeded in 1m17s:

  ```text
  rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 RUSTFLAGS='-C link-arg=-Wl,-rpath,@executable_path/../lib' cargo build --offline --locked --release -p opencode-rk-cli --bin oc2 --features native
  ```

  Source SHA `289f36110d03f80185d6db0c6e48949b6fee8cc7`. Product sources were
  committed; only this evidence document changed during validation.
- The real five-entry archive was installed with the integrated shell installer
  into a disposable HOME/layout, exit 0, then inspected with no `DYLD_*` override.
  Fixture root:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-release-macos-289f361-n_b22vl0`.
  `build.json` retains full build/installer commands and all hashes.
  Native release binary SHA-256
  `f2ca98fe5bd151f0da669ba7a5879a23395a41f523bfe2aabfa5b4e3378b5354`;
  library `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`;
  archive `6c51f2b4a9834534ae7eb37965851a79919113da73b3ef56d6a173bea98ddb54`.
- Independent verifier confirmed these hashes, the installed `oc2 0.1.0-alpha.1`
  identity and native `@rpath/libopentui.dylib` closure with relative executable/
  loader paths. Frozen packaged identity plus eight installer controls passed
  **10/10**. Command (ROOT is the fixture above):

  ```text
  OC2_TEST_BINARY=$ROOT/install/bin/oc2 OC2_TEST_NATIVE_LIBRARY=$ROOT/install/lib/libopentui.dylib TMPDIR=/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp python3 -m unittest tests.bootstrap.test_app010_packaged_cli_identity tests.bootstrap.test_tui011_installer_native_closure tests.bootstrap.test_tui011_installer_rollback tests.bootstrap.test_tui011_installer_signal_layout
  ```

- The required installed-CLI PTY regression exposed a real product RED:

  ```text
  OC2_NATIVE_BINARY=$ROOT/install/bin/oc2 MAC_OPENTUI_FIXTURE=$ROOT/install/lib/libopentui.dylib TMPDIR=/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp /usr/bin/arch -arm64 python3 tests/e2e/native_interactive_pty.py
  ```

  This target contains **one** test, distinct from the ten bridge lifecycle tests.
  It failed at line 156: `ICANON` remained 256 after the eight-second readiness
  deadline. Log `/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp/app010-pty.log`,
  SHA-256 `cc25c53a1d54fe4006d34552e9808d6bce95476722eba26e046b6925417de793`.
  Source review confirms `tui_entry::interactive_loop` still uses canonical lines;
  `render_once` does not own terminal input. Identity/closure are independently
  GREEN; the combined release handoff is **not ACCEPTED** with this G5 RED.

## Ubuntu native dependency recovery

- Main restored only the missing aarch64 Linux blob from preserved
  `552bc16309fbe7f56d3d84cf6513eab56a47b494`:
  `crates/opentui-bridge/native/lib/aarch64-unknown-linux-gnu/libopentui.so`.
  Git blob `0ac6a19a10d33930d56e84a082dac1bad74d1919`, 26,598,960 bytes,
  SHA-256 `e85a45710e9e181b3eb7cca877a1d9022f2210bfa1e06b7c159e734506da3b89`.
  Actual bytes match the retained audit fixture and preserved blob exactly.
- Historical `552bc16:worklog/TUI-011.md` records pinned fork `c01292fd`, Zig
  0.16.0, ReleaseSafe, `aarch64-linux-gnu.2.17`, five required ABI exports and
  bounded ELF dependency verification. The arm64 artifact is ELF aarch64,
  SONAME `libopentui.so`, without absolute RPATH. This is dependency recovery,
  not a passing Ubuntu product journey.
- The current canonical x64 artifact has SHA-256 `e84ced36…`; the audited fresh
  historical x64 blob has `9f074adf…`. Historical worklog explicitly distinguishes
  these. Existing x64 bytes remain preserved pending their own runtime/audit gate.
- Disposable Ubuntu arm64 container `oc2-v2-ubuntu-build` uses the exact acquired
  Ubuntu digest, two CPU/four-GiB limits and only an approved temporary bind mount:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-ubuntu-arm64-release-dqhrz_mn`.
  Public package setup and Rust 1.98.1 installation passed; the host registry cache
  was copied without user auth/config. Release build, native loading/installation,
  and actual Ubuntu golden behavior remain to be verified.

## Native daemon fixture freeze and runnable discovery RED

- Independent mechanical fixture history `459c531`, `f8c08e1`, `8b5b6ef` was
  selectively integrated as `e0563fc`, `12ad2b4`, `c8dd682`. Prior dirty bytes and
  both failed-helper PID files remain preserved. Portable embedded-driver checks
  passed true exit 0, false exit 1, and TERM cleanup of child plus descendant.
- Frozen Rust test SHA-256:
  `8d319b35ac851f96b1f79deb76c60c042e654d33864bf80f5c5a78ab1f74e21b`;
  installed interactive Python contract SHA-256:
  `02d0e7fd0211fa138594fc5633f32597af17261ec16d15e3fc54cf5c6c387342`.
  External `freeze.json` and helper results are retained under
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/native-fixture-interrupted-7v7fzxae`.
- Focused gate ran on exact integrated
  `0b00ae430c0ddddfe8cc70efa6707c1ac46d51b1`:

  ```text
  rtk /usr/bin/arch -arm64 env TMPDIR=/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 RUSTFLAGS='-C link-arg=-Wl,-rpath,@executable_path/../lib' cargo test --offline --locked -p opencode-rk-cli --features native --test native_daemon_flow -- --nocapture
  ```

  Compiled in 37.73s; **three GREEN / one semantic RED** in 0.76s. Fresh default
  daemon ownership, redirected-entry exit 2/no daemon, and explicit-origin live
  frame passed. Genuine `tui` without `--origin` failed at line 790: frame lacked
  `NativeTuiLiveProbe` and `(live)`. It showed `model: unset`, no live session.
  Log: `/Users/mymac/.local/share/opencode/shell/6b4e89130cd2bcaa27985bc8bdd3449317a541c3/sh_0f30e509a001qGXI9ePH8xWiOK.out`.
- This is now a compiling, executed product-discovery RED. It does not substitute
  default chat or explicit origin for the failing entrypoint. A source-only native
  entrypoint repair is leased separately over the frozen contracts; main retains
  integration/build authority. Fixture maintenance is not product acceptance.

## Ubuntu installed native release and platform-specific layout RED

- Archived source `0b00ae430c0ddddfe8cc70efa6707c1ac46d51b1` built offline/locked
  in the disposable Ubuntu 24.04 arm64 container, Rust 1.98.1, native release
  profile, two Cargo jobs, `$ORIGIN/../lib` executable RUNPATH. Cargo exited 0 in
  114.1 seconds including command overhead (reported build time 1m30s).
  Exact command is retained in `/work/build-command.txt`; output in
  `/work/cargo-release.log` under the approved Ubuntu fixture root above.
- The real native release archive installed into a disposable HOME, exit 0.
  Independently confirmed build/binary/library/archive provenance:
  binary SHA-256 `38ad662e59a6c37655320bfe5cad7caf982fadc2adbe976981ca9a29630d7d35`,
  library `e85a45710e9e181b3eb7cca877a1d9022f2210bfa1e06b7c159e734506da3b89`,
  archive `87abb050e20f2cf0ea76b827bb449da76456dd4fb97597fbd3efb8bada3c509f`.
  Installed fixture: `/work/ubuntu-installed-verified/install/bin/oc2`, companion
  `../lib/libopentui.so`, provenance `/work/ubuntu-installed-verified/build.json`.
- Independent `readelf`/`ldd` checks confirmed ELF64/AArch64, executable
  `NEEDED libopentui.so`, RUNPATH `$ORIGIN/../lib`, library SONAME `libopentui.so`,
  no unresolved dependencies, and resolution to the owned installed companion.
  Minimal-env version was `oc2 0.1.0-alpha.1`; no `LD_LIBRARY_PATH` was needed.
- Same frozen identity/installer controls were independently run in Ubuntu:

  ```text
  docker exec --workdir /work/source -e OC2_TEST_BINARY=/work/ubuntu-installed-verified/install/bin/oc2 -e OC2_TEST_NATIVE_LIBRARY=/work/ubuntu-installed-verified/install/lib/libopentui.so -e TMPDIR=/work/verify-tmp oc2-v2-ubuntu-build python3 -m unittest tests.bootstrap.test_app010_packaged_cli_identity tests.bootstrap.test_tui011_installer_native_closure tests.bootstrap.test_tui011_installer_rollback tests.bootstrap.test_tui011_installer_signal_layout
  ```

  **9/10 passed, one RED**: historical nested directory entries failed during
  extraction, exit 65. Independent log `/work/ubuntu-independent-verify/packaged-installer.log`,
  SHA-256 `573d3544d60457819ec3855dc3adfbde109763e010029a5f3a63a5a2607f9952`.
  The parent traced GNU tar's rejection to mode-000 directory headers; direct
  extraction reproduction failed exit 2 with permission errors. This contract is
  unchanged from the Mac run, where it passed.
- Product script repair is integrated CANDIDATE
  `96694bd23d97041647fe4f6308ed9fe53f2362d7`; source SHA-256
  `9b7f259dc02bb38e86cac52482e0ac01c2c09074c648533f3e6ec6d255c07f16`.
  Parent reruns passed Ubuntu 10/10 and Mac installer 8/8. Independent exact-script
  verification is pending. See `worklog/V2-UBUNTU-LAYOUT-REPAIR.md` for immutable
  baseline, contract hashes, repair and commands. Ubuntu native golden remains open.
- The initial independent ABI filter produced empty output because the filter was
  mechanical, not evidence of absent exports. The bounded parent symbol-table
  probe found all nine required render/lifecycle symbols. Independent corrected
  ABI evidence is pending alongside the script gate.

### Independent installer acceptance on `96694bd`

- Independent verifier staged only the exact integrated script in
  `/work/ubuntu-layout-independent-verify/install-oc2.sh`, hash `9b7f259d…`.
  All three frozen installer test sources match their original hashes and the
  canonical checkout. **Ubuntu 8/8 and Mac 8/8 passed independently**.
- Ubuntu exact-script gate used the frozen `/work/source` test modules and
  `TUI011_INSTALLER_SCRIPT=/work/ubuntu-layout-independent-verify/install-oc2.sh`.
  Log `/work/ubuntu-layout-independent-verify/ubuntu-8.log`, SHA-256
  `9daebb0e55c3e924de034b9ffc5547583736536d673350db0a8074ba0bfde932`.
  Mac exact-script gate used canonical `scripts/install-oc2.sh` with the same
  three unittest modules. Log
  `/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp/ubuntu-layout-independent-verify/mac-8.log`,
  SHA-256 `aebfc3e2c0462c30e33918cd883e662ed115c61eed4e3717d12f6b51f3fb86c4`.
- Corrected independent ABI filter found all nine actual required render/lifecycle
  exports in the installed Ubuntu native library. Bounded 154-byte log
  `/work/ubuntu-layout-independent-verify/native-abi-9.log`, SHA-256
  `3bf671486ebfcf409176428c13e023ab204861b2ffa6309b0222d3882b58d44e`.
- State: **ACCEPTED on exact integrated `96694bd` for installer-only portability**.
  Native Ubuntu binary provenance remains source `0b00ae4`; this acceptance does
  not relabel that artifact as a `96694bd` build or certify the native golden path.

## Native entrypoint recovery — exact integrated scoped acceptance

- Package base `0b00ae430c0ddddfe8cc70efa6707c1ac46d51b1`; candidate
  `3ece0394b414f87aab34700f1bb6c631566f5b3b`. Candidate commits `47d6e38`,
  `be7fe0a`, `3ece039` were independently preverified then integrated as
  `0e5ca05`, `8205ee7`, `6c52b41`. Changed paths:
  `crates/cli/src/main.rs`, `crates/cli/src/chat.rs`,
  `crates/cli/src/tui_entry.rs`, `crates/opentui-bridge/src/safe_renderer.rs`,
  `worklog/V2-NATIVE-ENTRYPOINT-RECOVERY.md`.
- Evidence: pinned OpenCode `95daf906` `packages/cli/src/tui.ts:7–19` drives the
  event-based TUI runner; historical `2263e91`/`53845c2` supply native default
  routing and persistent renderer salvage. The pinned OpenTUI fork `c01292fd`
  `packages/native/src/renderer.zig::{getCurrentBuffer,getNextBuffer,render}`
  identifies next as the writable scene and current as the presented scene.
  Painting current produced the independently observed all-space frame. The
  bridge now paints next and commits memory frames before snapshotting current.
- Native builds now route bare TTY launch and implicit `tui` through authenticated
  daemon discovery/ownership; empty authenticated catalogs create an actual first
  session. Interactive native rendering owns raw input/restoration through the
  existing bridge. Redirected entry remains exit 2; scriptable once/follow retain
  separate paths. Composer/transcript growth is bounded.
- Frozen Rust daemon-flow, Python interactive, and bridge PTY contracts retained
  their exact hashes `8d319b35…`, `02d0e7fd…`, `3799fb34…`. Main independently
  preverified candidate: **4 daemon-flow, 1 installed-PTY, 73 native bridge and
  10 bridge PTY tests passed**. Candidate/source hashes and logs:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-native-entry-preverify-3ece039-7uaoqt1y/preverification.json`.
- Main independently reran on exact integrated
  `6c52b419d4e4029f1baf15e4b25f289872cd0bb8`: **4 daemon-flow, 1 installed-PTY,
  73 native library, 10 bridge PTY, and 72 default library tests passed**.
  Required commands used jobs 2, test threads 1, offline/locked and the explicit
  relative executable loader flag:

  ```text
  /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-cli --features native --test native_daemon_flow -- --nocapture
  OC2_NATIVE_BINARY=<canonical>/target/debug/oc2 MAC_OPENTUI_FIXTURE=<canonical-native>/libopentui.dylib /usr/bin/arch -arm64 python3 tests/e2e/native_interactive_pty.py
  /usr/bin/arch -arm64 env DYLD_LIBRARY_PATH=<canonical-native> TUI015_NATIVE_LIB_DIR=<canonical-native> cargo test --offline --locked -p opencode-rk-opentui-bridge --features native --lib --test native_terminal_lifecycle
  /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-opentui-bridge --lib
  ```

  Full commands, environment values, source hashes, results and log hashes are in
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-native-entry-integrated-6c52b41-rrzto9oc/verification.json`.
  An initial bridge invocation inherited `DYLD_LIBRARY_PATH` before `arch`, which
  macOS strips; its loader failure is retained. The corrected manifest places the
  explicit fixture variable after `arch` and the same unchanged tests pass.
- State: **ACCEPTED on integrated `6c52b41` for native entrypoint authenticated
  discovery, persistent visible frames, raw single-byte redraw, Ctrl-C exit and
  restoration**. This is dev-profile scoped acceptance. In-app provider setup,
  native streaming, full UTF-8/escape/resize/mouse handling, interruption and
  second-client ownership, and exact release/platform golden journeys remain open.

## Paired installed native release — exact source `85c2547`

- Both release/native binaries were built offline/locked from exact integrated
  `85c2547f20cfcb34014d80f5ae72fc25702659bc`, archived with their platform library,
  and installed in separate disposable fixtures. Mac Cargo completed exit 0 in
  18.59s; Ubuntu completed exit 0 in 29.42s under the two-job budget. Build logs
  and receipts retain commands, source provenance and artifact hashes.
- Mac receipt:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-release-macos-85c2547-rd2cct9_/build.json`;
  binary `35dd32a48f94ec5dd8f0406a03adda09eb93ba280edebf7da0b3c98d5ed18f10`,
  native library `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`,
  archive `db5fd96081851cab661bbf748b8856f5a9e89c6f043dcc152aa0e97532371669`.
- Ubuntu receipt `/work/ubuntu-installed-85c2547/build.json` in the existing
  disposable container; binary
  `374313f2acc21e40c9bfe056fb206b234c7b39dc5c5626adae34ac5269a5a116`,
  library `e85a45710e9e181b3eb7cca877a1d9022f2210bfa1e06b7c159e734506da3b89`,
  archive `05f70dffeb00b8be62f64419c59c8904536d6e3055dae793f77cc16a27b8b735`.
  Its source archive is `/work/source-85c2547`, not the older `/work/source`.
- Portable PTY fixture was independently maintained as `6c24d337` then corrected
  `ff4ec3cc`, integrated as `62b8f40`/`4c7a8d4`. It changes only platform library
  staging/environment selection. An erroneously added parent controlling-TTY
  ioctl was caught and removed before runtime verification. The full test-method
  AST is identical to its original, SHA-256
  `f510581f59f4364c0aea44ded2222b2853d0ce989a5dc656d261e20f883a7efa`.
  Final file hash `51c37cfe066987ab94b3bba8e8496c7632a37902702e4caf571f8b1dc4ebc9b5`.
  All eight semantic assertions, timeouts and byte limits are retained.
- Independent verifier ran **22/22 controls GREEN** serially: the same ten
  packaged identity/installer controls and one installed native PTY test on each
  platform. Generic fixture variables were `OC2_TEST_BINARY`,
  `OC2_TEST_NATIVE_LIBRARY` for packaging and `OC2_NATIVE_BINARY`,
  `OC2_NATIVE_LIBRARY` for the PTY. Mac used `arch -arm64 env ... python3`; Ubuntu
  used `docker exec --workdir /work/source-85c2547 ... python3`. Both sets ran:

  ```text
  python3 -m unittest tests.bootstrap.test_app010_packaged_cli_identity tests.bootstrap.test_tui011_installer_native_closure tests.bootstrap.test_tui011_installer_rollback tests.bootstrap.test_tui011_installer_signal_layout
  python3 tests/e2e/native_interactive_pty.py
  ```

  Mac logs in the retained short-temp `release-verify-85c2547-mac` directory:
  packaging SHA-256 `ec6bc2cc460f09d040f90f84f5ffe82274fcc83d8f14dfbad674e74f1b2d6e99`,
  PTY `7935e4052339b7063a661f397fd8117cf6a7bf6cf6449c94879ee4fe09beb7e9`.
  Ubuntu `/work/release-verify-85c2547-ubuntu` logs:
  packaging `e0b03469e16c3c65fe8ac54b67d2b5127714f36320b8862e2602e642156065e0`,
  PTY `6c7b657d35909b1c58712fe4f9486f6977b2a49624ec97110cec00995d25639c`.
- Independently verified relative native loader closure without `DYLD_*` or
  `LD_LIBRARY_PATH` overrides, and all ten required render/lifecycle exports,
  including the newly used `getNextBuffer`.
- State: **ACCEPTED on integrated release source `85c2547` for packaged identity,
  installer rollback/layout and native raw-input/redraw/Ctrl-C/restoration**.
  Fresh-HOME in-app credentials/model choice, native streaming/tool/restart,
  second-client ownership and full G7/G8 golden acceptance remain open.
- Canonical commits through `85c2547` were pushed normally to `origin/main-v2`.

## Installed native provider setup — frozen executable RED

- Independent contract-owner history `67445cf`, `88f0df`, `39b71aa`, `79f8ec1`
  is preserved and imported. Controller maintenance/freeze `1523144` is integrated
  as `9a42c4a`. Only `tests/e2e/native_provider_setup.py` and
  `worklog/V2-NATIVE-PROVIDER-CONTRACT.md` are added by this test package.
- Pinned native authority is `95daf906`, `packages/tui/src/app.tsx:739–745`,
  `component/dialog-provider.tsx:228–230,352–417`,
  `component/dialog-model.tsx:136–154`, `context/local.tsx:164–234,320–337`, and
  the legacy-compatible auth control route/schema in
  `packages/opencode/src/server/routes/instance/httpapi/groups/control.ts:32–49`.
  `/connect` opens a provider dialog; user-entered API auth is persisted before
  selecting a catalogue model; model recents drive the restart fallback.
- Frozen test hash
  `752c97c5d1eee013e4d350e62428be5938987b5bb3491967122f7fa00b29b3ff`.
  The fixture supports valid Responses JSON and SSE wire formats but freezes only
  onboarding/auth/model/settled-history behavior. It cannot establish native
  streaming parity from a non-stream request.
- Controller ran the actual installed release source `85c2547`. Healthy owned
  authenticated descriptor and both exact catalogue IDs were verified first;
  `/connect` rendered as composer input and then ordinary turn HTTP 503. No
  connection dialog appeared, no `auth.json` was created, and the provider
  received zero requests. The run failed exit 1 in 20.77s, while all known owned
  process groups were independently confirmed stopped afterward.
- Reproduction, command manifest, sanitized PTY, receipt/test/log hashes and
  objective failure classification:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-g2-native-red-85c2547-opjif3xe`.
  A separate earlier socket-path fixture failure is retained and classified as
  infrastructure; it was repaired without relaxing the 100-byte socket bound.
- State: frozen **product RED**, ready for one coherent native provider connection,
  model selection and durable resume repair. This test handoff is not acceptance.

## Fresh workspace formatting failure

- Read-only `cargo fmt --all -- --check` on exact `0b00ae4` returned exit 1.
  Raw log contains 932 diff sections across 157 printed path spellings, including
  module aliases through `../src`. No files were reformatted by this gate.
- Evidence:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-mechanical-gates-0b00ae4-4s96j56l/fmt.log`;
  `result.json` records the source SHA, log hash and reported paths. Workspace
  formatting is a reproduced mechanical gate failure; Clippy/workspace runtime
  gates remain to be captured under the single-heavy-validation budget.

## Remaining observed product failures

- G2 persisted OpenAI API auth and in-app API-key/model setup are accepted on
  `ac635fa` as recorded below. OAuth and adaptation beyond OpenAI remain open.
- G3/G4 settled typed history now passes the scoped live writer/restart gate;
  interruption recovery and second-client ownership still require golden-journey
  validation.
- Native renderer lifecycle is accepted as above; the complete native CLI golden
  journey still requires G1–G4 integration.
- Docker server is ready. Ubuntu 24.04 arm64 image acquired at digest
  `sha256:008173c23f95b170204355c12626cb5a965d779a7e1283b09e9cffbb1bf33ca3`.
  Host-profile installer closure/transaction is accepted as recorded above.
  Ubuntu native release packaging/loader closure is verified as above; its complete
  native golden journey remains pending.

These component acceptances are not G0–G8 release acceptance.

## Integrated native onboarding, credential safety and paired release acceptance

- Package: `G2-native-provider`, plus the mechanically repaired Web runners.
  Product base `710a410e0ce32c716c40217696b515caa6c52310`; independently
  PREVERIFIED candidate `bdee47eac2e31f25a4787944a230a2ec8554e70b`; exact
  integrated revision **`ac635fac5298e9151c6575aa6a089f53c5aa4309`**.
- Product paths: `crates/cli/src/native_setup.rs`, `crates/cli/src/tui_entry.rs`,
  `crates/opentui-bridge/src/safe_renderer.rs`,
  `crates/providers/src/persisted_auth.rs`, `crates/server/src/daemon.rs`,
  `crates/server/src/daemon_auth.rs`, `crates/server/src/lib.rs`. Candidate
  `worklog/V2-NATIVE-PROVIDER-LIVE.md` is the eighth changed path.
- Preserved candidate commits `8d974c06`, `0cbaf618`, `c6a29470`, `aedc289`,
  `7db4a83`, `bdee47e` integrate as `7f06f45`, `5973f61`, `3ff80e3`, `552c617`,
  `aeaf87e`, `ac635fa`. The canonical branch is the only integration writer.
- Pinned upstream remains `95daf90670b7c039c436c85537da5fbfe2205b41`:
  provider dialog `packages/tui/src/component/dialog-provider.tsx:352–417`,
  model dialog `component/dialog-model.tsx:23–154`, model recents
  `context/local.tsx`, auth schema/filter/merge
  `packages/opencode/src/auth/index.ts:23–27,58–97`, legacy-compatible control
  `server/routes/instance/httpapi/groups/control.ts:32–49`, and per-client atomic
  recents persistence `packages/tui/src/util/persistence.ts:22–33` were inspected.
- Current frozen G2 SHA-256 is
  `32edfa5c0ac5eacef9cfa949e02fce0d3440fe1cd8f75580efb584bc0a7780dc`;
  eight credential controls are
  `f5b7dd3372b1d0aac4e285d16fb4295833854c8ab9b6ec836bacdb9205d8809d`.
  Independent observer maintenance handles dirty-cell VT output and Responses'
  default non-stream JSON. The old fixtures/RED evidence remain preserved.
  All seven original auth-control method ASTs remain identical.

### Actual exact-integrated gates

- Both releases were built with `cargo build --offline --locked --release -p
  opencode-rk-cli --bin oc2 --features native`, two Cargo jobs and one test
  thread, then archived and installed through `scripts/install-oc2.sh`. Mac uses
  `@executable_path/../lib`; Ubuntu uses `$ORIGIN/../lib`. No loader overrides are
  present in installed PTY/onboarding runs.
- Mac and Ubuntu each pass **two real provider requests across restart** from
  UI-created masked API auth, using non-default `gpt-5.6-mini`, restored model,
  prior user and assistant history, exact bearer and owner-only auth/descriptor.
  Independent verifier `ses_f0c3bda7effeqXZjDfJqgOWKVF` reran the canonical G2
  fixture and all **8/8 credential controls on each installed release**. All
  sixteen auth-case process receipts report reaped owned processes.
- Controller exact-integrated Mac regressions pass: **12 auth-source controls,
  4 native daemon flows, 73 bridge + 10 native lifecycle checks, one raw-input
  PTY/restoration journey, 10 installed identity/packaging controls, 49 Vitest
  assertions and 11 Node assertions**. Ubuntu passes the native PTY and all ten
  installed identity/packaging controls; normalized relative loader closure and
  ten required native ABI exports pass.
- The real Playwright browser ran the same exact Mac release: prompt, broker
  file write, typed tool continuation, second turn, fresh authenticated document
  reopen, daemon restart and a resumed third turn. Evidence confirms **4 provider
  requests, 7 distinct durable messages, one tool message and one ordered typed
  call/output pair in resumed input**, with the exact marker bytes and no fixture
  provider failure. The owned browser fixture stopped cleanly. Native streaming
  is outside the settled G2 contract.
- Mechanical Web package preserves all 20 original test files and accepted
  `api.ts` bytes from `710a410`; rejected malformed-fragment preflight proposal
  `45ed373` is preserved and explicitly superseded. Final runner/fixture net
  changes are independent of product transport.

### Retained evidence

All paths below share the approved artifact parent
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/`:

- Mac: `v2-integrated-mac-ac635fa-di8p3h86/`, including `build.json`,
  `commands.json`, original logs, `independent-g2-fresh/` and
  `independent-auth-fresh.log` (SHA-256
  `f34089458bce62b3d8fa0aafc21b5b2896b38dd18b2424a4a010a6ea4c567e9f`).
- Ubuntu: `v2-ubuntu-arm64-release-dqhrz_mn/ubuntu-native-ac635fa/`, with
  source archive/build receipts, all installed logs, fresh independent controls,
  `loader-abi-verification.json`. Source archive resides alongside
  `source-ac635fa.tar` and the read-only extracted source.
- Browser: `v2-browser-ac635fa-20261001/`, including provider inputs, seven
  durable messages, original browser captures, fixture result and browser
  verification SHA-256
  `5ba7c5ee204a1f7d639cbc88a09b87934aae70f97f36ebad19011e2077f6f371`.
- Mac binary/library/archive SHA-256: `a26248e738e8e5292e72fe75c85188d87aceca9b119e478b15a3400eb45a4a21`,
  `798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`,
  `8d8b40b8f7e94a08698cdf9d73cc56a3cce6d9f1f7875d466a4f3e449f60e6b8`.
- Ubuntu binary/library/archive SHA-256: `ea8f4bc0a98e12808ef561a5568ef10f05d2a2c7f2c31b07ace480052b3f1e52`,
  `e85a45710e9e181b3eb7cca877a1d9022f2210bfa1e06b7c159e734506da3b89`,
  `7bb980ceb93dc2e8f3faa15d88bf840655a78e06c7f2b8f16b9801b12b6836c5`.

State: **ACCEPTED for scoped native provider onboarding/credential safety,
paired release packaging and existing Web journey on exact `ac635fa`**.
Native provider streaming/tool UI, interruption, second-client ownership,
full input/resize/mouse behavior, per-session file rooting, non-OpenAI adaptation,
G0 product-tip disposition, formatting and fresh workspace/Clippy gates remain
open. This acceptance does not certify the complete G0–G8 release.

## Next reproducible failures after onboarding acceptance

- The separate frozen native streaming/tool journey is integrated as `7716079`
  and has SHA-256
  `f7d8fbe0cdc3f7be94e60a18bdfc2845f992038c6081bcba9c40ebbbe0c5f921`.
  Actual accepted installed `ac635fa` sends a non-stream provider request and
  displays only `NONSTREAM_COMPLETED`; it cannot show the gated first text delta.
  The fresh 11.37s product RED, bounded cleanup and source/test/command manifest
  are at `v2-stream-frozen-red-d2wrmua_` under the approved artifact parent.
- Source inspection of `TurnStreamStage::NextRound` additionally finds that
  clearing `assistant_text` discards pre-tool text before final persistence.
  The coherent client/server repair must retain it across tool continuation.
- Fresh `cargo fmt --all -- --check` on `4a3e98e` fails exit 1 in 1.09s.
  Fresh `cargo clippy --offline --locked --workspace --all-targets --all-features
  -- -D warnings` fails exit 101 in 4.42s at fourteen security-library lint
  diagnostics. Both were read-only, serial, with two Cargo jobs and two test
  threads. Logs/commands/hashes reside at `v2-workspace-gates-4a3e98e-ulyckkel`.
  The narrowly granted mechanical security package was subsequently ACCEPTED on
  exact `cb0ab24` with actual Clippy exit 0 and 148 library tests, as recorded in
  `worklog/V2-SECURITY-LINT.md`. Workspace formatting and broader Clippy remain
  RED.

## Selected approval expiry/retention salvage accepted

- Package `APP012-APPROVAL-LIFETIME`, base
  `c84b4f741b28b57308ef75a248fb292a4f8e3ffa`; independently PREVERIFIED product
  candidate `c8864330eb018b0e8c8cf02e0ead94a1647414de`; exact integrated SHA
  **`dddb8d4ab1de0d4501353451651bec979058f84f`**.
- Product paths: `crates/security/src/app_policy.rs`,
  `crates/storage/src/approvals_v2.rs`, `crates/storage/src/retention_v2.rs`.
  The owned handoff is `worklog/V2-APPROVAL-EXPIRY-LIVE.md`.
- Current approved authority is inclusive approval expiry (`expires_at_us <=
  now_us`), terminal state 3, caller-selected resolved-age retention and explicit
  human grant expiration bounds. No new cutoff, TTL or retention choice was
  introduced. Historical `dc3331ee257d` implementations/tests were selected as
  useful salvage and extended with a real transition/timestamp control; all
  proposal and claims evidence is preserved.
- The controller captured compiling RED before implementation, froze both
  executable contract hashes, and reran the same required gates after integration:
  **148 security library + 1 expiry + 3 real retention + 8 approval transition +
  6 retention regression tests**, plus security-library Clippy exit 0.
- Exact commands, source/test/log hashes and results reside at
  `v2-approval-lifetime-integrated-l1ymyfck` under the approved artifact parent;
  independent candidate evidence is `v2-approval-expiry-independent-c886`.
  State **ACCEPTED for expiry/terminal retention on integrated `dddb8d4`**.
  This does not accept the complete APP012 interactive approval story.

## Native stream/tool/restart accepted on paired installed releases

- Package `NATIVE-STREAM-TOOL-RESTART`; independent candidate
  `ed727a6577527fc560a6dfc736fbdb0479654f06`; integration base
  `85dfa9df6927e73922b33020d88c32a2204e01cc`; exact integrated release
  **`de7e05fefba5374246078674432dcba07b0b5ff2`**.
- Six net paths, exact upstream/current-requirement evidence and complete scoped
  verification are in `worklog/V2-NATIVE-STREAM-TOOL-LIVE.md`. All candidate
  history, failed input/cleanup attempts, old contract revisions and historical
  refs remain preserved.
- Native now streams early text while processing raw input, presents the actual
  broker-authorized write, persists partial-plus-final text across tool rounds,
  completes a second turn and resumes typed history after both CLI and daemon
  restart. Mac and Ubuntu each pass exactly four actual provider requests with
  owned cleanup; onboarding still passes with two requests and the non-default
  model. Both platforms pass all eight auth controls, raw PTY restoration and ten
  packaging controls. Mac additionally passes four daemon flows, 73 bridge + ten
  lifecycle checks and two server-stream tests. Ubuntu verifies relative RPATH,
  normalized adjacent-library resolution and ten required native exports.
- Same-SHA real Playwright passes write/continuation, second turn, authenticated
  fresh-document reopen, daemon restart and resumed third turn, with four actual
  requests, seven unique durable messages and one typed tool result. The owned
  fixture exits 0. Web regressions pass 49 Vitest tests + 11 Node assertions.
- Evidence roots under the approved artifact parent:
  `v2-native-integrated-de7e05f-slroa5md`,
  `v2-ubuntu-arm64-release-dqhrz_mn/ubuntu-native-de7e05f`,
  `v2-browser-de7e05f`. Independent verifier
  `ses_f0b98d797ffeWTf7GuNITBHwlW` confirmed **ACCEPTED on exact `de7e05f` for
  the scoped native streaming/tool/restart and paired-release/Web journeys**.
  This does not certify complete G0–G8 release acceptance.

## Fresh workspace failures after scoped streaming acceptance

Read-only, serial gates ran on `de7e05f`, with two Cargo jobs, one test thread,
fresh disposable HOME/XDG directories, the short fixture TMPDIR and staged
native library. Actual commands and outcomes:

```text
cargo fmt --all -- --check
  exit 1 in 1.60s: workspace formatting differences
cargo clippy --offline --locked --workspace --all-targets --all-features -- -D warnings
  exit 101 in 10.31s: 30 storage-library diagnostics
cargo test --offline --locked --workspace --all-targets --all-features -- --test-threads=1
  exit 101 in 42.14s: MCP config test fixtures do not compile
```

Evidence is retained at `v2-workspace-gates-de7e05f-nspragpt` under the approved
artifact parent. Exact log SHA-256 values:

- Formatting: `213e620ea45671eeedbce79e8ee02c1491f4b81aa02360ef5458ad26b83ab9fd`.
- Clippy: `9acdc4b943b640e39f78382f7ba4dbe6f79da1b07c784513042efec9eaa64899`.
- Test build: `d281dbee32386015f672e98e19da423c4f4a0ffd6a1853d5cbb64a5101682758`.

The workspace test failure is mechanical compiler RED, not feature RED or a
running test result. An independent test owner is repairing invalid JSON array
fixtures and missing imports without changing assertions. A separate source-only
storage package repairs diagnostics while preserving public APIs, SQL,
transactional admission, error checks, schema, byte bounds and protected tests.

### MCP compiler blocker repaired

Independent test-owner candidate `2db02c0dcc7b37af4a989f8c3bc8dfa427b6543e`
was reviewed and independently run before integration. The controller reran the
same focused target after integration on exact
`4ab19b78edd7f31ed76d1b06deb5ec3ed26573bc`: **20 passed, 0 failed, exit 0**.
Only `crates/tools/tests/mcp_config.rs` and
`worklog/V2-MCP-CONFIG-COMPILER.md` change. Argument counts/byte bounds,
URL/merge/status assertions and all named tests are preserved. State **ACCEPTED
for mechanical MCP fixture compilation on integrated `4ab19b7`**. Exact receipt
roots are `v2-mcp-compile-preverify-dg2lsi4n` and
`v2-mcp-compile-integrated-6ihhaphu` under the approved artifact parent.

### Storage diagnostic package accepted

Candidate `d099194edc1afb87adb1f1e10eab80e969aab2aa`, base `de7e05f`, was
independently PREVERIFIED after actual Clippy and all storage tests against its
attested installed release. The controller integrated it as
**`ed6e78736ae8aad091ba2f0ba182754cbbbe155c`**, rebuilt/archived/installed that
exact SHA and repeated all required gates: storage-library Clippy exit 0,
**212 storage tests + 1 frozen security expiry test passed**, plus frozen native
stream/tool/restart and onboarding journeys (four / two requests, both exit 0).
All eight storage source paths and the preservation/exception review are in
`worklog/V2-STORAGE-LINT.md`. Protected tests, SQL, schema, public signatures,
error checks and bounds remain unchanged. State **ACCEPTED for scoped
storage-library maintenance on exact `ed6e787`**.

Exact integrated receipts are `v2-native-storage-integrated-ed6e787-i82gzid6`
and `v2-storage-integrated-ed6e787-_y6n1zov` under the approved artifact parent.
The recorded offline-cache and missing-installed-fixture failures are preserved
and classified; neither was promoted to GREEN. Normal push through reviewed
canonical `9c8829019b750df6a4cb4e79d6be82bfa83e4dd8` succeeded without force.

### Formatting accepted; workspace gates advance to actual failures

Mechanical candidate `b046ca453a5e3edbe5b30bf53ccc42be9d04c074`, base
`ed6e787`, was independently compared against canonical rustfmt output for all
778 tracked Rust paths. It changes 149 Rust paths; all are formatter-only and
the protected native/auth/approval tests and lockfile stay byte-identical.
After integration on exact **`f6e2d3b05c21f97c7224a333f177c60dfe210c0d`**, the
workspace formatting gate passes. The installed native release still passes
four-request stream/tool/restart and two-request onboarding. State **ACCEPTED
for workspace formatting on exact `f6e2d3b`**. Full path manifest and evidence:
`worklog/V2-WORKSPACE-FORMATTING-PATHS.json` and
`worklog/V2-WORKSPACE-FORMATTING.md`.

Fresh workspace Clippy then fails at 44 tools-library and six provider-library
diagnostics. Fresh all-target/all-feature workspace tests compile and execute,
then fail `t09_distinct_session_ids`: two back-to-back child spawn plans share
the same clock-only session identity. These exact failures and return codes are
retained at `v2-workspace-gates-f6e2d3b-20jfnwb5` under the approved artifact
parent. Focused identity and provider maintenance candidates require independent
verification before integration.

### Identity and provider packages accepted on exact integrated SHA

Independent verifier `ses_f0b4ae7dbffeEfFB4wLruGJkYg` PREVERIFIED identity
candidate `2f06dc6` and provider candidate `bbdfff2`. The controller integrated
them as coherent commits and reran the required commands on exact
**`7ffdc6e13385027a1ecabff66d570ad88b963502`**: 11 focused identity tests,
**126 agents tests**, provider-library Clippy with `-D warnings`, and
**397 provider tests** pass. Workspace formatting also passes. All protected
tests, manifests and the lockfile remain unchanged. Exact evidence:
`v2-identity-provider-integrated-7ffdc6e-alf5eet2`, under the approved artifact
parent. Scope and paths are in `worklog/V2-DELEGATION-IDENTITY.md` and
`worklog/V2-PROVIDER-LINT.md`; these are **ACCEPTED for the composition identity
contract and provider-library diagnostic maintenance on exact `7ffdc6e`**.

Native valid-Unicode input has a newly frozen real installed PTY RED at
`f6e2d3b`: actual provider text is mojibake, with partial temporary-emoji
backspace. CLI exit, terminal restoration, validated daemon termination and
provider-thread cleanup are proven. The independent test contract and exact
frozen hash are in `worklog/V2-NATIVE-UTF8-CONTRACT.md`; product implementation
requires separate installed candidate and exact integrated verification.

### Transcript paging regression repaired

Unicode candidate `cfe6cbf` passes its real installed UTF-8/restart, stream,
provider, auth and PTY checks, but its required native-unit gate exposed two
existing transcript-paging failures. The controller reproduced both on canonical
baseline `06eadf1`, with the unchanged frozen in-module test body. Source-only
candidate `5c6f9cb` adds a bounded positional anchor matching the existing native
timeline helper. After independent candidate verification and integration, all
three focused paging tests and **314 native CLI unit tests** pass on exact
**`7c39feef3a3282831522ea226708e6aee8ff847b`**. State **ACCEPTED for the helper
paging contract**. Exact paths and evidence are in
`worklog/V2-NATIVE-TRANSCRIPT-PAGING.md`. Unicode requires renewed preverification
against this repaired canonical base before acceptance.

### Native Unicode accepted on exact paired installed release

Source-only UTF-8 candidate `cfe6cbf` was prepared against repaired base
`3086fc1` without changing its product bytes. Independent verifier
`ses_f0b3f6f38ffe7tuTisQyuMpbX9` PREVERIFIED all required installed and native
regression gates. After fast-forward integration, the controller repeated the
entire Mac build/archive/install/gate sequence on exact
**`a79ae96b93f9a0ab94ceafadcd9425db08f28027`**, then built/archived/installed
and verified Ubuntu ARM64 at that same SHA. Both pass two-request fragmented
Unicode/backspace/restart, four-request stream/tool/second-turn/restart,
two-request onboarding, eight auth controls and original PTY restoration.
Mac also passes 314 CLI units, 73 bridge + ten lifecycle checks, four daemon
flows and two server streams. Ubuntu passes ten packaging controls, relative
`$ORIGIN/../lib`, adjacent native loader closure and ten ABI exports.

State **ACCEPTED for scoped native valid UTF-8 input and paired installed
release regressions on exact `a79ae96`**, independently confirmed from actual
receipts. Full command/hash/path evidence is in `worklog/V2-NATIVE-UTF8-LIVE.md`.
The earlier same-SHA Mac/Ubuntu/Web streaming acceptance at `de7e05f` remains
preserved; this Unicode acceptance does not certify full G0–G8.

Fresh serial workspace gates on exact installed `a79ae96` retain actual results
at `v2-workspace-gates-a79ae96-llxq2uon` under the approved artifact parent:
formatting passes; Clippy exits 101 at tools-library and **19 session-library
diagnostics**; all-feature/all-target tests compile and run **807 passed,
1 failed, 0 ignored** before Cargo stops. The current failure is
`ci_ext_t03_max_steps_one_completes_like_baseline`, receiving usage exit 64
instead of success or budget-exhausted. This is not a full workspace pass.

### Shell compiling RED frozen; session diagnostics accepted

The strict independent shell lifetime fixture is integrated as `59dc523` and
frozen after actual current-canonical compiling RED: three controls pass;
aborted and timeout owners leave live direct-child PIDs after their deadlines.
Forced failure cleanup is recorded and subsequent controller probes prove the
owned PIDs absent. Fixture SHA-256 is
`8b39c6eafcbb065e42752c8c1d19bac53be6928612fc59b1da654465353097fd`;
full authority/command/hash evidence is in
`worklog/V2-SHELL-OWNED-LIFETIME-CONTRACT.md`.

Source-only session-library repairs were independently PREVERIFIED, then
fast-forward integrated and rerun on exact
**`0f01432793a599b312226b522c479a757ab34f5e`**. Formatting and session-library
Clippy pass; all **300 all-target/all-feature session tests** pass with zero
ignored. Existing constructor API, zeroizing secret drop, SQL and all frozen
test bodies are preserved. State **ACCEPTED for scoped session maintenance**;
exact paths/receipts are in `worklog/V2-SESSIONS-LINT.md`.

The combined tools/owned-shell candidate `35779c7` independently passes
formatting, tools-library Clippy and all five strict owned-process cases, then
fails its required full crate gate: 122 pass, one pre-existing MCP fixture
fails with `ENOENT`. The controller reproduces that failure on clean canonical
`89aeaa8` source. `/bin/false` is absent on macOS; independent fixture maintenance
changes only its authored pathname to `/usr/bin/false`, retaining every retry,
typed-error and no-silent-restart assertion. The combined candidate remains
unaccepted pending complete renewed verification.

During CI fixture preparation, two worker-owned files were accidentally written
to the canonical working directory. The controller moved that worker session
to its granted worktree, preserved all unexpected bytes in a fresh artifact
snapshot and append-only stash, and anchored them at
`archive/v2-canonical-ci-fixture-18cb219b96e9` (stash
`18cb219b96e91c69281805c6028324d93bfc1b3f`). No historical ref or stash was
removed. Canonical source was clean before the accepted session integration.
Preservation receipts: `v2-preserved-canonical-ci-fixture-e2o9nmxt` under the
approved artifact parent. CI assertions remain protected; fixture readiness
repair is not yet product or workspace acceptance.

### Owned shell children and tools diagnostics accepted

After the one-literal MCP fixture-path correction, the current combined tools
candidate was independently PREVERIFIED against accepted session base `a6b188e`.
After fast-forward integration, all required commands were repeated on exact
**`e1acd5258fed50b1f79f54b412921266f07dc8ed`**: formatting and tools-library
Clippy pass; the strict real-process target passes all five cases; all **458
full all-target/all-feature tools tests** pass with zero ignored. Focused and
full logs contain no forced cleanup.

State **ACCEPTED for Mac-verified library direct-child ownership, tools
diagnostics and MCP fixture maintenance on exact integrated `e1acd52`**.
Exact path/hash/authority evidence is in the three companion worklogs; receipts
are `v2-tools-owned-preverify-e1acd52-swt6njq0` and
`v2-tools-owned-integrated-e1acd52-mg5gxwlw` under the approved artifact parent.
This clears the recorded tools-library failures; native interruption, Ubuntu
execution of the new process contract, the CI fixture and fresh workspace/
installed-release journeys remain open.

### Authenticated CI fixture accepted

The independent evaluator classified the earlier `ci_ext_t03` exit 64 as a
missing-authentication/readiness fixture defect. The independent test owner and
controller prepared a real loopback provider, actual authenticated daemon,
offline advertised model and bounded owned-process/drain cleanup. All six test
function bodies remain byte-identical, including the success/budget exit check.
Source-only originals and intermediate failures remain preserved.

Independent verification passed on exact
**`8563c4d6442e496573b35dbef6e117ebce938ba5`**; after fast-forward integration,
the controller repeated the unchanged gates on that same SHA. Formatting and
all **12 CI-extension tests** pass, zero failed/ignored, with no provider or
cleanup failures. State **ACCEPTED for scoped mechanical CI fixture readiness**.
Detailed path/hash/authority evidence is in `worklog/V2-CI-EXT-FIXTURE.md`;
receipts are `v2-ci-ext-preverify-8563c4d-p_kqm01h` and
`v2-ci-ext-integrated-8563c4d-hdlip8ez` under the approved artifact parent.

The source investigator additionally confirms the remaining live session-owner
gap: the actual HTTP turn handlers use a global two-slot semaphore rather than
pinned V2's per-session `SessionRunCoordinator`, and do not wire the existing
cancellation state machines into an authenticated interrupt operation. This is
research evidence, not a frozen runtime RED or acceptance; a separate live
two-client ownership gate is required.

### Fresh workspace agents-library diagnostics accepted

Fresh formatting passes on `af3c8ca`; full workspace Clippy advances beyond
tools/sessions, then records three agents-library diagnostics. Their equivalent
source repairs were independently PREVERIFIED and integrated as exact
**`72e5aa499b124937ed8e193ca0f0d2d9689b5122`**. Formatting, agents-library
Clippy and all **126 agents crate tests** pass on the integrated revision,
zero failed/ignored. State **ACCEPTED for scoped library maintenance**;
`worklog/V2-AGENTS-LIB-LINT.md` records paths, preserved tests and receipts.

The controller also rebuilt, archived and installed native Mac release
`af3c8ca` before the agents integration. Frozen Unicode, four-request stream/tool
continuation, onboarding, eight auth controls, PTY restoration, four daemon
flows, 314 CLI units, 83 bridge/lifecycle tests and two server streams pass.
Evidence is retained at `v2-native-workspace-repairs-af3c8ca-cye52df1` under the
approved artifact parent. Independent auditing is pending; that artifact does
not attest the later `72e5aa4` source or full release readiness.

### Catalog library maintenance accepted

Fresh workspace Clippy on `364c92d` exposed three catalog-library diagnostics.
Their equivalent rewrites in `config.rs` and `health.rs` were independently
PREVERIFIED and fast-forward integrated as exact
**`97774583c6b008581fb26cc85a11b74b9d0692a1`**. The unchanged focused manifest
passes on the integrated revision: formatting, library Clippy and **42 crate
tests**, zero failed/ignored. Protected test bodies, latest-record health
behavior and Cargo.lock remain unchanged. State **ACCEPTED for scoped catalog
maintenance**; `worklog/V2-CATALOG-LIB-LINT.md` records the exact evidence.

### Installed Mac audit and subsequent workspace timeout

Independent verifier `ses_f0a8218c7fferXW1tN7e3tegZ9` audited the retained
Mac release `af3c8ca1989868110f2de4db0cc924fc712406ae`, recomputed all artifact
and command/log hashes and inspected observed PTY, provider, durable history,
authentication and cleanup receipts. State **ACCEPTED for the scoped Mac ARM64
installed regression on exact integrated `af3c8ca`**. The 11-command evidence
is `v2-native-workspace-repairs-af3c8ca-cye52df1`. This supersedes the pending
audit note above, not the same-current-SHA Mac/Web/Ubuntu release gate.

The later exact `364c92d` workspace run used an attested installed native binary
from `v2-native-workspace-tests-364c92d-fqin_b5i`. It completed **808 passing
tests, zero failures/ignored**, including all 12 repaired CI-extension tests,
then reached the 300-second deadline in `ci_mode::ci_t01`. The original provider
fixture blocks in `join()` after CI fails closed without a daemon bearer.
Receipt and timeout logs are `v2-workspace-tests-current-364c92d-70_oode8`;
a bounded process-table probe confirmed the exact hung test executable absent
after the runner's SIGTERM. This is an incomplete workspace run, not GREEN.
The independent fixture owner is preparing bounded authenticated readiness with
the three original test bodies and assertions preserved.

### CI-mode fixture and foundation maintenance accepted

Independent source review corrected CI-mode helper preparation while preserving
all three original test bodies byte-for-byte. Independent preverification and
the identical exact integrated rerun pass on
**`3abcc879dd68f5cf7df413d3118d03785f86fca2`**: formatting and all **9 CI-mode
tests**, zero failed/ignored and no provider/cleanup errors. State **ACCEPTED for
CI-MODE-AUTH-READINESS**. `worklog/V2-CI-MODE-FIXTURE.md` records source-only
preservation and receipts; product authentication and exit/determinism assertions
are unchanged.

Fresh workspace Clippy then records three foundation diagnostics. Equivalent
source-only repairs were prepared on the CI-mode integration, independently
PREVERIFIED and fast-forwarded as exact
**`6a71db3898abde832ff4ccce023a80f3bff252f9`**. The unchanged integrated
manifest passes formatting, foundation-library Clippy and all **168 foundation
tests**, zero failed/ignored. State **ACCEPTED for scoped foundation maintenance**;
`worklog/V2-FOUNDATION-LIB-LINT.md` records the retained root allocation,
overflow-denial behavior, protected tests and exact receipts.
