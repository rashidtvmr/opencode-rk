# APP-001-INSTALLED-SETUP-REPAIR — setup state wiring in main.rs

## Claim
- Task ID: APP-001-INSTALLED-SETUP-REPAIR
- Session: ses_f1eb9f800ffezgDdKnssZ436de
- Owned file: crates/cli/src/main.rs
- Route allowlist: @vyce-deepseek-v41 (on allowlist)

## Source evidence (commit 7938791)
- crates/cli/src/main.rs:224-268 — `run()` None arm: computes
  `plan_default_launch` but the non-native branch calls
  `chat::run(&data)`; `StartupView::Setup` is never consumed.
- crates/cli/src/app_start.rs:337-346 — `needs_setup` predicate and
  `setup_message()` sentence exist; only route to the view.
- crates/cli/src/app_start.rs:268-320 — `StartupView::Setup` produced by
  `plan_default_launch` when `creds_configured` is `Some(false)` or `None`.
- crates/cli/src/onboarding.rs:48-68 — `SetupStep` ordering
  (Welcome -> ProviderSelect -> CredentialEntry -> ModelSelect -> Done).
- crates/cli/src/onboarding.rs:348-398 — `MemoryAccountStore`: in-memory,
  no disk persistence of credentials (safe; disposable).
- crates/cli/src/onboarding.rs:406-586 — `OnboardingSession` state machine
  with `begin`, `advance_from_welcome`, `select_provider`,
  `submit_credential`, `select_model`, `cancel` (all pub).
- crates/cli/src/daemon_client.rs:776-791 — `creds_configured` env-var
  list: OPENAI_API_KEY, ANTHROPIC_API_KEY, GOOGLE_API_KEY, GEMINI_API_KEY.
  Provider ids derived: openai, anthropic, google, gemini (>=2 required
  by frozen test `shows_provider_selection_state`).

## Observable contract
- Fresh disposable HOME + data dir, no provider credential env vars.
- `oc2 --data-dir <disposable>` under PTY: first frame must render a
  provider-selection state: a line containing "provider" without "model:",
  plus >=2 of {openai,anthropic,google,gemini,azure,ollama,local}.
- Typing "openai" (+Enter) must advance to a credential-entry state:
  frame contains "api key"|"apikey"|"credential"|"secret"|"token".
- No real credentials written to disk (MemoryAccountStore only).
- Output bounded (<32 KiB per frozen fixture). No orphan daemon (test
  points PTY env OPENCODE_RK_DAEMON_ADDR at loopback health fixture).

## Target boundary
- Only crates/cli/src/main.rs edited. onboarding.rs, chat.rs,
  daemon_client.rs, app_start.rs read-only.
- Real state-machine driving via onboarding::OnboardingSession, not a
  printed fake string.

## Tests (frozen, immutable)
- crates/cli/tests/installed_setup_flow.rs — SHA256
  be2540694a08354e4ab16a6d7e69d4417875a1c02e130664f6001ea62fa7093c
- RED confirmed: 1 pass (T3 bounded teardown), 2 fail (T1/T2 setup
  states missing). Compiled standalone with `rustc --test` + OC2_E2E_BIN.

## Decisions
- Provide an interactive `run_setup` in main.rs driven by
  `onboarding::OnboardingSession` + `MemoryAccountStore`.
- Provider list sourced from the real `creds_configured` env-var set
  (openai/anthropic/google/gemini) — not fabricated.
- On Setup view, bypass `chat::run` and enter the setup loop.

## Remaining unknowns
- Whether native (native_interactive_loop) path also needs a setup
  branch (blocked on macOS: no vendored libopentui; focus on
  non-native path which the PTY test exercises).

## Audit (post-implementation)

### Production contract findings
1. **`MemoryAccountStore` dropped on exit (by design)**: `run_setup`
   holds a local `MemoryAccountStore` that is dropped when the function
   returns. No credential is written to disk, honoring the sandbox
   security policy. Frozen tests do not cover cross-launch persistence;
   this is the intended posture, not a defect in scope.

2. **`daemon_client::creds_configured` reads env only**: returns
   `Some(false)` when no provider API key env var is set. After setup
   completes, the entered credential lives in-memory only and is dropped
   with `run_setup`. The next launch sees `Some(false)` again and
   re-enters setup. **This is a durability gap** — the provider client
   cannot consume the entered key because no persistence API exists
   within main.rs's brokered surface.

3. **Empty model default was unreachable (FIXED)**: The original loop's
   `if input.is_empty() { continue; }` at the top fired before the
   `match session.step()`, making the `if input.is_empty()` branch inside
   `ModelSelect` dead code. Fixed by moving the empty-input guard into
   each match arm: ProviderSelect/CredentialEntry re-prompt on empty
   input, while ModelSelect falls through to the default model. This is a
   minimal, safe correction within main.rs.

### Durable credential consumption feasibility
**Not possible within main.rs via existing public brokered API.**
- `AccountStore` trait (onboarding.rs:348-354) only tracks staged/committed
  account existence; it does not store credentials.
- `MemoryAccountStore` (onboarding.rs:360-398) is in-memory only.
- No durable `AccountStore` implementation is accessible from main.rs.
- `daemon_client::creds_configured` (daemon_client.rs:776-791) reads env
  vars only; no write path exists.
- The daemon's authenticated descriptor (publish/read via
  `publish_backend_descriptor_with_auth` / `read_backend_descriptor`)
  carries the daemon's own bearer token, not provider API keys.

Per task constraints, this cross-file durability gap would require a RED
test and lease on `daemon_client.rs` (to expose a credential-write path
or a durable `AccountStore` backend), which is out of this lane's
single-file ownership. Marking the durability aspect BLOCKED; the frozen
tests pass with the current (in-memory) implementation.

## Test evidence
- Frozen test: crates/cli/tests/installed_setup_flow.rs
  SHA256 be2540694a08354e4ab16a6d7e69d4417875a1c02e130664f6001ea62fa7093c
  (verified: `sha256sum` match)
- Compiled standalone: `rustc --test --edition 2021 -o /tmp/test_installed_setup
  crates/cli/tests/installed_setup_flow.rs` (no errors)
- Ran with `OC2_E2E_BIN=target/debug/oc2`:
  3 passed; 0 failed in 30.26s (T1 ok, T2 ok, T3 ok)
- Binary rebuilt post-fix: `cargo build -p opencode-rk-cli --bin oc2` —
  compiles cleanly (only pre-existing warnings)
- Re-ran frozen tests post-fix: 3 passed; 0 failed in 30.26s

## Status: completed (frozen tests green; durability gap documented as BLOCKED)

