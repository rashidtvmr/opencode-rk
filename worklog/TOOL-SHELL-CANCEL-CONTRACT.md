# TOOL-SHELL-CANCEL-CONTRACT

Status: research contract only. No product source, test source, manifest, or release
claim is changed by this lane.

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

## Proposed public contract

The following is the integration contract, not a claim that these symbols exist
in the current revision.

```rust
pub struct ProcessRequest {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
}

pub struct ProcessLimits {
    pub timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_argv_bytes: usize,
    pub max_env_bytes: usize,
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

pub struct ProcessResult {
    pub terminal: ProcessTerminal,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub cleanup: CleanupStatus,
    pub duration_ms: u64,
}
```

Recommended public entrypoint on `ToolExecutor`:

```rust
pub async fn execute_authorized_process(
    &self,
    request: ProcessRequest,
    authorizer: &mut ToolAuthorizer<'_>,
    grant: Option<&Grant>,
    expected: &ExpectedScope,
    limits: ProcessLimits,
    cancellation: CancellationToken,
    ready: tokio::sync::oneshot::Sender<ProcessReady>,
) -> Result<ProcessResult, ProcessError>;
```

The security module remains the authority for constructing/validating the
authorization. The execution module must not accept `AllowAll`, a boolean policy
hook, a timeout, or an opaque command string as proof of authorization. The
implementation may use a typed capability instead of the exact parameter names
above, but it must preserve these observables and ownership rules.

### Request and environment invariants

- `program`, every argv element, and `cwd` are the exact values placed in
  `OperationIntent::Process` and in `Command`.
- No shell parsing, interpolation, concatenation, or implicit `-c` exists on the
  authorized direct-process path.
- `env` is explicit. `env_clear()` is mandatory. No secret-bearing inherited
  variable is available unless explicitly supplied and policy-authorized.
- PATH is a fixed minimal value unless the request explicitly supplies one.
- Empty program, NUL-containing values, invalid cwd, oversized argv/env, and
  invalid limits fail before spawn.
- A grant, if needed, remains scoped to the exact operation digest, workspace,
  session, requester, policy version, and expiry. It is single-use.

### Broker and no-spawn invariants

- Resolve the broker decision before `Command::spawn`.
- `Decision::Allow` is necessary but not sufficient for a human-gated intent;
  an explicit covering grant is required for `RequireHuman`.
- `Decision::Deny` and unresolved `RequireHuman` return no PID, no output, no
  process-group ID, and no durable dispatch/output-store side effect.
- `OperationIntent::Tool` is not a substitute for the exact process intent.
- Recheck cancellation after authorization and before spawn.

## Lifecycle and state machine

| State/event | Required action | Observable result/invariant |
|---|---|---|
| `Created` | Validate request, argv/env/cwd/limit sizes | Invalid input returns before broker/spawn. |
| `CancellationRequested` before authorization | Stop immediately | No broker grant consumption if no authorization was needed; no spawn. |
| `Authorized` | Store exact decision for this request only | No implicit permission retention. |
| `Denied` / `HumanRequired` | Return typed denial | No PID, no child, no side effect. |
| `SpawnPending` | Recheck cancellation; enforce deadline | Cancellation before spawn yields no child. |
| `Spawned` | Create owned process group; record child | No unrelated process can enter cleanup scope. |
| `Ready` | Publish one PID/group event | Exactly one readiness event after successful spawn. |
| `Running` | Read both pipes with retained-byte caps; drain excess | Memory remains bounded; no unbounded pipe wait. |
| `CancelRequested` | Mark request cancellation | Trigger affects only this request's owned group. |
| `Timeout` | Enter same cleanup path as cancellation | Terminal state is `TimedOut`, never success. |
| `Exited` | Await child and join readers | Exit code and bounded output are returned. |
| `Cleanup` | Group kill first; direct child kill fallback; explicit `Child::wait().await`; join readers | No zombie or owned descendant remains when status is `Reaped`. |
| `Completed` | Publish one result | Future is consumed; no detached owner remains. |
| `Dropped` | Signal emergency cleanup if possible | Best-effort only; caller cannot claim deterministic reap without awaiting completion. |

Cleanup must record whether group kill, child kill, wait, and reader joins
succeeded. A cleanup failure must never be reported as ordinary success. It is a
terminal error or an explicit failed `CleanupStatus`; the integration lane must
choose one stable representation before freezing RED assertions.

### Race and terminal semantics

- Cancellation before readiness: either no spawn (if observed before spawn), or
  spawn followed by owned group cleanup; no late side effect.
- Cancellation after readiness: readiness remains observable once; result is
  `Cancelled`; group and parent are reaped.
- Timeout and cancellation are the same cleanup state machine, with distinct
  terminal reasons.
- If the child has already exited, normal exit wins; a later cancellation request
  cannot retroactively change a completed result.
- If cancellation is observed before spawn, return `CancelledBeforeStart`.
- If readiness channel delivery fails, do not continue as if the caller received
  ownership; either cancel the child before returning or expose a terminal
  ownership error. The integration lane must freeze the chosen policy.

## Readiness contract

- Use a bounded `oneshot` channel, not an unbounded queue or readiness file.
- Send after successful spawn, before waiting on stdout/stderr.
- Send child PID and Unix process-group ID, with PID validity checks.
- Send exactly once; duplicate publication is an implementation error.
- Never send for `Denied`, `HumanRequired`, invalid request, or spawn failure.
- Readiness means the child exists; it does not mean the process has completed.
- A caller that receives readiness must still await the owner future for cleanup
  and result.

## Resource and lifetime ceilings

Existing constants provide useful compatibility targets; the new public contract
must enforce them at its own boundary rather than trusting callers:

- Existing `ToolExecutor::TimeoutConfig`: default 30 seconds, maximum 300 seconds.
- Existing `ShellTool` retained output ceiling: 10 MiB per stream.
- Existing dispatcher durable output ceiling: 256 KiB per record.
- Existing server shell command ceiling: 64 KiB.
- Existing `shell_bounds` pattern: retain only the configured prefix, drain and
  discard excess, and append a fixed truncation marker.

Required additional bounds before a RED can be frozen:

- argv byte count and element count;
- environment byte count, element count, and name/value validation;
- cwd byte limit and canonicalization policy;
- one process owner per request, with no unbounded pending queue;
- bounded startup/readiness deadline;
- bounded cleanup/reap deadline;
- bounded error text, with no secret values retained.

Cancellation is caller-owned through a scoped token/channel. The execution
future or explicit owner handle must be awaited. `kill_on_drop` is only a last
resort signal; it is not proof of process-group cleanup or reaping.

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
| Broker `Allow` | Exact process spawns; readiness/PID emitted once; result awaited. |
| Broker `Deny` | No spawn, no PID, no marker, no output-store record, no permit leak. |
| Broker `RequireHuman`, no grant | No spawn; explicit human-required result. |
| `RequireHuman` with valid grant | One covered operation may spawn; grant is single-use. |
| Invalid/oversized argv/env/cwd | Reject before broker/spawn; no side effect. |
| Cancellation before authorization | No spawn and no unrelated side effect. |
| Cancellation after authorization, before spawn | `CancelledBeforeStart`; no PID. |
| Cancellation before readiness | Owned child/group killed and reaped; no late marker. |
| Cancellation after readiness | Exactly one readiness event; cancellation cleanup; explicit reap. |
| Timeout | Bounded result `TimedOut`; group cleanup; explicit reap; no late marker. |
| Normal exit | Explicit wait, bounded stdout/stderr, deterministic exit result. |
| Spawn failure | Typed error; no readiness; no leaked child ownership. |
| Output over cap | Retained bytes stay within cap plus fixed marker; excess drained/discarded. |
| Reader/pipe failure | Cleanup still runs; failure cannot be reported as ordinary success. |
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
- exercise timeout, normal exit, spawn failure, output cap, and drop/abort;
- assert parent/descendant death within bounded deadlines;
- assert no delayed marker after a bounded settle period;
- assert no permit/store/FD leak;
- run only on the supported Unix backend unless Windows backend proof exists.

## Validation and landing record

Required bounded checks:

```sh
rtk git grep -n 'execute_authorized\|execute_with_startup\|ShellTool\|execute_shell' -- crates
rtk shasum -a 256 crates/tools/tests/phase1_shell_broker.rs
rtk git diff --check
```

Expected:

- source citations resolve at candidate revision;
- frozen broker test hash remains
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`;
- diff check passes;
- no Cargo/heavy process is run for this research landing;
- only this worklog and the existing claim row are committed;
- branch is pushed without force and remote containment is verified.

## Unresolved gaps

- No public executor-level authorized cancellation API exists yet.
- No cancellation RED exists or is frozen.
- Current `ShellTool` timeout/drop path lacks an explicit awaited public cleanup
  result.
- Current server does not inject a broker into `new_shell_execution` and does
  not authorize the exact process intent.
- Current dispatcher default remains `AllowAll`.
- No Windows process-tree backend or real-platform proof exists.
- `ShellBounds` is a reference module, not a live caller.
- Cleanup-error representation, readiness-channel failure policy, and exact
  resource constants require integration-owner decisions before RED freeze.
- A parent shell journey remains open until the new API, broker wiring,
  cancellation implementation, independent RED verification, and integrated
  rerun are complete.
