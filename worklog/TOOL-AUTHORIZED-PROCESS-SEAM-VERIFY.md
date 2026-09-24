# TOOL-AUTHORIZED-PROCESS-SEAM-VERIFY

## Claim

- Task: `TOOL-AUTHORIZED-PROCESS-SEAM-VERIFY`
- Type: verification
- Role: Rust process-lifecycle/capability verifier
- Session: `ses_f2c0335b5ffePuDIBoawHPZagL`
- Route: `9router-xk-gpt56-luna`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-tool-authorized-process-seam`
- Branch: `verify/TOOL-AUTHORIZED-PROCESS-SEAM`
- Candidate: `c15c09f957aa81adf409d84f2d92a93b93fce9be`
- Contract: `c630f3f`
- Frozen broker-test hash: `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`

## Scope

Independent public-API seam verification only. No product repair, cancellation RED,
server/registry wiring, frozen-test edits, or test refreeze. Disposable external
probe may be created under `crates/tools/tests/`, then removed before commit.

## Source evidence

- `crates/tools/src/executor.rs:49-145`: public request, limits, readiness,
  terminal, cleanup, error, result, and cancellation types match the accepted
  direct-process seam names and shapes.
- `crates/tools/src/executor.rs:193-350`: request/limit validation and one cwd
  canonicalization occur before authorizer use.
- `crates/tools/src/executor.rs:705-722`: public method signature accepts
  `ProcessRequest`, mutable `ToolAuthorizer`, optional `Grant`, `ExpectedScope`,
  `ProcessLimits`, watch receiver, and readiness oneshot sender.
- `crates/tools/src/executor.rs:744-805`: validation, pre-auth cancellation,
  exact `OperationIntent::Process`, baseline gate, grant gate, and pre-spawn
  cancellation/deadline checks.
- `crates/tools/src/executor.rs:807-917`: exact direct argv/canonical cwd,
  `env_clear`, fixed PATH fallback, Unix process group, owned readers, startup
  check, one readiness send, and receiver-drop cleanup.
- `crates/tools/src/executor.rs:919-1079`: biased child/cancellation/deadline
  observation and explicit cleanup/readers/wait result handling.
- `crates/security/src/tool_authorize.rs:94-161`: baseline broker gate and
  grant authorization; `authorize_with_grant` owns bounded single-use ledger.
- `crates/security/src/app_policy.rs:288-338`: mandatory Deny cannot be lifted;
  valid grants are digest/scope/version/freshness checked and consumed once.
- `crates/security/src/lib.rs:243-300`: Process policy rejects shell `-c` and
  destructive argv before permission rules; ordinary direct argv can Allow.
- `crates/tools/Cargo.toml:14-18` plus workspace `Cargo.toml:39`: existing Tokio
  `sync`/`time` features are available; no dependency edit is needed.

## Probe plan

External integration probe will compile only against public exports. It will cover
normal direct argv and exact canonical cwd, explicit environment clearing, Deny,
HumanRequired without grant, valid grant, wrong/stale/replayed grants, invalid
request/limits/cwd before broker, readiness once, receiver drop cleanup,
post-readiness cancellation, timeout cleanup, and output caps. Startup timeout
source enforcement will be audited separately because no public spawn hook exists.

## Results

External probe `crates/tools/tests/tmp_authorized_process_seam_probe.rs` was
created, run, then deleted. It compiled against only the public API. Two tests
passed. Covered: direct `/bin/echo`, canonical relative cwd observed via
`/bin/pwd`, audit intent equality, `env_clear` plus fixed PATH, baseline Deny,
HumanRequired, exact valid grant, replay with the same authorizer, wrong digest,
stale version, invalid env before authorization, stdout truncation/draining,
post-readiness cancellation and group cleanup, process timeout cleanup,
startup timeout with `1ns`, readiness receiver drop cleanup. No probe child
remained after completion.

Probe did not exercise descendants or a cancellation/exit tie; source audit is
recorded below. No source repair permitted.

## Source audit

- ACCEPTED: direct argv only at this public method; no shell parsing or `-c`.
- ACCEPTED: validation/canonicalization before broker and spawn; one canonical
  `PathBuf` reused in intent and `Command` (`executor.rs:744-765,809-814`).
- ACCEPTED: explicit `env_clear`, fixed PATH fallback, no inherited env
  (`executor.rs:807-820`).
- ACCEPTED: baseline Deny is terminal; grant only follows HumanGate
  (`executor.rs:757-793`; `tool_authorize.rs:122-161`; `app_policy.rs:288-338`).
- ACCEPTED: Unix process group, kill-first cleanup, explicit child wait, reader
  joins, bounded readers (`executor.rs:445-519,807-845,1014-1073`).
- ACCEPTED: startup timeout branch is executable, not validation-only
  (`executor.rs:827-875`); probe forced `1ns` and observed `TimedOut` with
  `CleanupStatus::Reaped`.
- ACCEPTED: non-Unix early `UnsupportedPlatform` (`executor.rs:723-737`).
- GAP: `prepare_canonical_cwd` uses `Path::to_string_lossy().len()` for cwd
  byte ceilings/NUL checks (`executor.rs:197-223`), not raw OS path bytes. No
  non-UTF-8 path probe was run. Contract says cwd byte cap applies to input and
  canonical output; classify as implementation risk for integrator review.
- GAP: no external descendant fixture was run. Source uses `process_group(0)`
  and `kill_process_group` (`executor.rs:825,460-476`), but runtime descendant
  proof remains absent from this verifier.

## Recovery checks and verdict

- Corrected preflight probe `tmp_process_preflight_probe`: 1 passed. Covered
  zero timeout and invalid cwd before broker audit, mandatory Deny, and true
  cancellation before authorization with `CancelledBeforeStart` and closed
  readiness channel. Probe deleted afterward.
- Error-bound probe: 1 passed. Covered NUL-bearing request and invalid limits;
  debug-rendered errors were <=512 bytes and did not contain the sentinel.
  Wrong-grant error path also exercised and remained bounded/redacted. Probe
  deleted afterward.
- Non-UTF-8 cwd probe could not create the fixture on macOS. `create_dir` with
  an invalid byte failed with `Os { code: 92, message: "Illegal byte sequence" }`.
  This is a platform fixture limitation, not proof of compliance.
- Source defect remains real: `prepare_canonical_cwd` calls
  `cwd.to_string_lossy().len()` at lines 202, 207, 220. The accepted contract
  requires OS path byte accounting. Lossy conversion can transform invalid
  bytes and produce a different length. No source repair authorized.
- Cancellation-after-authorization/before-spawn and child/cancel/deadline tie
  have no deterministic public checkpoint. No flaky timing proof authored.
- Existing stale executor tests remain: `execute_success` and
  `execute_timeout`; untouched.
- Candidate verdict: **BLOCKED**. The cwd byte-budget mismatch is a contract
  defect requiring a later implementation repair. Cancellation RED remains
  unauthorized.

## Landing

- Final artifact check: `git diff --check` passed; no `crates/tools/tests/tmp_*`
  probe and no probe child remained. Only this worklog and the verifier claim
  row are persistent changes. No source, frozen test, or cancellation RED edit.
- Resource check: `free -h` showed 6.4 GiB available before landing. Tests ran
  sequentially with `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`.
- Lane status recorded `blocked` through `tools/completion_claims.py`.
