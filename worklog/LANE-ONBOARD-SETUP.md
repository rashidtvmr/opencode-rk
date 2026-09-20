# LANE-ONBOARD-SETUP — scratchpad

Claim: LANE-ONBOARD-SETUP, session ses_f423cd0e5ffeeHMozSNUJfQ3zq. Own: crates/cli/src/onboarding.rs only.

Source evidence:
- crates/cli/src/onboarding.rs:424-586 OnboardingSession state machine (Welcome->ProviderSelect->CredentialEntry->ModelSelect->Done).
- crates/cli/src/onboarding.rs:687 start_offline helper only; no interactive driver exists (grep run_interactive = 0 hits).
- crates/cli/src/app_start.rs:295-321 plan_default_launch routes creds None/Some(false) to StartupView::Setup; main.rs:216-255 never branches on view (always chat) — main.rs NOT owned, wiring left to orchestrator.
- Repo rev 6b19524 matches task context.

Observed scenario: Setup view has no driver; need pub run_interactive driving session to Done, opening on missing creds, bounded steps.

Target boundary: add MAX_INTERACTIVE_STEPS + InteractiveOutcome + run_interactive + drive helper in onboarding.rs; 5 new tests in existing mod tests. No main.rs, no test edits to frozen 8.

Tests: T01 opens-setup None/Some(false)->Done+committed+steps<=MAX; T02 skips Some(true)->None+store empty; T03 bad credential->Err+no residue; T04 bad model->Err+no residue; T05 steps==4<=MAX.
RED sha: 3d67c6706a87f7c39d2351ad47b432f557e53450d65375f3ccfc605d2fb2b4d9 (RED-LANE-ONBOARD-SETUP.rs; 9 pass/4 fail, fails = 4 interactive_* missing-driver). RED sha: 3d67c6706a87f7c39d2351ad47b432f557e53450d65375f3ccfc605d2fb2b4d9 (9 pass/4 fail, fails=4 interactive_* missing-driver). GREEN sha: ef30545b5e63fac97d7f7f0f315e5b5da98a783f571cf1d5992947fdac4c8295 (13/13 GREEN, zero warnings). Evidence: /tmp/opencode/RED-LANE-ONBOARD-SETUP.rs + /tmp/opencode/GREEN-LANE-ONBOARD-SETUP.rs.

Decisions: secrets as &SecretString (no raw bytes in errors); failure path cancels session (no staged residue); creds Some(true)->Ok(None).
Verify cmd: CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 110 cargo test -p opencode-rk-cli --bin oc2 onboarding => 13 passed 0 failed (289 filtered). Stale build-script cache (CARGO_MANIFEST_DIR=/tmp/opencode/tui-paint-verify) fixed by touch crates/opentui-bridge/build.rs. Unrelated lanes touched ci_run.rs+tui_entry.rs -> stashed (not owned), untracked native_daemon_flow.rs + .opencode/* left alone.
