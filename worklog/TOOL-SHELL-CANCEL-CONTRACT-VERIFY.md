# TOOL-SHELL-CANCEL-CONTRACT-VERIFY

Status: independent verification only. No contract, product, test, manifest, or
release artifact is changed by this lane. This artifact does not author RED and
does not authorize implementation by itself.

## Claim and scope

- Task: `TOOL-SHELL-CANCEL-CONTRACT-VERIFY`
- Task type: `verification`
- Session: `ses_f2d45606dffeR8fPUUtbT3Z7FV`
- Branch: `plan/TOOL-SHELL-CANCEL-CONTRACT`
- Verified artifact: `worklog/TOOL-SHELL-CANCEL-CONTRACT.md`
- Verified revision: `da003a9c14d3a4c9907079e2bbee9967172e0641`
- Frozen broker suite SHA-256:
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`
- Product/test/platform proof: none run or claimed. Windows process-tree proof
  remains unavailable.

## Authority and method

Verified current behavior by reading the cited source at the verified revision.
A contract claim is PASS only when the cited file:line/symbol resolves and the
described behavior matches the code. Research text is a claim to verify, not
authority. No Cargo, test, browser, database, or heavy process was run; checks are
bounded text/Git/Python only.

## Source-citation matrix

| # | Contract claim | Cited evidence | Resolved | Match | Verdict |
|---|---|---|---|---|---|
| C1 | `bash`/`shell` route directly to `execute_shell` | `crates/tools/src/executor.rs:108-134` | yes: `execute` routes `bash`/`shell` at :118-119 | exact | PASS |
| C2 | `execute_authorized` handles only `read`, shell falls back | `executor.rs:136-147` | yes: `if call.name != "read" { return self.execute(call) }` | exact | PASS |
| C3 | `execute_shell` builds `bash -c`; timeout wraps `output()`, no child reap on timeout | `executor.rs:240-297` | yes: `:246` `Command::new("bash").arg("-c")`, `:248` `timeout(.., cmd)` | exact | PASS |
| C4 | Default dispatch policy is `AllowAll` | `registry_dispatch.rs:64-78,131-145` | yes: trait :66-68, `AllowAll` :71-78, `new()` :133-135 | exact | PASS |
| C5 | Boolean preflight is not a process broker; execution uses unbrokered executor | `registry_dispatch.rs:187-207` | yes: policy check :193, `executor.execute(call)` :203 | exact | PASS |
| C6 | Server builds `ShellTool` with no broker injected | `crates/server/src/lib.rs:1071-1095` | yes: `ShellTool::new("bash", ["-c", ...])` :1075-1084, no `.broker` | exact | PASS |
| C7 | Server authorizes `OperationIntent::Tool` then constructs shell; no exact process intent | `server/src/lib.rs:1388-1437` | yes: `:1390-1393` `OperationIntent::Tool`, `:1395` `new_shell_execution` | exact | PASS |
| C8 | `ShellTool` broker optional; when present program/argv/cwd authorized pre-spawn; `None` legacy bypass | `shell_tool.rs:83-97,145-149,189-211` | yes: `authz: Option<PermissionBroker>` :93, `.broker` :146-149, intent+`authorize` :192-211 | exact | PASS |
| C9 | `env_clear`, bounded PATH, Unix process group, kill-on-drop | `shell_tool.rs:213-237` | yes: `:215` env_clear, `:221-226` PATH, `:234` `process_group(0)`, `:235` `kill_on_drop(true)` | exact | PASS |
| C10 | Normal path awaits `Child::wait`; timeout returns without explicit group-kill/wait | `shell_tool.rs:270-315` | yes: `:276` `child_ref.wait()`, timeout `:312-314` maps to `Timeout` with no kill/wait | exact | PASS |
| C11 | `cancel`/`Drop` issue best-effort kills; no awaited cleanup result | `shell_tool.rs:340-367` | yes: `:341-350` `kill_process_group`/`start_kill`, `:358-367` Drop calls cancel | exact | PASS |
| C12 | Retained bytes capped while excess drained | `shell_tool.rs:394-412` | yes: `read_bounded` `retained_limit = limit+1`, discards rest | exact | PASS |
| C13 | `Process` binds program/argv/cwd; `Deny`/`RequireHuman` non-allow; `-c` human-gated | `crates/security/src/lib.rs:42-71,206-227,290-300` | yes: `Process` :47-51, `Decision` :63-72, `authorize` :206-227, `authorize_process` :290-301 (shell `-c` :292-294) | exact | PASS |
| C14 | Direct-argv intent + scoped/digest/single-use grant exist | `security/src/tool_authorize.rs:63-78,122-160`; `app_policy.rs:288-338` | yes: `shell_intent` :63-69, `argv_intent` :73-79, `authorize` :124-136, `authorize_with_grant` :139-162, `decide` :291-339 | exact | PASS |
| C15 | `shell_bounds` is a bounded reference, not wired into live path | `shell_bounds.rs:10-23,119-158,281-320` | yes: module exported `lib.rs:57`, zero live callers outside its own file | exact | PASS |

All 15 citations resolve and match. No fabricated citation found.

## Resource-ceiling matrix

| Ceiling | Contract value | Source | Verdict |
|---|---|---|---|
| Tool timeout default/max | 30 s / 300 s | `executor.rs:36-43` | PASS |
| Shell retained output | 10 MiB/stream | `shell_tool.rs:21` (`MAX_OUTPUT_BYTES`) | PASS |
| Dispatcher durable record | 256 KiB | `registry_dispatch.rs:46` (`DEFAULT_MAX_OUTPUT_BYTES`) | PASS |
| Server shell command | 64 KiB | `server/src/lib.rs:123` (`MAX_SHELL_COMMAND_BYTES`) | PASS |
| Retain-prefix drain-discard pattern | reference | `shell_bounds.rs:119-158,192-243` | PASS |
| argv/env/cwd element+byte, readiness, cleanup deadlines | required, values open | contract :236-244, :370-371 | OPEN (declared) |

## Constructibility and authorization-forgery review

- Public surface used by the proposed signature exists: `ToolAuthorizer` and
  `ToolAuthorizer::new` (`tool_authorize.rs:94-110`), `Grant`+`Grant::new` with
  public fields (`app_policy.rs:128-150`), `ExpectedScope` public fields
  (`app_policy.rs:187-194`), `OperationIntent::Process` (`security/lib.rs:47-51`).
- Binding is real, not decorative: `Grant::covers` checks expiry, policy
  version, digest equality, workspace/session/requester scope
  (`app_policy.rs:155-182`); `decide` enforces baseline-first (mandatory `Deny`
  never lifted) and single-use replay via `GrantLedger`
  (`app_policy.rs:291-339`, `:249-286`). The proposed API consumes a
  `ToolAuthorizer`, so the single-use ledger and broker precedence already exist.
- Forgery boundary: authorization is capability-based, not cryptographic. A
  caller that constructs its own broker/`ExpectedScope` can authorize; this is
  the existing `app_policy::decide` trust model (caller-supplied `now` and
  `policy_version`). The contract does not claim an OS sandbox or a signed
  capability, and it explicitly refuses a shell-string sandbox claim. Within
  that honest boundary the digest+scope+single-use binding is unforgeable for a
  retargeting attacker. PASS.
- `AllowAll`/boolean/timeout/opaque-string cannot serve as proof: the proposed
  type takes a broker-backed authorizer, not `DispatchPolicy`. PASS.
- Defect to flag (not in the contract's current-behavior table): `ShellTool`
  authorizes `cwd` as `self.cwd or "/tmp"` (`shell_tool.rs:196-200`) but only
  sets `Command::current_dir` when `cwd` is `Some` (`:228-230`); with `cwd =
  None` the child runs in the parent cwd while the broker saw `/tmp`. The
  contract's exact-cwd invariant is correct as a new requirement but the current
  mismatch is a latent authorization/exec asymmetry the new path must not
  inherit. Correction C3.

## State and race matrix

Every state and scenario the brief requires is present in the contract.

| Requirement | Present | Deterministic outcome | Verdict |
|---|---|---|---|
| Authorization before spawn | yes (`SpawnPending`, broker section) | Deny/HumanRequired: no PID/side effect | PASS |
| Cancellation before authorization | yes (`CancellationRequested`) | no spawn | PASS |
| Cancellation after auth before spawn | yes | `CancelledBeforeStart`, no PID | PASS |
| Cancellation before readiness | yes | owned group killed+reaped, no late marker | PASS |
| Cancellation after readiness | yes | exactly one readiness event, reap | PASS |
| Timeout | yes | `TimedOut` via same cleanup machine | PASS |
| Spawn | yes | owned process group; no unrelated entry | PASS |
| Readiness | yes | exactly one event, PID validity | PASS |
| Exit | yes | explicit wait, bounded output | PASS |
| Drop | yes | best-effort only, no deterministic claim | PASS |
| Output cap | yes | retain cap+marker, drain excess | PASS |
| Reader failure | yes | cleanup still runs, never ordinary success | PASS |
| Descendant cleanup | yes | group kill reaches descendant | PASS |
| Reap | yes | explicit `Child::wait().await`, no zombie | PASS |
| Reap race (timeout vs cancel tie) | partial | contract says distinct terminal reasons but no precedence when deadline and token fire together | CORRECTION C4 |
| Readiness-send failure policy | open | integration owner to freeze | OPEN (declared :209-211) |
| Double-cancel idempotency | partial | implied by token; not asserted | minor |

## Security review

| Invariant | Verdict |
|---|---|
| Deny/RequireHuman fail before spawn | PASS (matrix + existing `decide` baseline) |
| Exact-process authorization bound to program/argv/cwd digest | PASS (digest binding verified) |
| No shell-string sandbox claim | PASS (contract states it; `authorize_process` human-gates `-c`) |
| No inherited secrets | PASS (mandatory `env_clear`, explicit env, no secret retained) |
| Kill only owned process tree | PASS (process group owned; unrelated-process scenario) |
| Unknown cleanup-error representation | OPEN (declared, `CleanupStatus` undefined) |

## Platform review

- Unix: process group, group kill, explicit wait, reader join, no-delayed-marker
  assertions. Matches available primitives (`rustix` kill_process_group,
  `shell_tool.rs:18,340-345`). PASS.
- Windows: contract asserts no Windows process-tree backend, refuses a
  compile-only/mocked claim, and requires an explicit unsupported/gated result.
  Consistent with `platform_matrix.rs` (declares `Os::Windows` capability table
  but no process-tree cleanup) and `app_policy::require_os_sandbox`
  (`app_policy.rs:362-371` fails closed). PASS.

## Future-RED feasibility

- Path `crates/tools/tests/phase1_shell_cancellation.rs` is a new file; frozen
  `phase1_shell_broker.rs` (t01-t03, hash unchanged) is not touched. No conflict.
- Command `cargo test -p opencode-rk-tools --test phase1_shell_cancellation` is
  a valid separate target.
- Blocker: no public executor-level authorized-cancellation API exists
  (`executor.rs:136-147` handles only `read`; `ShellTool` has no broker-injected
  cancellation result). A RED authored against the proposed
  `execute_authorized_process` would fail to compile. The accepted split
  proposal forbids a compile-fail as invalid RED. The contract's "Future RED
  contract" section does not restate this prerequisite sharply enough, so an
  orphaning of the RED lane against a nonexistent symbol is possible.
  Correction C1 (blocking for RED authorization).
- Dependency check: `CancellationToken` is a `tokio-util` type; `tokio-util` is
  a workspace dependency (`Cargo.toml:40`) but is NOT a dependency of
  `crates/tools` (`crates/tools/Cargo.toml`). `tokio::sync::oneshot` is
  available via the workspace `sync` feature. The proposed signature is a
  shape; the chosen API must use an available cancellation type or add the
  dependency through an integration decision. Correction C2.

## Verdict

ACCEPT WITH CORRECTIONS.

The contract is source-grounded (15/15 citations resolve), secure within the
existing capability model, and its state/scenario matrix covers every required
lifecycle, race, bound, platform, and ownership item with deterministic
outcomes. It is not yet precise enough to authorize a cancellation RED lane
unconditionally.

### Required corrections before RED authoring is authorized

- C1 (blocking): `## Future RED contract` must state that the public authorized
  seam is a hard prerequisite, that a RED against a nonexistent symbol is a
  forbidden compile-fail, and that the RED lane opens only after the API/
  implementation owner exposes the chosen entry point.
- C2 (blocking for signature freeze): define `CleanupStatus` and `ProcessError`
  (or explicitly mark both as integration-owner placeholders), and replace
  `CancellationToken` with an available type or record the `tokio-util`
  dependency decision.
- C3 (correctness): require the identical `cwd` in `OperationIntent::Process`
  and `Command::current_dir`, and call out the current `ShellTool` `/tmp`
  fallback mismatch (`shell_tool.rs:196-200` vs `:228-230`) as a defect the new
  path must not inherit.
- C4 (determinism): freeze terminal precedence when timeout deadline and
  cancellation token fire together.
- C5 (minor): unify `duration_ms` type against `ShellResult` (`u128` vs `u64`),
  and state that grant/scope honesty is caller-supplied within the capability
  model (no signed capability, no OS sandbox claim).

ACCEPT (unconditional RED authorization) is withheld until C1-C2 are addressed;
C3-C5 are non-blocking but must be resolved before freeze.

## Validation performed

```sh
rtk git rev-parse HEAD
rtk shasum -a 256 crates/tools/tests/phase1_shell_broker.rs
rtk git diff --check
rtk git grep -n 'execute_authorized|execute_with_startup|ShellTool|execute_shell|OperationIntent::Process' -- crates
rtk python3 -c '<contract headings/states/scenarios/symbol check>'
```

Observed:
- HEAD = `da003a9c14d3a4c9907079e2bbee9967172e0641`.
- broker suite hash = `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8` (matches frozen).
- `git diff --check` clean.
- all required contract headings/states/scenarios present; `CleanupStatus` and
  `ProcessError` referenced but undefined.
- branch containment: `origin/plan/TOOL-SHELL-CANCEL-CONTRACT` contains HEAD.
- no Cargo/heavy process run.

## Unresolved gaps

- Windows process-tree proof unavailable; platform support remains an explicit
  unsupported/gated state, not a proof.
- Cleanup-error representation, readiness-channel-failure policy, exact resource
  constants, and timeout/cancel precedence remain integration-owner decisions.
- Shell-broker GREEN is a separate active lane and is not assumed complete.
- This verdict authorizes RED authoring only after C1-C2 corrections.
