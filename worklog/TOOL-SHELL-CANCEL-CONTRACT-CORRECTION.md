# TOOL-SHELL-CANCEL-CONTRACT-CORRECTION scratchpad

## Claim and route

- Task: `TOOL-SHELL-CANCEL-CONTRACT-CORRECTION`
- Type: `research`
- Session: `ses_f2d25ff53ffeI8JA2mrF6iLY1Y`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/plan-tool-shell-cancel-contract`
- Branch: `plan/TOOL-SHELL-CANCEL-CONTRACT`
- Owned file: `worklog/TOOL-SHELL-CANCEL-CONTRACT.md`
- Permitted companion: this scratchpad; own ledger row only
- Route: `oc-space-bunny-free`; allowlist permission confirmed
- Base HEAD: `2c7575362ff73a17134767068809c7513651f501`
- Verifier evidence revision: `da003a9c14d3a4c9907079e2bbee9967172e0641`

## Source evidence

- `worklog/TOOL-SHELL-CANCEL-CONTRACT-VERIFY.md:157-188`: `ACCEPT WITH
  CORRECTIONS`; C1/C2 block RED authorization, C3-C5 required before freeze.
- `crates/tools/src/executor.rs:108-147`: shell routes through unbrokered
  `execute`; `execute_authorized` handles only `read`.
- `crates/tools/src/executor.rs:240-297`: `bash -c`, timeout around output,
  no owned child cleanup on timeout.
- `crates/tools/src/shell_tool.rs:23-33`: legacy `ShellResult.duration_ms` is
  `u128`; `:192-230`: broker sees `/tmp` when cwd is absent while command
  inherits the parent cwd; `:270-367`: timeout/drop cleanup is not an awaited
  public result.
- `crates/tools/Cargo.toml:7-14`: Tokio is present with `io-util`, `rt`, and
  `process`; workspace `Cargo.toml:39-40` provides `sync` and `tokio-util`, but
  `crates/tools/Cargo.toml` does not depend on `tokio-util`.
- `crates/security/src/lib.rs:42-71,206-227,290-300`: exact `Process` intent,
  `Allow`/`Deny`/`RequireHuman`, and opaque `-c` human gate.
- `crates/security/src/tool_authorize.rs:63-78,122-160` and
  `crates/security/src/app_policy.rs:288-338`: direct-argv intent plus scoped,
  digest-bound, single-use grant path.

## Observed scenario and target boundary

The verifier found a precise contract, not an implementation: C1 lacked a hard
seam/verification dependency edge; C2 left cleanup/error types and cancellation
primitive unresolved; C3 exposed the `/tmp` authorization/spawn mismatch; C4
left simultaneous timeout/cancel selection open; C5 left duration type and
conversion inconsistent. This lane edits only the contract. It authors no RED,
touches no Rust source/test/manifest, adds no dependency, and claims no platform
or release acceptance.

## C1-C5 correction matrix

| Finding | Contract location | Correction recorded | Evidence/acceptance boundary |
|---|---|---|---|
| C1 | `Mandatory seam and RED ordering`; `Future RED contract` | Public `execute_authorized_process` seam implementation, then fresh independent seam verification, then compiling behavior RED; compile/import-fail RED forbidden; strict dependency edge stated twice. | No RED may be claimed until exact public revision is independently verified. |
| C2 | `Proposed public contract`; cancellation semantics | Defines `CleanupStep`, `CleanupStatus::{NotRequired,Reaped,Failed}`, every `ProcessError` variant, readiness-send policy, and `ProcessCancellation = tokio::sync::watch::Receiver<bool>` with closed/false/true semantics. | Existing Tokio `sync` only; no `CancellationToken`/`tokio-util` dependency. |
| C3 | `Canonical cwd identity` | Mandatory `PathBuf`; one validation/canonicalization pass; returned `PathBuf` reused unchanged for intent and `Command::current_dir`; no `/tmp` fallback or second rewrite. | Current ShellTool defect is recorded as a non-inherited mismatch. |
| C4 | `Race and terminal semantics`; `Future RED contract` | One absolute timeout; child completion first, cancellation second, deadline third; cancellation wins cancellation/timeout ties; preloaded watch and paused-time tests required. | Scheduler-independent branch order and bounded cleanup are explicit. |
| C5 | `ProcessResult.duration_ms`; resource ceilings | Exactly `u64`; `u64::try_from(as_millis()).unwrap_or(u64::MAX)`; sub-ms is `0`; no unchecked cast; legacy `ShellResult` `u128` is not silently unified. | Matches `ToolResult` and bounded public result surfaces. |

## Decisions

- Readiness sender failure: latch `Cancelled`, run bounded cleanup, return
  `ReadinessReceiverClosed` only after `Reaped`; otherwise return
  `CleanupFailed` with `CleanupStatus::Failed`.
- Normal exit: `Reaped`, wait/readers `Succeeded`, kill steps `NotRequired`.
- Pre-spawn cancellation: `CancelledBeforeStart`; post-spawn cancellation:
  `Cancelled`; timeout: `TimedOut`; cleanup failure never becomes `Ok`.
- Public limits are finite and positive, with hard caps: 300s timeouts, 10 MiB
  per retained stream, 64 KiB/4096 argv, 64 KiB/1024 env, 4 KiB cwd.
- Grant and expected scope remain caller-supplied capability inputs within the
  existing broker model; no signed-capability or OS-sandbox claim.

## Validation

- Prior literal validator failed only on markdown wrapping: it expected
  `compile-fail/import-fail RED is forbidden` as one literal string, while the
  contract renders that identical sentence across a line break. Exact failure:
  `C1 missing: ['compile-fail/import-fail RED is forbidden']`.
- Corrected semantic validator command (whitespace-normalized, regex-checked;
  no requirement removed):
  `rtk python3 -c 'from pathlib import Path; import re; s=re.sub(r"\\s+", " ", Path("worklog/TOOL-SHELL-CANCEL-CONTRACT.md").read_text()); checks={"C1":[r"## Mandatory seam and RED ordering",r"Independent seam verification lane",r"no cancellation RED lane may be claimed, authored, or frozen",r"compile-fail/import-fail RED is forbidden"],"C2":[r"pub enum CleanupStep",r"pub enum CleanupStatus",r"pub enum ProcessError",r"pub type ProcessCancellation = tokio::sync::watch::Receiver<bool>",r"Only Tokio.s existing `sync` feature is used",r"`tokio-util` and `CancellationToken` are not part"],"C3":[r"#### Canonical cwd identity \\(one preparation pass\\)",r"Call `std::fs::canonicalize` exactly once",r"Command::current_dir\\(&canonical_cwd\\)",r"no `/tmp` fallback"],"C4":[r"### Race and terminal semantics",r"child completion, cancellation change, then deadline",r"cancellation wins",r"paused Tokio time"],"C5":[r"pub duration_ms: u64",r"u64::try_from\\(start.elapsed\\(\\).as_millis\\(\\)\\).unwrap_or\\(u64::MAX\\)",r"sub-millisecond duration is `0`",r"ShellResult.duration_ms: u128"]}; missing={k:[v for v in vals if not re.search(v,s)] for k,vals in checks.items()}; missing={k:v for k,v in missing.items() if v}; assert not missing, missing; assert s.count("### Broker and no-spawn invariants")==1; assert "integration lane must choose one stable representation" not in s; assert "Cleanup-error representation, readiness-channel failure policy" not in s; print("C1-C5 semantic contract checks PASS")'`
  -> `C1-C5 semantic contract checks PASS`.
- `rtk git grep -n 'tokio-util\\|CancellationToken\\|CleanupStatus\\|ProcessError\\|duration_ms\\|compile-fail' -- worklog/TOOL-SHELL-CANCEL-CONTRACT.md` -> 33 matching lines; scan includes the explicit prohibition and all required definitions.
- `rtk shasum -a 256 crates/tools/tests/phase1_shell_broker.rs` -> `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8` (frozen hash unchanged).
- `rtk git diff --check` -> exit 0, no output.
- Scope check before claim update -> exactly three intended paths present: `tasks/completion/claims.json`, `worklog/TOOL-SHELL-CANCEL-CONTRACT.md`, and `worklog/TOOL-SHELL-CANCEL-CONTRACT-CORRECTION.md`.
- No Cargo, browser, database, or heavy process run by this research lane.

## Remaining unknowns

- Fresh independent verification of the corrected contract remains required.
- Public seam, cancellation implementation, broker/server wiring, and RED remain
  future lanes. Windows process-tree proof remains unavailable.
- This scratchpad records no acceptance verdict.
