# TOOL-SHELL-BROKER-FULL-GREEN-VERIFY — verifier worklog

- Claim: TOOL-SHELL-BROKER-FULL-GREEN-VERIFY, session ses_f2ccf9035ffekU39eHBRc323Lw, status blocked (see ledger row).
- Target revision: b66ee38 (TOOL-SHELL-BROKER-REGISTRY-GREEN). Parent ed4164d (executor gate). Pre-gate baseline 2c5e9cf.
- Role: independent verifier. No source/test repair, integration, or acceptance. Later RED author and implementer remain separate.
- Governing refs: AGENTS.md, docs/TDD.md, docs/SECURITY.md.

## Frozen suite

- File: crates/tools/tests/phase1_shell_broker.rs
- Hash: ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8 (reproduced via shasum, unchanged).
- Result: 3/3 GREEN (`cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=2`):
  - t01 shell_without_broker_denied_before_spawn ok (no marker, denial text)
  - t02 denial_leaves_no_marker_and_no_secret ok (no marker, no sentinel leak)
  - t03 dispatcher_without_broker_denies_shell_no_store_write ok (store (0,0), permits intact)

## Source matrix (deny-before-effect)

- executor.rs:122-156 generic `execute()` denies bash/shell pre-spawn with fixed string "shell execution denied: broker authorization required"; no input echo; `execute_batch` (:417-460) routes via `execute()` so batch shell denied too.
- executor.rs:158-178 `execute_authorized` routes shell via broker; :270-326 `execute_shell_authorized` validates command/cwd, authorizes Process intent (Allow only), spawns post-Allow with env_clear + fixed PATH.
- registry_dispatch.rs:43-44 fixed SHELL_DENIED_NO_BROKER (no command/input/env/identity).
- registry_dispatch.rs:181-219 `dispatch()`: empty/input/unknown/disabled/policy-deny checks, then `is_shell_alias` gate BEFORE permit/executor/store; returns `shell_denied_record` Ok(false).
- registry_dispatch.rs:229-343 `dispatch_batch()`: shell alias resolves Immediate Ok(denied), never spawned, never store-written.
- registry_dispatch.rs:360-384 `is_shell_alias` (id bash|shell|/bin/bash|/bin/sh or name bash|shell) + `shell_denied_record` (no input echo).
- Non-shell (echo) path unchanged; AllowAll boolean never becomes process authority for shell aliases.
- Security checks: no input echo/side effects on denial; no secret in loggable output (no RED-SENTINEL string in executor.rs/registry_dispatch.rs); no surviving test child; no /tmp markers left.

## Registry filter + full lib counts

- `cargo test -p opencode-rk-tools registry_dispatch::tests --lib`: 6 passed, 1 failed.
  - FAIL disc103_t05_batch_respects_max_parallel_bound, panics at registry_dispatch.rs:597 `batch item must dispatch: Unknown("slot-0")`.
- `cargo test -p opencode-rk-tools --lib`: 107 passed, 4 failed:
  - executor::tests::execute_success (executor.rs:489, assumes unbrokered shell spawn succeeds)
  - executor::tests::execute_timeout (executor.rs:510, assumes shell timeout error)
  - mcp_spawn::tests::disc111_t04 (SpawnFailed missing binary, environment)
  - registry_dispatch::tests::disc103_t05 (above)
- Pre-gate baseline 2c5e9cf full lib: 110 passed, 1 failed (only mcp env). So executor x2 + registry t05 failures appear only after shell gating; they are stale-intent conflicts, not random regressions.
- `git diff --check`: clean.
- Classification: executor x2 stale (obsolete spawn assumptions under broker gate); mcp env pre-existing; t05 stale-intent PLUS new implementation defect below.

## Blocking defect: dispatch_batch Immediate-result loss

- Location: crates/tools/src/registry_dispatch.rs:278-279 spawn-extraction loop.
- Mechanism: `if let Some(Ready::Spawn {..}) = slot.take()` takes EVERY slot; non-Spawn (Immediate) variants are dropped to None. Rebuild loop :324-339 then matches None without spawn result to `_ => Err(Unknown("slot-{idx}"))`.
- Effect: every fail-closed batch item (Unknown/Disabled/Denied/InputTooLarge/EmptyName) and every shell-denied Immediate resolves as misleading `Unknown("slot-i")` instead of its exact error/denied record. Breaks single/batch parity.
- Reproduction: existing disc103_t05 at b66ee38 fails with `Unknown("slot-0")` because its 4 bash batch items are now shell-denied Immediates, all dropped by take(). Any batch with a denied/unknown item reproduces; no new test authored (verifier boundary).
- Repair direction (for later implementer, not done here): preserve Immediate slots instead of take-then-drop (e.g. only take Spawn arms, or drain Immediates into output map before spawn loop); add batch fail-closed parity test (unknown/denied/shell each map exactly, order kept, no store write, permits intact).

## Verdict

- BLOCKED for full acceptance. Frozen 3/3 GREEN for correct reasons confirmed, hash unchanged, zero test edits, no new crate regressions beyond the classified conflicts. Full-GREEN acceptance blocked by dispatch_batch Immediate-loss defect + 3 stale tests needing refreeze/update in separate lanes. Do not mark completed; do not repair here.

## Resource observations

- Prior runs sequential: CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2, --test-threads=2. No parallel heavy jobs. No surviving child processes observed. No memory pressure noted. This landing step: text/Git only, no Cargo rerun.
