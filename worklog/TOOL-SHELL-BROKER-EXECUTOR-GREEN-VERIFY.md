# TOOL-SHELL-BROKER-EXECUTOR-GREEN-VERIFY scratchpad

## Claim
- Task `TOOL-SHELL-BROKER-EXECUTOR-GREEN-VERIFY`; session
  `ses_f2d1b7173ffeqxI21PqE0cVUpM`; route `9router-th-dsv41-flash-free` (in allowlist).
- Branch `verify/TOOL-SHELL-BROKER-EXECUTOR-GREEN`; owned file this scratchpad + own ledger row.
- Role: independent verifier. No source/test edit, no registry edit, no merge.

## Candidate under verification
- Commit `ed4164db1774603cc44e8d45716cf70a794f471d` (HEAD of this worktree).
- Parent `2c5e9cf`; base `5d66683` ancestor chain intact.
- Diff `2c5e9cf..ed4164d` = 3 files only:
  - `M crates/tools/src/executor.rs` (+129 -54)
  - `M tasks/completion/claims.json` (+6 -0, author row only)
  - `A worklog/TOOL-SHELL-BROKER-EXECUTOR-GREEN.md`
- Scope PASS: no Cargo, no frozen test, no policy, no server/registry change.
- `git diff --check` clean, exit 0.

## Frozen integrity
- `shasum -a 256 crates/tools/tests/phase1_shell_broker.rs`
  = `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`
  before AND after both test runs. Byte-identical. PASS.
- `#[cfg(test)] mod tests` in executor.rs extracted old vs new: diff empty, sha256
  `c0627964...` both sides. Unit-test code NOT modified by the implementation.
  (The 183-line executor.rs diff touches only hunks starting line <=267; test mod
  begins line 469 and is unchanged.)

## Command evidence
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=2`
  - compiled; `running 3 tests`; `2 passed; 1 failed; 0 ignored`; EXIT 101.
  - `t01_shell_without_broker_denied_before_spawn ... ok`
  - `t02_denial_leaves_no_marker_and_no_secret ... ok`
  - `t03_dispatcher_without_broker_denies_shell_no_store_write ... FAILED`
    - panic `phase1_shell_broker.rs:153`: `assertion left == right failed: denied
      dispatch must write no durable record`; `left: (1, 4)`; `right: (0, 0)`.
    - marker assert (line 148) and deny-text assert (143) PASSED; failure is ONLY
      the store-stats assert.
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools executor::tests --lib -- --test-threads=2`
  - `running 5 tests`; `3 passed; 2 failed`; EXIT 101.
  - FAIL `executor::tests::execute_success` (executor.rs:489 `assert!(result.success)`)
  - FAIL `executor::tests::execute_timeout` (executor.rs:510 error not "timed out")
  - ok: `execute_failure`, `result_has_duration`, `batch_executes_all`.

## Decision matrix (source ground truth)
| Path | Source | Broker call before spawn? | Result |
|---|---|---|---|
| Generic `execute("bash"/"shell")` | executor.rs:134-141 | no spawn, no `Command::new` | fixed denial `"shell execution denied: broker authorization required"` |
| Authorized shell `execute_shell_authorized` | executor.rs:270-326 | builds intent (309-313), `broker.authorize` (315) BEFORE `Command::new` (328) | Allow -> spawn; Deny/RequireHuman -> denial, no spawn |
| Authorized read `execute_read` | executor.rs:180-268 | `file_ops::execute_authorized` under broker | unchanged |
| `RegistryDispatcher::dispatch` | registry_dispatch.rs:193-207 | policy AllowAll default (133-135); calls generic `execute` (203); `store_result` unconditional (206) | writes record even on denial -> t03 |

## Correctness of core close
- Deny-before-spawn: `execute` returns denial struct without constructing a
  `Command` (executor.rs:134-141). t01/t02 pass marker + no-secret asserts.
- Exact intent: `OperationIntent::Process { program:"bash", args:["-c",command],
  cwd }` (executor.rs:308-313) matches the actual spawn `Command::new("bash")
  .args(&args).current_dir(&cwd)` (328-332). Intent == spawn. PASS.
- Environment cleared: `.env_clear()` + minimal `PATH` (329-332). PASS.
- Denial message bounded, no command/input/env/approval-id echo (323). PASS.
- `Decision::Allow` is the ONLY gate to spawn; `Deny | RequireHuman` both deny
  (317-325). PASS.

## t03 root cause (registry-owned, outside executor)
- `registry_dispatch.rs:203` calls the generic `executor.execute(call)`, which now
  denies shell. But line 206 `store_result(...)` pushes unconditionally and line
  207 returns `Ok(record)` regardless of `result.success`. Hence store stats
  `(1, 4)` and the deny-text assert passed (result carried the executor denial).
- Sole fix is in `registry_dispatch.rs` (default-deny policy / conditional store),
  which is NOT the executor's owned file. Expected RED for this slice. CONFIRMED.

## Authorized-shell reachability finding (non-blocking, must be recorded)
- `execute_shell_authorized` Allow branch is NOT reachable with the real
  `PermissionBroker`: `authorize_process` (security/src/lib.rs:292) unconditionally
  returns `RequireHuman` for any shell interpreter with `-c`/`/c`, BEFORE the
  destructive-argv check. No policy/permission config can flip that.
- No production caller passes shell to `execute_authorized`: the server
  (server/lib.rs:1591) routes `bash`/`shell` through `ShellExecutionGuard` /
  `ShellTool` and only calls `execute_authorized` (1616) for non-shell tools.
- Therefore the executor's authorized-shell spawn is sanctioned-but-unwired for
  production shell. It is public API, compiles, and binds the exact intent; it is
  not a security regression, but "authorized shell path is real, not dead" is only
  true at the API boundary, not yet in a production call site. Recorded as a gap,
  not a rejection of the slice.

## Side-effect / artifact evidence
- `pgrep -fl "bash -c"` after runs: only host login shells (pid 735/3293/3321/83675),
  no test marker/sleep child survives. PASS.
- No marker file: t01/t02 marker asserts passed; tempdirs disposable.
- Sentinel `RED-SENTINEL` never emitted (asserts use lengths only).

## Verdict
`ACCEPT WITH CORRECTIONS`.
- Slice deliverable (close unbrokered shell bypass, deny before spawn, t01/t02
  GREEN, no secret/marker leak, frozen hash intact, scope clean) is TRUTHFUL.
- Corrections/blockers, none of which the executor slice may suppress:
  1. t03 remains RED; cause is `registry_dispatch.rs:203/206` (registry lane).
  2. Two legacy unit tests `executor::tests::execute_success` and
     `executor::tests::execute_timeout` FAIL because they call the now-deny-default
     `execute` with `bash` and expect spawn. This is a contract-review blocker:
     the old tests encode the removed bypass. They must be reviewed/reclassified
     by the owning contract lane, NEVER decoded by editing tests in this verifier.
  3. Authorized-shell spawn unreachable under the real broker; production shell
     uses ShellTool. Not a defect in this diff, but the "authorized path" claim is
     API-level only.

## Unresolved gaps / blockers
- Registry default-deny + conditional store write (t03) — separate owned slice.
- Legacy executor shell unit-test contract conflict — blocked contract review.
- Cancellation contract (t04 of the original frozen set) — separate, out of scope.

## Scope / hash final
- Frozen hash unchanged pre/post. `git diff --check` clean. No test/source edit by
  this verifier; only this scratchpad + own ledger row written.
