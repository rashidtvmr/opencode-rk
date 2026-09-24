# TOOL-RED-SHELL-VERIFY scratchpad

## Claim
- Task `TOOL-RED-SHELL-VERIFY`, session `ses_f2dd51d07ffe8n7qJum4Kh4DPT`, scratchpad `worklog/TOOL-RED-SHELL-VERIFY.md`.
- Role: independent RED verifier. No production code, no test edit, no merge.

## Verdict
`ACCEPT WITH SPLIT`. Candidate frozen RED is authentic, compiling, fails for
intended missing behavior, hash-exact, secret-clean, side-effect-clean. Four tests
are NOT one coherent fix: t01-t03 = broker authorization default-deny contract;
t04 = process cancellation (kill-on-drop/reaping) contract, and t04 as written is
unsatisfiable by the broker fix (see Unresolved gaps).

## Candidate under verification
- Ref `origin/lane/TOOL-RED-SHELL` == `2cd9ca9987bed99adcff0d1bee0038f7f1b2c3c5`.
- Base `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b` confirmed ancestor of `2cd9ca9`.
- Changed files (scope audit): `A crates/tools/tests/phase1_shell_broker.rs`,
  `A worklog/TOOL-RED-SHELL.md`, `M tasks/completion/claims.json` (author row only,
  `+6 -0`). No source/Cargo/existing-test/policy change. Scope PASS.
- Frozen SHA-256 `2853960b7ad711d88384b136db5b37eca1dd827e82c4339990c120267a4e11ed`
  matches claimed value, before and after the run (byte-identical).
- `git diff --check` clean; working tree clean before run (only own later edits).

## Public API and fixture audit
- Public APIs only: `ToolExecutor::new/execute`, `ToolCall::new/with_timeout`,
  `ToolResult`, `ToolRegistry::default/register`, `Tool::new`,
  `RegistryDispatcher::new/dispatch/available_permits/max_permits`,
  `OutputStore::new/stats`, `Mutex`. No private access, no `#[ignore]`.
- Fixtures: per-test `tempfile::tempdir`, fixed `touch <marker>` / `sleep 1; touch`
  commands. No network, no real secrets, no user DB.
- Bounds: Tokio `timeout(Duration::from_secs(10))` per call; t04 call timeout 50ms,
  1.5s settle. No unbounded wait.
- Sentinel `RED-SENTINEL-7f3a-not-a-secret` is fake; all assert messages use
  lengths, never the value. Log grep `RED-SENTINEL` = 0 occurrences. No leak.

## Command evidence
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=2`
- Result: compiled OK; `running 4 tests`; `0 passed; 4 failed; 0 ignored`;
  `error: test failed`; finished in 1.56s.

### Per-test failure classification (all intended product failures, not typos/private API)
- t01 line 60: `unauthorized shell must not succeed; got output len 0` -> shell
  spawned unbrokered; `execute` routed to `execute_shell` (executor.rs:109-134).
  INTENDED missing gate.
- t02 line 90: same denial cause. INTENDED missing gate.
- t03 line 135: `unauthorized shell dispatch must not report success` -> default
  `RegistryDispatcher::new()` uses `AllowAll` (registry_dispatch.rs:133-141), shell
  ran. INTENDED missing default-deny.
- t04 line 191: `timed-out shell left a late marker: child survived cancellation`
  -> `execute_shell` (executor.rs:240-297) spawns via `Command::new("bash")` with no
  `kill_on_drop`/group kill; child outlived the 50ms timeout. INTENDED but belongs
  to a DIFFERENT contract (see Unresolved gaps).

## Side-effect / artifact evidence
- Post-run: no `phase1_shell_broker`, `sleep 1`, or `touch <marker>` process alive.
- No tempdir marker leftovers (`no-spawn`, `no-side-effect`, `dispatch-denied`,
  `cancelled`) under `$TMPDIR`.
- Sentinel not present in log (grep count 0).

## Authority cross-check
- Production bypass exists exactly as scratchpad claims: `executor.rs:109-134`
  shell direct route; `execute_authorized` non-`read` falls through to unbrokered
  `execute` (:138-147); `execute_shell` no auth/kill (:240-297);
  `registry_dispatch.rs:133-141` default `AllowAll`, `:168-208` dispatch.
- `crates/server/src/lib.rs:1572` live `ToolExecutor::new()` unbrokered path present.
- Existing unit tests in `shell_tool.rs:536-565` prove broker-deny works only when
  a broker is injected into `ShellTool`; executor/dispatcher inject none. RED
  targets the public executor/dispatcher paths, correctly.

## Implementation-boundary recommendation (split)
1. Broker authorization lane (own t01, t02, t03): make shell/bash default-deny in
   `ToolExecutor::execute` without a concrete broker capability, and make
   `RegistryDispatcher` default policy deny shell. No spawn, no store write.
2. Cancellation lane (own t04, separate RED required): `execute_shell` must kill
   the child on timeout (e.g. `kill_on_drop(true)` / process-group kill) so no late
   marker. This is a distinct mechanism/owner from authorization.

## Resource observations
- 16 KiB pages, free ~183,635 (~2.8 GiB) before run; no host `timeout` binary, so
  relied on the tests' own bounded Tokio waits (10s + 50ms + 1.5s) plus single
  serial Cargo process (CARGO_BUILD_JOBS=2, RUST_TEST_THREADS=2).
- One Cargo process only; total host RSS observed ~98 MB pre-run; no swap panic;
  run finished 1.56s.

## Unresolved gaps
- t04 unsatisfiability (BLOCKER for a single-fix GREEN): t04 calls the SAME
  `ToolExecutor::new().execute()` path t01/t02 require to deny. After the broker
  gate lands, t04 fails at line 185 (`expected a timeout failure`) because the
  result error is a denial, not `timed out`; the child never spawns. t04 can only
  pass via an authorized-spawn entry (broker-allow) plus real child cancellation.
  Do not weaken t04; re-home it to the cancellation lane with its own RED against
  an authorized path.
- Convergence gate intentionally not reconciled (remains blocked).
