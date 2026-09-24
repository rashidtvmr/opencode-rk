# TOOL-SHELL-LEGACY-TEST-CONTRACT-REVIEW

## Claim
- Task `TOOL-SHELL-LEGACY-TEST-CONTRACT-REVIEW`, session `ses_f2d1ade6dffewXM5hKsvbMtxiW`.
- Route `vyce-deepseek-v41`, confirmed in allowlist.
- Branch `plan/TOOL-SHELL-LEGACY-CONTRACT`; owned file: this worklog only.
- Status: in-progress (ledger updated).

## Source evidence

### Frozen broker suite (authoritative)
- `crates/tools/tests/phase1_shell_broker.rs` SHA-256: `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`
- Freeze chain: `2cd9ca9` (original RED, 4 tests) -> `2c5e9cf` (refreeze: t04 removed, t01-t03 retained) -> `ed4164db` (GREEN executor).
- The refreeze ledger note (claims.json line 1074) states: t01/t02 GREEN after executor impl; t03 "remains registry-owned RED if dispatcher is unchanged."
- t01, t02, t03 are frozen. No edit/delete/rename/skip permitted (AGENTS.md frozen-test rules).

### Legacy in-file unit tests (conflicting)
- `crates/tools/src/executor.rs:482-512`:
  - `execute_success` (line 482): calls `executor.execute(call)` with `ToolCall::new("test-1", "bash", ...)`, asserts `result.success` and `result.output.contains("hello")`. Calls unbrokered shell.
  - `execute_timeout` (line 495): calls `executor.execute(call)` with `ToolCall::new("timeout-test", "bash", ...)`, asserts `!result.success` and `result.error` contains "timed out". Calls unbrokered shell.
  - `execute_failure` (line 514): also calls unbrokered `bash` with `exit 1`.
- `git blame -L 407,448` shows base commit `2ff1806` (2026-09-14) for the test block (lines were originally at 434-486 before the GREEN diff shifted them; the `execute` method body at lines 407-448 is from `2ff1806`).
- These tests have NO frozen hash. The original RED (2cd9ca9) froze only `crates/tools/tests/phase1_shell_broker.rs`, not the in-file tests in `executor.rs`.

### Current executor.rs (post-GREEN at ed4164db)
- `execute()` lines 134-141: `bash`/`shell` calls now return a deny result immediately — `error: Some("shell execution denied: broker authorization required")` — with NO spawn.
- `execute_authorized()` lines 160-178: routes `bash`/`shell` through `execute_shell_authorized` with broker consult.
- `execute_shell_authorized()` lines 270-372: full broker authorization path with `OperationIntent::Process`, `Decision::Allow` gate, `env_clear`, bounded PATH.
- There is no `execute_shell()` public method anymore (removed in the GREEN diff).

### RegistryDispatcher
- `RegistryDispatcher::new()` uses `AllowAll` policy (always returns `true`).
- `dispatch()` consults `self.policy.authorize()` before spawning.
- t03 calls `RegistryDispatcher::new()` (AllowAll) and expects denial — this FAILS post-GREEN because AllowAll permits the shell through, and then executor denies (so t03 gets an "error" but not the `Denied` enum variant t03 expects).

## Provenance table

| Test | File:Line | Introduced in commit | Frozen hash | Current status |
|------|-----------|---------------------|-------------|----------------|
| execute_success | executor.rs:482 | 2ff1806 (base feature) | none | FAILS (unbrokered, asserts success) |
| execute_timeout | executor.rs:495 | 2ff1806 (base feature) | none | FAILS (unbrokered, asserts timeout) |
| execute_failure | executor.rs:514 | 2ff1806 (base feature) | none | FAILS (unbrokered, asserts failure path) |
| t01_shell_without_broker_denied_before_spawn | phase1_shell_broker.rs:48 | 2cd9ca9 | ef56f63a... (frozen) | GREEN (post ed4164db) |
| t02_denial_leaves_no_marker_and_no_secret | phase1_shell_broker.rs:79 | 2cd9ca9 | ef56f63a... (frozen) | GREEN (post ed4164db) |
| t03_dispatcher_without_broker | phase1_shell_broker.rs:105 | 2cd9ca9 | ef56f63a... (frozen) | RED (dispatcher AllowAll unchanged) |

## Authority ranking

1. Frozen broker suite (`ef56f63a...`) — highest authority. Immutable per AGENTS.md/TDD.md. Authoritative deny-by-default contract (PLAN.md ADR-006).
2. Legacy in-file unit tests — NO frozen hash, introduced in the same base commit as the original executor feature, never independently frozen or tied to release criteria.
3. Security policy (docs/SECURITY.md) — deny-before-spawn, no cfg(test) bypass, no content allowlist (line 37-42, 54-56).

## Satisfiability / compatibility proof

### Can the legacy tests pass without insecure special-casing?

**No.** The legacy tests call `ToolExecutor::execute()` directly with `bash` tool name and assert:
- `execute_success`: success + output containing "hello"
- `execute_timeout`: failure + error containing "timed out"

Post-GREEN, `execute()` denies ALL `bash`/`shell` calls with no spawn (line 134-141). To make these pass, `execute()` would need to spawn shell processes WITHOUT broker authorization — directly violating:
- docs/SECURITY.md sec 2-3: "No direct secret file access... Permission `*` cannot bypass human-only grant."
- PLAN.md ADR-006: "All agent-originated operations use a trusted policy broker."
- AGENTS.md: "No cfg(test) bypass, no caller detection."
- The frozen broker suite itself (t01/t02 which assert denial before spawn).

There is no implementation that makes unbrokered `execute("bash")` succeed while also keeping t01/t02 green (which assert the same call path must deny). These are **direct contradictions**.

### Can the legacy tests pass via `execute_authorized` with a broker?

**Partially for success, impossible for timeout.** `execute_authorized` now routes shell through the broker. But:
- `execute_success` calls the unbrokered `execute()`, not `execute_authorized()`.
- Even if migrated, `execute_timeout`'s semantics (timeout error) cannot be exercised through an AllowAll broker — the broker would authorize, the shell would spawn, and `sleep 10` would be killed by timeout. But the test asserts `result.error.unwrap().contains("timed out")` — this requires the OLD code path that returns "Execution timed out" from `tokio::time::timeout` on `Command::output()`. The GREEN path also returns "Execution timed out" (line 369), so this COULD work IF routed through `execute_authorized` with a permissive broker.
- BUT: migrating tests means editing frozen test code, which is forbidden.

## Verdict

**The two legacy executor unit tests (`execute_success` and `execute_timeout`) are STALE tests incompatible with the independently frozen deny-before-spawn broker contract.**

They were never independently frozen (no hash in claims.json, no separate freeze commit). They predate and contradict the security-mandated broker gate. They assert unbrokered shell success/timeout, which is now a security violation. t03 (registry dispatcher) remains a separate pending RED owned by the dispatcher lane.

## Recommendation: exact minimal correction (no edits authorized in this lane)

This verification lane produces evidence and recommendation only. The correction must be executed by an authorized test-owner/controller with independent refreeze.

### Smallest future correction plan

1. **Remove** the stale in-file unit tests `execute_success` (executor.rs:482-493) and `execute_timeout` (executor.rs:495-512).
   - Rationale: they assert unbrokered shell success/timeout — a direct contradiction of the frozen deny-before-spawn contract (t01/t02). No secure implementation can satisfy both.
   - `execute_failure` (line 514-523) also uses unbrokered `bash` but asserts failure; it is likewise stale for the same reason (the deny path returns failure, not the shell spawning and exiting 1).

2. **If shell success/timeout coverage is desired**, add NEW tests in the broker test file `crates/tools/tests/phase1_shell_broker.rs` that:
   - Construct a permissive broker (fake grant issuer) that returns `Decision::Allow` for the specific `OperationIntent::Process { program: "bash", ... }`.
   - Call `executor.execute_authorized(call, &permissive_broker)` for shell.
   - Assert success + marker file created for the success case.
   - Assert timeout error + no surviving marker for the timeout case.

3. **RED phase for the replacement tests** (must compile and fail before implementation):
   - New test file or additions: assert that an Allow-all broker still denies when `Decision::Deny` is returned (negative); assert that `Decision::Allow` for a fixed `echo` command returns success with correct output.
   - Capture exact failing fixture, freeze hash.

4. **Do NOT touch** the frozen broker suite hash `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`. t01/t02/t03 remain immutable.

5. **Do NOT** introduce cfg(test) bypass, caller detection, or AllowAll default in `execute()`.

## Conflict reproduction evidence
- `git diff --check`: clean (no uncommitted changes in this worktree).
- Frozen broker suite hash: `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8` (confirmed via `shasum -a 256`).
- Legacy tests: no frozen hash, no separate freeze commit, introduced in base commit `2ff1806`.
- GREEN commit `ed4164db` made `execute()` deny all shell (lines 134-141), making `execute_success` and `execute_timeout` impossible to satisfy without violating the deny-before-spawn contract.

## Conclusion
The legacy in-file unit tests are stale contracts that contradict the frozen deny-by-default broker security model. They should be removed and replaced with broker-authorized tests, pending controller authorization and independent refreeze. This review lane makes NO code changes.