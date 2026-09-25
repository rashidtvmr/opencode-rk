# MAC-INSTALLED-RED scratchpad

Claim: MAC-INSTALLED-RED, session ses_f3c4de578ffelQv59xDXmOs03B.

Base: `7262682e31c1f473912c9483804c9430eb4ee4c5` on
`red/PHASE1-MACOS-INSTALLED`.

Source evidence:

- Current code, `crates/cli/src/main.rs:220-258`, computes the real
  no-subcommand launch plan, but the `NativeTui` arm calls `chat::run` unless a
  user supplied the non-default `--native` flag.
- Current compatibility frontend, `crates/cli/src/chat.rs:47-85,98-108`, prints
  `type /help for commands` and owns/kills an auto-spawned daemon on exit.
- Frozen disputed test `crates/cli/tests/installed_default_entrypoint.rs` on this
  baseline contains `todo!()` placeholders and is immutable; this lane does not
  edit or use it as proof.
- Synthesis stage `MAC-INSTALLED-RED` at commit `7262682` assigns the new test
  path `crates/cli/tests/phase1_macos_installed.rs` and later implementation to
  the serialized `crates/cli/src/main.rs` owner.
- Product map `worklog/PHASE1-VERTICAL-PRODUCT-MACOS-MAP.md:86-102` requires a
  fresh disposable HOME, real macOS PTY, no-subcommand native setup route,
  bounded capture/timeout, daemon cleanup, and unsigned artifact honesty.

Target observable contract: the real `oc2` binary, under `/usr/bin/script` with
no subcommand and a fresh disposable HOME/data root, enters the native setup
view and never displays the compatibility line-chat marker. Capture must be
capped, startup/exit bounded, environment allowlisted, and teardown restricted
to the disposable authenticated-daemon descriptor.

Target boundary: one macOS-only compiling behavioral RED. It would not claim an
archive install, signing, provider turn, or full APP-012 journey; those remain
later E2E stages.

## Blocker receipt

- A bounded real-PTY test was drafted but **not frozen**. Both the requested
  `--features native` command and the same focused target without an explicit
  feature failed during dependency build, before any test executed.
- Exact attempted command:
  `OC2_BUILD_REVISION=7262682e31c1f473912c9483804c9430eb4ee4c5
  CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli
  --test phase1_macos_installed -- --test-threads=1` (also attempted with
  `--features native`).
- Cause: `crates/cli/Cargo.toml:40-42` enables the native bridge for every CLI
  integration test via its dev-dependency. Baseline `7262682` build script
  `crates/opentui-bridge/build.rs:3-23` requires `libopentui.a` or
  `libopentui.so`, but the assigned baseline contains no
  `native/lib/aarch64-apple-darwin` artifact. The build panics before compiling
  this test.
- The drafted test was deleted because compile failure is not RED. No SHA-256
  was frozen and no product/existing-test/manifest/library file changed.
- Separate evidence: product-spine revision `5d66683` includes a revised native
  build gate, arm64 artifacts, and a five-scenario frozen
  `installed_default_entrypoint` suite reported GREEN. Therefore authority must
  either rebase this stage onto the approved artifact/prewire revision and
  classify the behavior as existing, or define a genuinely missing additive
  contract. It must not duplicate or edit the disputed frozen test.

Status: BLOCKED on prewire/base selection. No implementation or acceptance is
authorized.
