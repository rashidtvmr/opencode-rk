# APP-001-SETUP-REPAIR-RED

## Claim

- Task: `APP-001-SETUP-REPAIR-RED`; type `test-author`; role `worker`; UNFROZEN RED draft.
- Assigned route: `9router-th-dsv41-flash-free`; user allowlist; route/model confirmed against prompt (`9router/th/deepseek-v4.1-flash:free`).
- Owned files: `crates/cli/tests/installed_setup_flow.rs`, this scratchpad, own ledger row. No other path written.
- Working dir: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/repair-app001-setup`; branch `red/APP-001-SETUP-REPAIR`; base `7938791ba76645115e418928a728869b48beb73b`. Confirmed via `rtk git rev-parse --show-toplevel` / `--branch --show-current`.
- Claim session: `ses_f22bd809bffe0VDwgKNqNQEYwQ` (this run). Claimed via `cc.claim` AFTER a read-only ledger scan; no collision with the distinct foreign row `APP-001-REPAIR-RED` (different task id, session `ses_f2ec4da52ffeqvBmjXifr2Bx28`).
- Prior author `ses_f22ca19e2ffeJTDbTkdvUQolyU` stopped; its draft had no test file on disk (only the scratchpad). Orchestrator reclaimed its root-ledger claim; this worktree ledger had no live row for this exact id until my claim.

## Intake and gates (read-only)

- Source SHA: `7938791ba76645115e418928a728869b48beb73b`.
- `python3 tools/convergence_gate.py`: exit 1; `total=83` pre-existing off-plan/completion-accounting findings. No task file changed by the gate.
- `python3 tools/validate_repository.py`: blocked by pre-existing backlog-exhaustion/classification drift (per prior scratchpad and repo-wide state). No task file changed.
- `tools/lane_gate.py` targets storage lanes only; it does not gate this file.
- No competing `cargo`/`rustc` process at intake (`pgrep -fl 'cargo|rustc'` empty). Host `aarch64-apple-darwin`, rustc/cargo 1.98.1.
- `timeout`/`gtimeout` are absent on this macOS host; used bounded child waits inside the test instead.

## Source evidence (all at base 7938791)

- `crates/cli/src/main.rs:224-268` (`run`, `None` arm): computes `plan_default_launch`/`needs_setup` but the non-native build calls `chat::run(&data)`; `StartupView::Setup` is never consumed, so missing credentials do not open setup.
- `crates/cli/src/chat.rs:134-155` (`run`): prints banner + `[offline] ... serve` hint, enters line loop. No provider/credential state.
- `crates/cli/src/chat.rs:447-479` (`Chat::loop_until_exit`): a plain line is a chat turn; `openai` becomes `text => self.send_turn(text)` -> `[error] no session; run /new first`. No setup transition.
- `crates/cli/src/onboarding.rs:48-74` (`SetupStep`) and `crates/cli/src/native_host.rs:100-129`: real ordered state is `Welcome -> ProviderSelect -> CredentialEntry -> ModelSelect -> Done`.
- `crates/cli/src/app_start.rs:287-346` (`plan_default_launch`/`needs_setup`/`setup_message`): a printed sentence/plan is not interactive proof; `setup_message` must not pass the test.
- `crates/cli/src/daemon_client.rs:776-791` (`creds_configured`): returns `Some(false)` when none of `OPENAI_API_KEY`/`ANTHROPIC_API_KEY`/`GOOGLE_API_KEY`/`GEMINI_API_KEY` is set; `env_clear` in the test guarantees this.
- `crates/cli/src/chat.rs:59-64,94-121` (`prepare_daemon`): probes `OPENCODE_RK_DAEMON_ADDR` `/health` first; a healthy loopback fixture prevents any real daemon spawn (no orphan process).

## Observable contract and failure states

Fresh disposable HOME+data, no provider credential, real `oc2`, macOS `/usr/bin/script` PTY, owned loopback `/health` fixture:

1. Process starts within 15s. Missing binary is reported as a setup failure (explicit message), never a behavioral pass.
2. Output bounded to 32 KiB; overflow fails the test (sticky flag).
3. Initial frame exposes a provider-selection state (a non-`model:` line containing `provider` plus at least two provider identities), not only `OpenCode RK`/`model:`/offline prose/trace.
4. Test sends only a provider id (`openai\n`); no credential or secret.
5. After that input the frame exposes a credential-entry state (`api key`/`credential`/`secret`/`token`), proving an actual transition. The test first asserts the credential state was absent, so the transition is non-vacuous.
6. `setup_message()` text, the chat banner, and `model: openai/...` alone can never satisfy (3) or (5).
7. Teardown uses only the owned `script` Child handle; no PID signalling, no host-destructive commands.

Fixture ownership/lifetime: `TestState` owns and recursively removes only its fresh temp HOME/data. `HealthFixture` owns one loopback listener thread (bounded `HEALTH_MAX_CONNS=64`, non-blocking, stopped on drop). `PtyClient` owns the exact `script` child + stdin/stdout/stderr and two reader threads; `Capture` is a fixed 32 KiB ring with an overflow flag; `shutdown` sends `/exit`+`:q` and waits up to 15s, killing only the owned handle if the bound elapses.

Security/resources: `env_clear` then explicit `HOME`, `PATH`, `LANG`, `LC_ALL`, `TERM`, `OPENCODE_RK_DAEMON_ADDR`; no inherited secrets, no network beyond loopback, no shell-string concatenation, no arbitrary PID, no user data. One child + two reader threads + fixture thread; 32 KiB retained bound; 15s waits.

## RED plan and result

Test both compiles (std-only root target) and fails for the missing behavior, not for a missing executable.

- Compile: `rustc --edition 2021 --test crates/cli/tests/installed_setup_flow.rs -o <tmp>` -> exit 0, one warning fixed (unused import removed before freeze).
- Run: `OC2_E2E_BIN=$PWD/target/debug/oc2 <tmp> --test-threads=1` -> `test result: FAILED. 1 passed; 2 failed`. T1 and T2 fail on the missing setup state; T3 (bounded capture + owned teardown) passes.
- Captured T1 frame: `OpenCode RK` / `daemon: http://127.0.0.1:<port>` / `model: openai/gpt-5.6 | ...` (no provider/credential state). T2, after `openai`, shows `openai` then `[error] no session; run /new first` (proves the provider id was treated as a chat turn).

## Decisions

- The test is std-only so it compiles as a root integration target with no new dependency. It prefers `OC2_E2E_BIN`, else `option_env!("CARGO_BIN_EXE_oc2")`.
- Per-task constraint: provider id only, never a credential; loopback `/health` fixture keeps daemon startup owned; no PID kill (same-process teardown only).
- Did not attempt to build or vendor native artifacts (out of owned scope; base lacks them).

## Remaining gaps / exact blocker

1. Missing behavior (the RED's target): the default TTY missing-credential launch on `main.rs:224-268` still routes to `chat::run` and never consumes `needs_setup`/`StartupView::Setup`. An implementer must wire the real interactive onboarding/`SetupStep` UI (provider select -> credential entry) into the no-subcommand TTY route.
2. Native artifact gap (pre-existing, outside this lane): base 7938791 has no `crates/opentui-bridge/native/lib/aarch64-apple-darwin/` artifact (only `x86_64-unknown-linux-gnu/libopentui.so`). `crates/cli/Cargo.toml` dev-dependencies force `opencode-rk-opentui-bridge` with `features=["native"]`; `build.rs` panics "native libopentui artifact missing for target 'aarch64-apple-darwin'". Therefore `cargo test -p opencode-rk-cli --test installed_setup_flow` cannot build on this host (verified: `cargo check -p opencode-rk-opentui-bridge --features native` and `cargo test -p opencode-rk-cli --test installed_default_entrypoint --no-run` both panic in the bridge build script). The bin `oc2` itself builds without the native feature (`cargo build -p opencode-rk-cli --bin oc2` -> Finished), which is how the behavioral RED was exercised.
3. The frozen verifier must either (a) run the frozen test through the rustc harness with `OC2_E2E_BIN`, or (b) land the aarch64-apple-darwin artifact (lane/APP-001-REPAIR-RED carries it) so the cargo test target builds.

## Hash / manifest (frozen)

- Test file: `crates/cli/tests/installed_setup_flow.rs`, 460 lines, sha256 `be2540694a08354e4ab16a6d7e69d4417875a1c02e130664f6001ea62fa7093c`.
- Compile cmd: `rustc --edition 2021 --test crates/cli/tests/installed_setup_flow.rs -o /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/oc2_setup_flow_test`
- Run cmd: `OC2_E2E_BIN=$PWD/target/debug/oc2 /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/oc2_setup_flow_test --test-threads=1`
- Bin build cmd: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo build -p opencode-rk-cli --bin oc2`