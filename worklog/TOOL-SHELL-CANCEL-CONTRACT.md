# TOOL-SHELL-CANCEL-CONTRACT

Status: corrected research contract only; pending independent verification. No
product source, test source, manifest, or release claim is changed by this lane.

## Claim and scope

- Task: `TOOL-SHELL-CANCEL-CONTRACT`
- Task type: `research`
- Session: `ses_f2d51a501ffe3gAaRGLcrbItWU6`
- Branch: `plan/TOOL-SHELL-CANCEL-CONTRACT`
- Candidate revision inspected: `2c5e9cfbdc4989a54e88b0ebb095a078cc4805de`
- Owned artifact: `worklog/TOOL-SHELL-CANCEL-CONTRACT.md`
- Product/test edits: forbidden for this lane.
- No new RED is authored. The contract is a prerequisite for an independently
  frozen cancellation RED.

## Correction lane

- Task: `TOOL-SHELL-CANCEL-CONTRACT-CORRECTION`
- Task type: `research`
- Session: `ses_f2d25ff53ffeI8JA2mrF6iLY1Y`
- Base contract revision: `2c7575362ff73a17134767068809c7513651f501`
- Verifier evidence: `worklog/TOOL-SHELL-CANCEL-CONTRACT-VERIFY.md` at
  `da003a9c14d3a4c9907079e2bbee9967172e0641`
- Scope: reconcile C1-C5 only; no product, test, manifest, or dependency edits.
- The corrected artifact remains a contract, not an implementation or acceptance
  claim. A fresh verifier must validate it on the pushed revision.

## Authority and evidence

Governing sources: `AGENTS.md`, `.agents/WORKER.md`, `PLAN.md`, `docs/TDD.md`,
`docs/SECURITY.md`, `docs/CONVERGENCE.md`, and the accepted split proposal at
`worklog/TOOL-RED-SHELL-SPLIT-PROPOSAL.md`.

Current behavior is distinguished from compatibility, planned behavior, and new
requirements below. A source comment or a design artifact is not implementation
evidence.

## Current call graph

| Surface | Current evidence | Consequence |
|---|---|---|
| Generic executor | `crates/tools/src/executor.rs:108-134`, `ToolExecutor::execute` | `bash` and `shell` route directly to `execute_shell`. |
| Authorized executor | `crates/tools/src/executor.rs:136-147`, `ToolExecutor::execute_authorized` | Only `read` is concrete; shell falls back to unbrokered `execute`. |
| Legacy shell executor | `crates/tools/src/executor.rs:240-297`, `execute_shell` | Builds `bash -c`; timeout wraps `Command::output()` and does not own/reap the child on timeout. |
| Registry dispatch | `crates/tools/src/registry_dispatch.rs:64-78,131-145` | Default policy is `AllowAll`. |
| Registry dispatch call | `crates/tools/src/registry_dispatch.rs:187-207` | Boolean preflight is not a process broker; execution still uses the unbrokered executor. |
| Server shell construction | `crates/server/src/lib.rs:1071-1095`, `new_shell_execution` | Constructs `ShellTool` without injecting a broker. |
| Server shell authorization | `crates/server/src/lib.rs:1388-1437` | Authorizes `OperationIntent::Tool`, then constructs the shell path; no exact process intent is passed to `ShellTool`. |
| Direct shell owner | `crates/tools/src/shell_tool.rs:83-97,145-149,189-211` | Optional broker; when present, exact program/argv/cwd are authorized before spawn. `None` remains a legacy bypass. |
| Process setup | `crates/tools/src/shell_tool.rs:213-237` | Clears inherited environment, supplies bounded PATH, creates Unix process group, enables kill-on-drop. |
| Process completion | `crates/tools/src/shell_tool.rs:270-315` | Normal path awaits `Child::wait`; timeout returns without an explicit group-kill/wait cleanup sequence. |
| Cancellation/drop | `crates/tools/src/shell_tool.rs:340-367` | `cancel` and `Drop` issue best-effort kills; neither exposes an awaited cleanup result. |
| Output bound | `crates/tools/src/shell_tool.rs:394-412` | Retained bytes are capped while excess is drained. |
| Broker intent | `crates/security/src/lib.rs:42-71,206-227,290-300` | `Process` binds program/argv/cwd; `Deny` and `RequireHuman` are non-allow decisions. Shell `-c` is human-gated. |
| Grant semantics | `crates/security/src/tool_authorize.rs:63-78,122-160`; `crates/security/src/app_policy.rs:288-338` | Direct-argv intent and scoped, digest-bound, single-use grant checks already exist. |
| Unwired reference | `crates/tools/src/shell_bounds.rs:10-23,119-158,281-320` | Provides a useful bounded process-tree reference; it is not wired into the live path. |

### Classification

- Native current behavior: direct-argv `ShellTool` authorization when a broker is
  injected; Unix process-group setup; bounded retained output; best-effort drop.
- Shared/legacy compatibility: `ToolExecutor::execute`, `bash -c`, optional broker,
  `AllowAll` dispatcher default, and `ShellTool::cancel` on drop.
- Partial/planned behavior: explicit concurrent cancellation, readiness contract,
  cleanup result, and executor-level authorized process owner.
- New requirement: one public request owner, exact pre-spawn authorization,
  bounded environment/argv/output/waits, deterministic terminal result, explicit
  reap, and unrelated-process isolation.
- Deliberate deviation: opaque shell strings are not the cancellation fixture;
  direct argv is required because the broker treats `bash -c` as human-only.

## Historical t04 conflict

The superseded cancellation test used the same `ToolExecutor::execute` call as
 the denial tests and expected a timeout. That contract is unsatisfiable with
pre-spawn broker enforcement:

1. Broker denial must happen before `spawn`.
2. The old call supplies no broker decision or human grant.
3. Correctly denying it means t04 observes denial, never `timed out`.
4. A timeout value is not an authorization capability.
5. Leaving the child alive after timeout is a separate lifetime defect, not a
   reason to bypass the broker.

The accepted split is therefore:

- `crates/tools/tests/phase1_shell_broker.rs`: retain t01-t03 unchanged.
- Future separate file: `crates/tools/tests/phase1_shell_cancellation.rs`.
- Current frozen broker test SHA-256 remains
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.

No test source is edited by this research lane.

## Mandatory seam and RED ordering

The executor-level authorized process entrypoint below is a required public
seam, not an optional recommendation and not a symbol claimed to exist today.
Before a cancellation RED file may be authored:

1. **Seam implementation lane.** Implement and expose the exact public
   `ToolExecutor::execute_authorized_process` API, its request/result/error
   types, direct-argv authorization, canonical cwd preparation, bounded
   cancellation receiver, readiness sender, and explicit owner future. This lane
   may add no dependency and must preserve t01-t03.
2. **Independent seam verification lane.** A fresh verifier exercises the
   public API against the real broker and direct-argv fixture, including
   `Deny`, `RequireHuman` without a grant, valid grant, and no-spawn invariants;
   then reruns the unchanged broker suite and records the seam revision.
3. **Compiling behavior RED lane.** Only after step 2 is accepted, author
   `phase1_shell_cancellation.rs`. It must compile against the public seam and
   fail at behavioral assertions (cleanup, precedence, bounds, or lifetime),
   never because a type, import, method, or module is absent. Freeze its hash
   and command manifest.
4. **Cancellation implementation lane.** Implement the minimum behavior that
   turns that frozen RED green.
5. **Independent cancellation verification.** Re-run the frozen test, focused
   regressions, and exact integrated journey. No lane may self-accept.

The dependency edge is strict: no cancellation RED lane may be claimed, authored,
or frozen until the seam implementation is pushed and the independent seam
verifier has accepted the exact public API revision. A compile-fail/import-fail
RED is forbidden. A test that targets a private symbol, a shell-string bypass,
`AllowAll`, or a nonexistent proposed API is not RED evidence. The existing broker
RED remains t01-t03 only and stays unchanged.

## Proposed public contract

The following is the integration contract, not a claim that these symbols exist
in the current revision. Every public type and variant named here is normative
for the later seam/RED lanes.

```rust
pub struct ProcessRequest {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
}

pub struct ProcessLimits {
    pub timeout: Duration,
    pub startup_timeout: Duration,
    pub cleanup_timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_argv_bytes: usize,
    pub max_argv_elements: usize,
    pub max_env_bytes: usize,
    pub max_env_elements: usize,
    pub max_cwd_bytes: usize,
}

pub struct ProcessReady {
    pub pid: u32,
    #[cfg(unix)]
    pub process_group: u32,
}

pub enum ProcessTerminal {
    Exited { exit_code: Option<i32> },
    TimedOut,
    Cancelled,
    CancelledBeforeStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupStep {
    NotRequired,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupStatus {
    NotRequired,
    Reaped {
        group_kill: CleanupStep,
        child_kill: CleanupStep,
        wait: CleanupStep,
        readers: CleanupStep,
    },
    Failed {
        group_kill: CleanupStep,
        child_kill: CleanupStep,
        wait: CleanupStep,
        readers: CleanupStep,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessError {
    InvalidRequest { field: &'static str, reason: String },
    InvalidCwd { reason: String },
    InvalidLimits { reason: String },
    Denied { reason: String },
    HumanRequired { reason: String },
    InvalidGrant { reason: String },
    Spawn { reason: String },
    ReadinessReceiverClosed { pid: u32 },
    Io { operation: &'static str, reason: String },
    CleanupFailed { terminal: ProcessTerminal, status: CleanupStatus },
    UnsupportedPlatform { operation: &'static str },
}

pub struct ProcessResult {
    pub terminal: ProcessTerminal,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub cleanup: CleanupStatus,
    pub duration_ms: u64,
}

pub type ProcessCancellation = tokio::sync::watch::Receiver<bool>;
```

`ProcessLimits` is required on every request and is validated at the seam boundary
before authorization. Every timeout is a positive, finite `Duration`; every byte
and element ceiling is positive. `startup_timeout` is the bounded interval from
spawn attempt until readiness publication, `timeout` is one absolute deadline for
the whole request, and `cleanup_timeout` starts when cleanup begins. The hard
ceilings are normative: timeout fields at most 300 seconds; retained stdout and
stderr at most 10 MiB each; argv at most 64 KiB and 4,096 elements; environment
at most 64 KiB and 1,024 elements; cwd at most 4 KiB. The retained-byte caps
exclude the fixed truncation marker. A request cannot widen these ceilings by
passing larger values.

`ProcessResult.duration_ms` is exactly `u64`, matching the existing public
`ToolResult.duration_ms` at `crates/tools/src/executor.rs:74-87` and the other
bounded public result surfaces. It is computed from one `Instant` at request
entry as `u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)`: a
sub-millisecond duration is `0`; an elapsed value above `u64::MAX` saturates at
`u64::MAX`; no unchecked `as` cast, wrap, or platform-dependent integer type is
permitted. The legacy `ShellResult.duration_ms: u128` at
`crates/tools/src/shell_tool.rs:23-33` is not this public result type and is not
silently cast or changed by this contract.

`ProcessResult` is returned only for a completed, bounded request. `Exited`
carries `CleanupStatus::Reaped` with kill steps `NotRequired` when the child
already exited, `wait`/`readers` `Succeeded`, and the exact exit code (or
`None` when the platform cannot provide one). `TimedOut`, `Cancelled`, and
`CancelledBeforeStart` carry `CleanupStatus::NotRequired` only when no child was
created; after a child exists they require `Reaped`. No `ProcessError` returns a
partial stdout/stderr result.

Required public entrypoint on `ToolExecutor`:

```rust
pub async fn execute_authorized_process(
    &self,
    request: ProcessRequest,
    authorizer: &mut ToolAuthorizer<'_>,
    grant: Option<&Grant>,
    expected: &ExpectedScope,
    limits: ProcessLimits,
    cancellation: ProcessCancellation,
    ready: tokio::sync::oneshot::Sender<ProcessReady>,
) -> Result<ProcessResult, ProcessError>;
```

`ProcessError` meanings are exact: `InvalidRequest` covers malformed program,
argv, or env; `InvalidCwd` covers cwd validation/canonicalization failure;
`InvalidLimits` covers non-positive or over-limit bounds; `Denied` and
`HumanRequired` are pre-spawn broker outcomes; `InvalidGrant` is a supplied
grant that fails scope, digest, freshness, policy-version, or single-use
checks; `Spawn` is an OS spawn failure; `ReadinessReceiverClosed` is returned
only after a child was successfully spawned and the `ready` oneshot receiver was
dropped, after the owner has completed bounded cleanup; `Io` identifies a
bounded wait/read/join operation; `CleanupFailed` is returned instead of an
ordinary result when required cleanup cannot be proven; and
`UnsupportedPlatform` is the fail-closed result for a platform without the
required backend. `InvalidRequest.field` is one of `program`, `args`, or `env`;
`Io.operation` is one of `readiness_pid`, `stdout_read`, `stderr_read`, `child_wait`,
or `reader_join`. Error reason text is bounded to 512 bytes and redacted; it
never retains raw env, argv, cwd, or secret values.

When `ready.send(ProcessReady { .. })` fails, the executor must immediately latch
`Cancelled` as the cleanup terminal, run the same group-kill/child-kill/wait/
reader-join sequence with `cleanup_timeout`, and return no ordinary
`ProcessResult`. If cleanup is `Reaped`, return
`ProcessError::ReadinessReceiverClosed { pid }`; if any required cleanup step
fails or its bounded wait expires, return
`ProcessError::CleanupFailed { terminal: ProcessTerminal::Cancelled, status }`
with `status` equal to `CleanupStatus::Failed`. The PID is retained only as the
numeric error field needed for bounded diagnostics, never as a readiness event.

`CleanupFailed.status` is always `CleanupStatus::Failed` and `terminal` is the
already-latched terminal reason. A cleanup failure cannot be downgraded to a
warning or ordinary success.

`CleanupStep` has exactly three meanings. `NotRequired` is legal only when
that specific operation was unnecessary: no child was created; the child was
already reaped; the group-kill API was unavailable after the child exited; or no
reader task was created because the corresponding pipe was absent. `Succeeded`
means the operation was attempted and its required observable effect completed
without error. `Failed` means the operation was required but its effect was not
observed, its await exceeded `cleanup_timeout`, or joining the corresponding
reader failed.

`CleanupStatus::NotRequired` means no child was ever created. For
`CleanupStatus::Reaped` or `CleanupStatus::Failed`, each field is interpreted
as follows: `group_kill` is required whenever a child exists and a termination
request is latched; `child_kill` is required only when the group-kill operation
was unavailable or did not establish child termination; `wait` is required after
a successful spawn until `Child::wait` has returned; and `readers` is required for
every pipe that was taken, until its reader task has been joined. A child already
reaped by a successful `wait` makes the later kill steps `NotRequired`; it does
not make `wait` `NotRequired`. `Reaped` is valid only when every required field
is `Succeeded`, every other field is an allowed `NotRequired`, and the owned child
has been explicitly waited. `Failed` means at least one required field is
`Failed`; it is never returned inside `Ok(ProcessResult)` and never claims
deterministic cleanup.

The security module remains the authority for constructing/validating the
authorization. The execution module must not accept `AllowAll`, a boolean policy
hook, a timeout, or an opaque command string as proof of authorization. The
public seam must preserve these observables and ownership rules. `Grant` and
`ExpectedScope` remain caller-supplied capability inputs within the existing
broker model; this contract claims neither a signed capability nor an OS
sandbox, and a caller must not treat a self-constructed grant as authority
outside the trusted broker/policy boundary.

### Cancellation primitive and exact semantics

`ProcessCancellation` is a caller-owned `tokio::sync::watch::Receiver<bool>`.
The caller creates `watch::channel(false)`, retains the `watch::Sender<bool>` for
the operation lifetime, and passes the receiver to the executor. The receiver is
the only cancellation state read by the request. `borrow()` checks the current
value; `changed()` is awaited only while at least one sender remains. A `true`
value is a monotonic cancellation request: once observed, the request is latched
and later `false` values cannot reopen it. Sending `false`, sending no value, or
dropping all senders without first sending `true` does **not** cancel the
operation. A closed receiver is treated as “no cancellation signal,” not as
cancellation.

The receiver is checked from its current value before authorization, after
authorization, immediately before spawn, and at every bounded running
observation. Each check is made in the fixed precedence below. Repeated
cancellation is idempotent, and a signal received after the terminal state is
latched has no effect. Cancellation is scoped to this one request; no shared or
global token exists. Only Tokio's existing `sync` feature is used; `tokio-util`
and `CancellationToken` are not part of this contract.

### Request, cwd, and environment invariants

- `program`, every argv element, and the prepared canonical `cwd` are the exact
  values placed in `OperationIntent::Process` and in `Command`.
- No shell parsing, interpolation, concatenation, or implicit `-c` exists on the
  authorized direct-process path.
- `env` is explicit. `env_clear()` is mandatory. No secret-bearing inherited
  variable is available unless explicitly supplied and policy-authorized.
- PATH is a fixed minimal value unless the request explicitly supplies one.
- Empty program, NUL-containing values, invalid cwd, oversized argv/env, and
  invalid limits fail before broker/spawn. All byte and element ceilings in
  `ProcessLimits` are checked before authorization; a zero limit is invalid.
- A grant, if needed, remains scoped to the exact operation digest, workspace,
  session, requester, policy version, and expiry. It is single-use.

Validation is exact: `program` is non-empty and NUL-free; every argv element is
NUL-free and within the byte/element limits; each environment key is non-empty,
NUL-free, and contains no `=`; each environment value is NUL-free and within
the byte/element limits. A malformed program/argv/env value is
`InvalidRequest`; a malformed or non-directory cwd is `InvalidCwd`; a zero,
non-finite, or over-cap limit is `InvalidLimits`. These checks precede broker
authorization and grant-ledger mutation.

#### Canonical cwd identity (one preparation pass)

`ProcessRequest.cwd` is mandatory; there is no optional cwd and no `/tmp`
fallback. Before broker authorization, the executor performs one preparation
function, in this order:

1. Reject empty, NUL-containing, and over-`max_cwd_bytes` path values.
2. Call `std::fs::canonicalize` exactly once. Reject failure, non-absolute
   results, over-limit canonical bytes, and paths whose metadata is not a
   directory. Return `ProcessError::InvalidCwd` for any failure.
3. Store the returned `PathBuf` unchanged as `canonical_cwd`. Do not call
   `canonicalize`, lexical-normalize, substitute, or otherwise rewrite it again.

The exact `canonical_cwd` value is cloned into
`OperationIntent::Process { cwd: ... }` and passed to
`Command::current_dir(&canonical_cwd)`. No second canonicalization, lexical
normalization, current-directory lookup, `/tmp` substitution, or alternate
default is permitted between authorization and spawn. Authorization and spawn
therefore consume the same path value/bytes. The current `ShellTool` fallback at
`crates/tools/src/shell_tool.rs:196-200` versus `:228-230` is a latent defect;
the new seam must not inherit it. Tests use a direct executable to record its
actual cwd and compare it byte-for-byte with the broker-bound canonical path,
including a relative-input/canonical-output case.

### Broker and no-spawn invariants

- Resolve the broker decision before `Command::spawn`.
- `Decision::Allow` is necessary but not sufficient for a human-gated intent;
  an explicit covering grant is required for `RequireHuman`.
- `Decision::Deny`, `HumanRequired`, and `InvalidGrant` return no PID, no
  output, no process-group ID, and no durable dispatch/output-store side effect.
- `OperationIntent::Tool` is not a substitute for the exact process intent.
- Recheck the deadline and cancellation receiver after authorization and before
  spawn; use the precedence rule below.

## Lifecycle and state machine

| State/event | Required action | Observable result/invariant |
|---|---|---|
| `Created` | Validate request, argv/env/cwd/limit sizes; create the request deadline | Invalid input returns before broker/spawn. |
| `CancellationRequested` before authorization | Latch cancellation and stop | Return `CancelledBeforeStart`; no broker grant consumption and no spawn. |
| `Authorized` | Store exact decision for this request only | No implicit permission retention. |
| `Denied` / `HumanRequired` | Return typed denial | No PID, no child, no side effect. |
| `SpawnPending` | Recheck cancellation, then deadline, using the fixed precedence | Cancellation before spawn yields `CancelledBeforeStart`; deadline yields `TimedOut`; neither spawns. |
| `Spawned` | Create and record the owned Unix process group and child | No unrelated process can enter cleanup scope. |
| `Ready` | Publish one PID/group event | Exactly one readiness event after successful spawn; a send failure enters `CleanupFailed` policy. |
| `Running` | Read both pipes with retained-byte caps; drain excess; await completion | Memory remains bounded; no unbounded pipe wait or deadline reset. |
| `CancelRequested` | Latch cancellation for this request | Trigger affects only this request's owned group; cleanup follows the shared state machine. |
| `Timeout` | Latch timeout and enter the same cleanup path | Terminal state is `TimedOut`, never success. |
| `Exited` | Await child and join readers | Exit code and bounded output are returned; later signals cannot rewrite the result. |
| `Cleanup` | Group kill first; direct child kill fallback; explicit `Child::wait().await`; join readers | `Reaped` only after required steps succeed; otherwise return `CleanupFailed`. |
| `Completed` | Publish one result or one typed error | Future is consumed; no detached owner remains. |
| `Dropped` | Signal emergency cleanup if possible | Best-effort only; caller cannot claim deterministic reap without awaiting completion. |

Cleanup records the four `CleanupStep` values. A failed step or expired
`cleanup_timeout` is represented only by `ProcessError::CleanupFailed`; it is
never an ordinary `Ok(ProcessResult)`, and no later integration decision can
change that representation.

### Race and terminal semantics

The request has one monotonic `Instant` deadline. `timeout` covers the whole
request from entry; `startup_timeout` covers only the interval from the spawn
attempt until the readiness send; `cleanup_timeout` starts when cleanup begins.
Each read, wait, and reader join uses the remaining bounded budget; polling or
reading more output never resets the deadline.

At every observation checkpoint, terminal selection is deterministic:

1. If the child-completion future is already resolved, latch `Exited`.
2. Otherwise, if `cancellation.borrow()` is `true`, latch cancellation
   (`CancelledBeforeStart` before spawn, `Cancelled` after spawn).
3. Otherwise, if the current instant is at or beyond the relevant deadline,
   latch `TimedOut`.
4. Otherwise continue the bounded wait.

The running select must use this branch order (for example, a biased select):
child completion, cancellation change, then deadline. Thus, when cancellation
and timeout are both observable in the same poll, cancellation wins; if child
completion is also already observable, normal exit wins. A later signal cannot
retroactively change a latched terminal state.

- Cancellation before authorization, after authorization but before spawn, or
  after the deadline has also elapsed before spawn returns
  `CancelledBeforeStart`; no PID, group, output, or broker side effect exists.
- Cancellation after spawn and before readiness runs owned cleanup and returns
  `Cancelled`; readiness is sent only if its receiver is still live.
- Cancellation after readiness leaves exactly one readiness event, returns
  `Cancelled`, and reaps the owned group and parent.
- Timeout and cancellation share the cleanup state machine but retain distinct
  terminal reasons. Timeout is `TimedOut`, never success.
- If the child has already exited, normal exit wins; a later cancellation or
  timeout cannot change `Exited`.
- If `ready.send(...)` fails, follow the fixed `ReadinessReceiverClosed`
  policy in `ProcessError`: clean up first, return that error only after a
  `Reaped` status, or return `CleanupFailed` if cleanup cannot be proven.

## Readiness contract

- Use a bounded `oneshot` channel, not an unbounded queue or readiness file.
- Send exactly once after successful spawn, before waiting on stdout/stderr.
- Send child PID and Unix process-group ID, with PID validity checks. On Unix,
  both values must be greater than zero. An invalid PID is returned as
  `ProcessError::Io { operation: "readiness_pid", .. }` after bounded owned
  cleanup is `Reaped`; if that cleanup fails, return
  `ProcessError::CleanupFailed` instead. It is never a readiness success.
- Never send for `Denied`, `HumanRequired`, invalid request, or spawn failure.
- Readiness means the child exists; it does not mean the process has completed.
- A caller that receives readiness must still await the owner future for cleanup
  and result.
- If the receiver is dropped before or during `ready.send`, the send failure is
  not silently ignored. Latch `Cancelled`, run bounded owned cleanup, then
  return `ReadinessReceiverClosed { pid }` only when cleanup is `Reaped`; return
  `CleanupFailed` with a `Failed` status when any required cleanup step fails.
  Do not publish a second readiness event or return a normal `ProcessResult`.

## Resource and lifetime ceilings

Existing constants provide useful compatibility targets; the new public contract
must enforce them at its own boundary rather than trusting callers:

- Existing `ToolExecutor::TimeoutConfig`: default 30 seconds, maximum 300 seconds.
- Existing `ShellTool` retained output ceiling: 10 MiB per stream.
- Existing dispatcher durable output ceiling: 256 KiB per record.
- Existing server shell command ceiling: 64 KiB.
- Existing `shell_bounds` pattern: retain only the configured prefix, drain and
  discard excess, and append a fixed truncation marker.

Required additional bounds are normative at the public seam, not values a
caller can opt out of:

- `timeout`, `startup_timeout`, and `cleanup_timeout` are finite positive
  `Duration`s, each no greater than 300 seconds; `startup_timeout` also may not
  exceed `timeout`;
- `max_stdout_bytes` and `max_stderr_bytes` are each 1..=10 MiB; retained data
  stays within the selected cap, while the fixed truncation marker is outside
  that cap;
- when a stream exceeds its cap, its `*_truncated` flag is `true` and the exact
  marker `"\n[truncated]"` is appended once; otherwise both flags are `false`;
- `max_argv_bytes` is 1..=64 KiB and `max_argv_elements` is 1..=4,096; byte
  counts include every argv element's UTF-8 bytes;
- `max_env_bytes` is 1..=64 KiB and `max_env_elements` is 1..=1,024; byte
  counts include every name and value, and names/values containing NUL are
  rejected;
- `max_cwd_bytes` is 1..=4 KiB and applies to both the input and returned
  canonical path;
- one process owner exists per request, readiness is a single-slot `oneshot`,
  error text is at most 512 bytes, and no pending queue or detached owner is
  permitted.

`kill_on_drop` is only a last-resort signal. It is not proof of process-group
cleanup, reader joining, or child reaping; the owner future or explicit owner
handle must be awaited for the bounded cleanup result.

## Unix and Windows boundary

### Unix

- Set the child as a new process group before spawn.
- Kill only that owned group on cancel/timeout.
- Use group kill before any direct-child fallback.
- Await `Child::wait().await` explicitly.
- Join/drain both output readers before publishing completion.
- Verify no delayed marker and no owned descendant remains in future tests.

### Windows

There is no Windows process-tree backend, Job Object/AppContainer cleanup, or
real-platform proof in this revision. A child-only kill is not equivalent to a
Unix process-group kill. The public contract must return an explicit unsupported
platform result or remain Unix-gated until a Windows backend is implemented and
validated. Do not claim Windows cancellation support from a compile-only or
mocked test.

## Failure and scenario matrix

| Scenario | Required observable behavior |
|---|---|
| Broker `Allow` | Exact process intent spawns; readiness/PID emitted once; result awaited. |
| Broker `Deny` | No spawn, no PID, no marker, no output-store record, no permit leak. |
| Broker `RequireHuman`, no grant | No spawn; explicit `HumanRequired` error. |
| `RequireHuman` with valid grant | One covered operation may spawn; grant is single-use. |
| Invalid/oversized argv/env/cwd/limits | Reject before broker/spawn; no side effect. |
| Cancellation before authorization | Return `CancelledBeforeStart`; no spawn and no unrelated side effect. |
| Cancellation after authorization, before spawn | Return `CancelledBeforeStart`; no PID, group, or output. |
| Cancellation before readiness | Owned child/group is cleaned and reaped; no late marker. |
| Cancellation after readiness | Exactly one readiness event; `Cancelled`; explicit group/parent reap. |
| Cancellation and timeout observed together | Cancellation wins; deterministic `Cancelled`/`TimedOut` branch, never a scheduler-dependent result. |
| Timeout | Bounded result `TimedOut`; group cleanup; explicit reap; no late marker. |
| Normal exit | Explicit wait, bounded stdout/stderr, deterministic exit result. |
| Spawn failure | Typed `Spawn` error; no readiness; no leaked child ownership. |
| Readiness receiver dropped | Bounded cleanup, then `ReadinessReceiverClosed` or `CleanupFailed`; never normal success. |
| Output over cap | Retained data stays within the cap; fixed marker is appended; excess drained/discarded. |
| Reader/pipe failure | Cleanup still runs; `Io` or `CleanupFailed`, never ordinary success. |
| Drop/abort | Emergency signal only; deterministic guarantee requires awaiting owner. |
| Descendant survives parent | Group cleanup kills descendant; test observes bounded death. |
| Unrelated process present | It remains alive; cleanup is scoped to owned process group. |
| Windows target | No support claim; explicit unsupported/blocked result until backend proof. |

## Integration ownership

This lane owns exactly one research artifact and its own ledger row. It does not
edit product or test source.

Future ownership must remain serialized where files overlap:

1. `TOOL-SHELL-BROKER-EXECUTOR`: `crates/tools/src/executor.rs`; broker gate and
   denial semantics after t01-t03 freeze.
2. `TOOL-SHELL-BROKER-DISPATCH`: `crates/tools/src/registry_dispatch.rs`; real
   broker wiring and default-deny dispatch.
3. `TOOL-SHELL-CANCEL-EXECUTOR`: `crates/tools/src/executor.rs`; authorized
   direct-process API and cancellation semantics, after API decision and RED
   freeze. It must be serialized with item 1 because the owned file overlaps.
4. `TOOL-SHELL-SERVER-WIRING`: `crates/server/src/lib.rs`; pass the exact
   process authorization and await the owner. No `OperationIntent::Tool`-only
   shell path.
5. Security integration lane: any capability/grant API change in
   `crates/security/src/`; no ad hoc grant format in the tools crate.
6. Independent test owner: `crates/tools/tests/phase1_shell_cancellation.rs`;
   this research lane cannot author it.

## Future RED contract

This section is subordinate to **Mandatory seam and RED ordering**. The seam
implementation and its independent verification must be accepted first. A RED
authored before then is forbidden, especially a compile-fail or import-fail
caused by the absent public API. The RED lane must use the public seam exactly as
specified; it may not reach into private modules or use a shell-string bypass.

Path: `crates/tools/tests/phase1_shell_cancellation.rs`

Command:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test phase1_shell_cancellation -- --test-threads=1
```

The fixture must:

- use a disposable direct executable, never an opaque shell string;
- pass PID, descendant, and late-marker paths as argv;
- use an explicit scoped broker authorization;
- assert `Deny` and `RequireHuman` cause zero spawn;
- observe exactly one readiness/PID event;
- cancel before readiness and after readiness in separate tests;
- pre-load or send the watch cancellation value before the observation poll for
  deterministic cancellation cases;
- use paused Tokio time plus an explicit deadline advance for timeout cases;
  test the cancellation/deadline tie with both values observable before one
  poll, rather than relying on scheduler timing;
- exercise timeout, normal exit, spawn failure, output cap, readiness-receiver
  drop, and drop/abort;
- assert parent/descendant death within bounded deadlines;
- assert no delayed marker after a bounded settle period;
- assert no permit/store/FD leak;
- assert `duration_ms` is `u64`, is `0` for a sub-millisecond fixture, and
  saturates rather than wrapping for a deliberately injected elapsed value;
- run only on the supported Unix backend unless Windows backend proof exists.

## Validation and landing record

Required bounded checks for this correction:

```sh
rtk git grep -n 'tokio-util\|CancellationToken\|CleanupStatus\|ProcessError\|duration_ms\|compile-fail' -- worklog/TOOL-SHELL-CANCEL-CONTRACT.md
rtk shasum -a 256 crates/tools/tests/phase1_shell_broker.rs
rtk git diff --check
```

The C1-C5 matrix is captured in
`worklog/TOOL-SHELL-CANCEL-CONTRACT-CORRECTION.md`. Expected:

- the scan shows the existing `tokio::sync::watch::Receiver<bool>` seam, the
  named `CleanupStatus`/`ProcessError` definitions, the `u64` duration rule,
  and the explicit compile-fail prohibition; it must not require a
  `CancellationToken` or `tokio-util` dependency;
- frozen broker test hash remains
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`;
- diff check passes;
- no Cargo/heavy process is run for this research landing;
- only this contract, its correction scratchpad, and the existing own ledger
  row are committed;
- branch is pushed without force and remote containment is verified.

## Unresolved gaps

- No public executor-level authorized cancellation API exists yet; the seam
  implementation and its independent verification remain future lanes.
- No cancellation RED exists or is frozen; authoring remains blocked until the
  strict seam/verification ordering above is satisfied.
- Current `ShellTool` timeout/drop path lacks an explicit awaited public cleanup
  result.
- Current server does not inject a broker into `new_shell_execution` and does
  not authorize the exact process intent.
- Current dispatcher default remains `AllowAll`.
- No Windows process-tree backend or real-platform proof exists; Windows remains
  explicitly unsupported/gated.
- `ShellBounds` is a reference module, not a live caller.
- A parent shell journey remains open until the new API, broker wiring,
  cancellation implementation, independent RED verification, and integrated
  rerun are complete.
- This correction is not independently verified or accepted by this research
  lane; a fresh verifier must inspect the pushed contract revision.
