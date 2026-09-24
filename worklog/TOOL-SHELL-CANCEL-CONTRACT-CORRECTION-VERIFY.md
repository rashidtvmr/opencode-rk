# TOOL-SHELL-CANCEL-CONTRACT-CORRECTION-VERIFY

## Identity and authority

- Task: `TOOL-SHELL-CANCEL-CONTRACT-CORRECTION-VERIFY`
- Task type: `verification`
- Session: `ses_f2d0df495ffeaOgqWW9QOCFy1E`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/plan-tool-shell-cancel-contract`
- Branch: `plan/TOOL-SHELL-CANCEL-CONTRACT`
- Verified correction commit: `38593f37b641781a548c768933f6411a43a96eb2`
- Verified artifact: `worklog/TOOL-SHELL-CANCEL-CONTRACT.md`
- No product source, test, Cargo, dependency, platform-proof, or release artifact
  changed. No cancellation RED authored or authorized here.

## C1-C5 correction matrix

| Finding | Corrected evidence | Independent result | Verdict |
|---|---|---|---|
| C1 seam and RED ordering | Contract `:96-126` makes public `ToolExecutor::execute_authorized_process` implementation a hard prerequisite, then fresh seam verification, then compiling behavioral RED. Compile-fail/import-fail RED is forbidden. Types/signature: `:128-262`. | Current `crates/tools/src/executor.rs:136-147` still lacks the seam, so no cancellation RED can accidentally compile against an old/private path. `crates/tools/src/lib.rs:13` already publicly exports `executor`. | PASS |
| C2 types and cancellation primitive | `CleanupStep`, `CleanupStatus`, all `ProcessError` variants, `ProcessResult`, and `ProcessCancellation = tokio::sync::watch::Receiver<bool>`: contract `:168-218`. Readiness-close and cleanup mapping: `:280-315`, `:467-484`. | `crates/tools/Cargo.toml:14,18` enables Tokio process/io/runtime features; workspace `Cargo.toml:39` supplies `sync`/`time`. Existing tools code uses `tokio::sync::oneshot`/`mpsc`. `tokio-util` is only a workspace dependency (`Cargo.toml:40`); proposed type does not need it. | PASS |
| C3 canonical cwd identity | Contract `:370-392` requires one `std::fs::canonicalize`, preserves returned `PathBuf`, clones the same value into `OperationIntent::Process`, and passes it to `Command::current_dir(&canonical_cwd)`. No `/tmp` fallback or second rewrite. | Existing defect is explicitly recorded at `:388-392`, matching `crates/tools/src/shell_tool.rs:196-200,228-230`. Existing process intent has exact program/argv/cwd shape at `crates/security/src/lib.rs:47-51`. | PASS |
| C4 terminal race | Contract `:429-465` defines one monotonic deadline and precedence: child completion, cancellation, deadline. It requires biased branch order; cancellation wins cancellation/deadline ties; already-resolved child completion wins. Future RED requires preloaded watch state and paused Tokio time: `:607-627`. | No scheduler-dependent tie remains. Cleanup/readiness, terminal latching, explicit wait, reader joins, and no-late-marker requirements are stated at `:405-427`, `:455-465`, `:480-484`. | PASS |
| C5 duration type/conversion | Contract `:231-239` fixes `ProcessResult.duration_ms` to `u64`, requires `u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)`, defines sub-ms as `0`, and keeps legacy `ShellResult.duration_ms: u128` distinct. | Existing `ToolResult.duration_ms` is `u64` at `crates/tools/src/executor.rs:74-87`; no incompatible dependency/type is required. | PASS |

## Public-type and authorization audit

- `ToolAuthorizer` is public at `crates/security/src/tool_authorize.rs:92-110`;
  `authorize`/`authorize_with_grant` are public at `:122-162`. `Grant` and
  `ExpectedScope` are public at `crates/security/src/app_policy.rs:125-194`.
- `PermissionBroker::authorize` dispatches exact `OperationIntent::Process` at
  `crates/security/src/lib.rs:206-248,290-300`. `app_policy::decide` enforces
  baseline-first, scope/digest/freshness, and single-use grant semantics at
  `crates/security/src/app_policy.rs:288-339`.
- Seam must preserve `Denied`, unresolved `HumanRequired`, and invalid supplied
  grants. No security API change is needed. Any security API change needs a
  separate integration proposal.
- `OperationIntent::Process` does not include environment bytes. Contract does
  not claim grant digest binding for `env`; it requires `env_clear`, explicit
  environment, bounded validation, and no inherited secrets at `:347-368`.
  Seam must not add inherited environment or claim stronger grant binding.

## Race, lifetime, resource, platform audit

- Pre-authorization cancellation, post-authorization pre-spawn cancellation,
  broker denial, unresolved human gate, invalid grant, and invalid input prohibit
  spawn and durable side effects (`:394-420`, `:544-567`).
- Post-spawn cancellation/timeout share group-kill, child-kill fallback,
  explicit `Child::wait`, and reader-join cleanup. `Reaped` requires all required
  steps to succeed; `Failed` cannot be ordinary success (`:294-315`, `:420-427`).
- Readiness is one `oneshot`; receiver closure has typed post-cleanup result and
  no ordinary success (`:467-484`). No unbounded queue or detached owner.
- Bounds: finite positive timeouts, max 300 seconds; output 1..=10 MiB per
  stream; argv 1..=64 KiB and 1..=4,096 elements; env 1..=64 KiB and 1..=1,024
  elements; cwd 1..=4 KiB; errors max 512 bytes (`:220-229`, `:486-522`).
- Unix process-group support required. Windows explicitly unsupported until a
  real process-tree backend and platform proof exist (`:524-542`).

## Stale-gap audit

- No corrected-contract OPEN, undefined cleanup type, unresolved readiness
  policy, `CancellationToken`, or `tokio-util` requirement remains. Historical
  OPEN claims belong only to the prior verifier artifact, not the corrected
  contract.
- `ProcessError`/`CleanupStatus` are concrete definitions, not placeholders.
- `git grep` scan passed with 33 matches; all are definitions or explicit
  prohibitions.
- Frozen broker hash unchanged:
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.

## Validation

- `rtk git grep -n 'tokio-util\|CancellationToken\|CleanupStatus\|ProcessError\|duration_ms\|compile-fail' -- worklog/TOOL-SHELL-CANCEL-CONTRACT.md` -> 33 matches; scan passed.
- `rtk shasum -a 256 crates/tools/tests/phase1_shell_broker.rs` -> `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.
- `rtk git diff --check` -> exit 0.
- Independent semantic parser -> `C1-C5 semantic contract checks PASS`.
- No Cargo, browser, database, platform, or heavy process run.

## Verdict and next-lane boundary

**ACCEPT for seam implementation only.** Commit `38593f37b641781a548c768933f6411a43a96eb2` closes C1-C5. This does not authorize cancellation RED, cancellation implementation acceptance, broker GREEN, server wiring, Windows support, or parent/release acceptance.

Next authorized sequence:

1. Implement the exact public seam in `crates/tools/src/executor.rs`; no new
   dependency. Preserve frozen broker test source/hash and t01-t03 behavior.
   `crates/tools/src/lib.rs` is already wired.
2. Independently verify the pushed public revision against direct-argv broker
   fixtures: `Deny`, `RequireHuman` without grant, valid grant, invalid request,
   no-spawn, exact cwd identity, readiness, and unchanged broker hash.
3. Only after that verifier accepts the exact pushed API revision may the RED
   owner create `crates/tools/tests/phase1_shell_cancellation.rs`. Its failure
   must be behavioral, never missing import/type/method.

Remaining gaps: no seam exists at this revision, no cancellation RED is frozen,
legacy shell/server/dispatcher paths remain unwired, and Windows process-tree
proof is unavailable.
