# TOOL-AUTHORIZED-PROCESS-SEAM scratchpad

## Claim and route

- Task: `TOOL-AUTHORIZED-PROCESS-SEAM`
- Type: `implementation`
- Session: `ses_f2cc620c6ffeMsfgIyrBm8Ap2H`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/lane-tool-authorized-process-seam`
- Branch: `lane/TOOL-AUTHORIZED-PROCESS-SEAM`
- Owned product file: `crates/tools/src/executor.rs`
- Permitted companion: this scratchpad; own ledger row only
- Route: `9router-th-dsv41-flash-free`; allowlist permission confirmed
- Base HEAD: `b66ee38`

## Source evidence (read first)

- `.agents/WORKER.md`: claim before edits, one owned file, no test edits, push without force.
- `docs/TDD.md` sections 2-6; `docs/SECURITY.md`; `AGENTS.md` non-negotiables and 8 GiB budget.
- Accepted cancellation contract `origin/plan/TOOL-SHELL-CANCEL-CONTRACT`
  commit `c630f3f`, `worklog/TOOL-SHELL-CANCEL-CONTRACT.md`:
  - "Proposed public contract" through "Resource and lifetime ceilings" is normative.
  - "Mandatory seam and RED ordering": seam first, then independent verification, then a
    compiling cancellation RED; compile/import-fail RED is forbidden.
- Current source: `crates/tools/src/executor.rs` (generic `execute` denies shell;
  `execute_authorized` handles only `read`); `crates/tools/src/shell_tool.rs`
  (legacy process path, `/tmp` cwd fallback defect); `crates/security/src/tool_authorize.rs`
  (`ToolAuthorizer`, `ToolGate`); `crates/security/src/app_policy.rs`
  (`Grant`, `ExpectedScope`, `decide` mandatory-Deny-first).
- `crates/tools/Cargo.toml`: tokio `io-util`, `rt`, `process`; no `sync`/`time`/`macros`
  in this crate, but the workspace `Cargo.toml:39` unifies features to include
  `sync` and `time`, so `tokio::sync::{oneshot, watch}` and `tokio::time::Instant`
  are available without a manifest edit. `rustix` (`process`) already present.
- Frozen broker test `crates/tools/tests/phase1_shell_broker.rs` hash:
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.

## Observed scenario and target boundary

No public executor-level authorized-process owner existed. The contract requires a
public `ToolExecutor::execute_authorized_process` whose exact types and semantics
are the seam a later cancellation RED compiles against. This lane implements only
that seam plus its normative public types: direct-argv authorization, one canonical
cwd preparation, `env_clear`, caller watch cancellation, one readiness oneshot, one
absolute deadline, bounded draining output, Unix process-group kill, explicit child
wait and reader joins, bounded cleanup status, fixed redacted <=512-byte errors,
and Windows fail-closed. No server wiring, registry repair, dependency, unsafe,
test edit, or cancellation RED is in scope.

## Public API delivered

Types: `ProcessRequest`, `ProcessLimits`, `ProcessReady`, `ProcessTerminal`,
`CleanupStep`, `CleanupStatus`, `ProcessError`, `ProcessResult`,
`ProcessCancellation = tokio::sync::watch::Receiver<bool>`, and the method
`ToolExecutor::execute_authorized_process(&self, ProcessRequest, &mut ToolAuthorizer,
Option<&Grant>, &ExpectedScope, ProcessLimits, ProcessCancellation,
oneshot::Sender<ProcessReady>) -> Result<ProcessResult, ProcessError>`.
All are `pub` in the public `executor` module, verified by an external integration
probe (since deleted).

## State / lifetime matrix

| Event | Implementation | Observable |
|---|---|---|
| Invalid limits | `validate_process_limits` first | `InvalidLimits`, no broker/spawn |
| Invalid program/argv/env | `validate_process_request` | `InvalidRequest { field }`, no side effect |
| cwd prep | `prepare_canonical_cwd`: one `fs::canonicalize`, abs + dir + <= cap | `InvalidCwd` on failure; one `PathBuf` reused |
| Cancel before authorization | `*cancellation.borrow()` | `CancelledBeforeStart`, no grant burn |
| Deny | baseline `authorizer.authorize` | `Denied`, never lifted by grant |
| RequireHuman, no grant | baseline `HumanGate` | `HumanRequired` |
| RequireHuman, grant | `authorize_with_grant` (single-use ledger) | `Allow` once; rejected -> `InvalidGrant` |
| Spawn | exact program/args/canonical cwd, `env_clear`, minimal PATH, new group | PID/group owned |
| Readers | two `tokio::spawn(read_capped)` before readiness | excess drained, retained <= cap |
| Startup window | spawn attempt to readiness vs `startup_timeout` | breach -> `TimedOut` + cleanup |
| Invalid PID | `pid == 0` | cleanup then `Io{readiness_pid}` or `CleanupFailed` |
| Readiness | one `oneshot::send` | exactly one event |
| Send failure | cleanup then error | `ReadinessReceiverClosed{pid}` if `Reaped`, else `CleanupFailed` |
| Running select | `biased; wait, cancel change, deadline` | child > cancellation > timeout |
| Cancel at readiness | explicit `borrow()` latch | `Cancelled` + group cleanup |
| Normal exit | `join_readers`, kill steps `NotRequired` | `Reaped { wait: Succeeded }` |
| Timeout/Cancel | `cleanup_process` group-kill/child-kill/wait/readers | `Reaped` or `CleanupFailed` |
| Wait error | cleanup then `Io{child_wait}` | no leaked child/readers |
| Non-Unix | `cfg(not(unix))` early return | `UnsupportedPlatform` |

## Decisions

- Grant consulted only after a baseline `HumanGate`; a mandatory baseline `Deny`
  is terminal, so `*`/grants cannot lift it (matches `app_policy::decide` order).
- Error reasons are the fixed literal or `redact_secrets`-scrubbed text capped at
  512 bytes; raw argv/env/cwd never appear in an error.
- `duration_ms` computed once from `process_now()` via
  `u64::try_from(as_millis()).unwrap_or(u64::MAX)`; sub-ms is `0`.
- Tokio clock used for the deadline so paused-time tests can advance it.
- Truncation marker `"\n[truncated]"` appended exactly once, outside the byte cap.
- `kill_on_drop(true)` retained only as a last-resort signal; cleanup is always the
  explicit `cleanup_process` sequence.

## Validation (sequential, sole Cargo lane)

- `rtk shasum -a 256 crates/tools/tests/phase1_shell_broker.rs`
  -> `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8` (unchanged).
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 rtk cargo check -p opencode-rk-tools`
  -> `0 errors, 6 warnings`; zero warnings attributable to `executor.rs`.
- External public-API probe (temporary `crates/tools/tests/tmp_seam_final.rs`,
  deleted before commit): 4/4 passed. Covered allow exit with `Reaped` cleanup,
  `Deny`, `HumanRequired`, valid single-use grant, timeout reap, cancel before
  start, cancel after readiness, truncation bound, canonical cwd byte equality,
  invalid limits/env pre-spawn rejection, and readiness-receiver-closed reap.
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 rtk cargo test -p opencode-rk-tools
  --test phase1_shell_broker -- --test-threads=2` -> 3 passed, 0 failed.
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools
  executor::tests --lib -- --test-threads=2` -> 3 passed, 2 failed
  (`execute_success`, `execute_timeout`). These are the documented stale unit tests
  that assert the pre-broker `bash` behavior; identical to the pre-change baseline,
  not regressions, and not edited (test immutability).
- `rtk git diff --check` -> exit 0, no output.
- No `sleep 30`/`yes` child remained after the probe runs (`pgrep` empty).

## Remaining unknowns / gaps

- Independent seam verification must exercise the public API before any
  cancellation RED is authored; this lane does not self-accept.
- Windows process-tree backend and its real-platform proof remain absent; the seam
  fails closed there rather than claiming support.
- `startup_timeout` is enforced as the spawn-to-readiness window on the Tokio
  clock; a real high-latency host would exercise the breach branch, which the probe
  did not force deterministically (branch reviewed, not runtime-forced).
- Server wiring, dispatcher default-deny, and the cancellation RED remain future
  lanes; this lane does not fix the known `dispatch_batch` Immediate-loss.