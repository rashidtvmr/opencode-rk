# V2 integration evidence — 2026-09-30

## Integrated revisions

`0759e01464a7a0dbed614163a14b6f60673e2412`, pushed to `origin/main-v2`.
Native lifecycle package: `425d617a3576c791e7027b2cc332a75e7441fd4e`.
Typed live writer/restart package: `31ea2b0d39e4cf23e8eae5f894763edf88678cb9`.
Browser transport: `25b64488cfc4282b867a924570ba267fbcd46c32`.
Persisted API auth and rooted tool continuation: `f5cfb012369f3a4187cfa4f50ad01eb4051081c6`.
Installer native closure/transaction: `14ff5fb4d93b25ab1c8e6ccc64603b2dbf3ea9a9`.
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

## Remaining observed product failures

- G2 persisted OpenAI API auth is connected and accepted for the scope above;
  in-app provider setup, OAuth, and provider adaptation beyond OpenAI remain open.
- G3/G4 settled typed history now passes the scoped live writer/restart gate;
  interruption recovery and second-client ownership still require golden-journey
  validation.
- Native renderer lifecycle is accepted as above; the complete native CLI golden
  journey still requires G1–G4 integration.
- Docker server is ready. Ubuntu 24.04 arm64 image acquired at digest
  `sha256:008173c23f95b170204355c12626cb5a965d779a7e1283b09e9cffbb1bf33ca3`.
  Host-profile installer closure/transaction is accepted as recorded above.
  Ubuntu packaging and its real native golden journey remain unrun.

These component acceptances are not G0–G8 release acceptance.
