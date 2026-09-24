# Cross-platform runner closure verification

## Scope and revision

- Task: `CROSS-PLATFORM-RUNNER-CLOSURE-VERIFY`
- Verification target: commit `39a08732930cf89d3d225a0a6c07db6328620057`
  (`research: map cross-platform runner closure evidence`)
- Method: source and workflow inspection only. No workflow/source/test
  implementation, runner execution, signing, notarization, or artifact
  generation.
- Release boundary: unsigned evidence candidate only. No release acceptance.

The preceding closure plan is a proposal, not runtime evidence. This report
checks its claims against the target tree and records what remains absent.

## Executive verdict

**BLOCKED.** The target tree contains generic Ubuntu and Windows CI only. It
does not contain target-specific release jobs, package/install execution,
Linux Landlock enforcement, Linux installed APP-012, or Windows GNU runtime
and ConPTY APP-012 evidence. Vendored native bytes have integrity and ABI
receipts, but `runtime_execution` explicitly says only macOS ran. The only
honest near-term result is an unsigned, target-specific evidence bundle after
the prerequisite implementation and protected workflow changes land.

Signing/notarization remains an external blocker. It is not needed for an
explicitly unsigned candidate, but cannot be implied by CI or source policy.

## Capability matrix

| Claim | Exact evidence at target revision | Verification result |
|---|---|---|
| Generic Linux CI | `.github/workflows/ci.yml:29-50` uses `ubuntu-latest` in a generic OS matrix and runs workspace check/test | **PASS, insufficient**: no release/native/package/install/runtime proof |
| Generic Windows CI | `.github/workflows/ci.yml:29-50` uses `windows-latest` in the same generic matrix | **PASS, insufficient**: ABI and native runtime are unspecified |
| Linux x86_64 native bytes | `crates/opentui-bridge/native/artifacts.json:71-82` records `x86_64-unknown-linux-gnu`, 26,627,832 bytes, SHA-256 `9f074adf1e3c67bb027433d44da05b9e285a37ae58cf0b2ed76504304450c79e` | **PASS, source integrity only**: `:169` says non-macOS runtime targets were not run |
| Windows GNU native pair | `crates/opentui-bridge/native/artifacts.json:85-117` records the DLL, import archive, hashes, sizes, and pair metadata; `:151-155` records `431/431` correspondence | **PASS, source integrity only**: no Windows process load proof |
| Linux target link gate | `crates/opentui-bridge/build.rs:17-21,28-46` selects `native/lib/<triple>` and accepts Linux shared/static artifacts | **PASS, compile-time gate only**: no target build receipt or installed loader run |
| Windows GNU target link gate | `crates/opentui-bridge/build.rs:31-32,47-51,57-63` requires both `libopentui.dll.a` and `opentui.dll` | **PASS, compile-time gate only**: no GNU-target build or package receipt |
| Linux installer security | `scripts/install-oc2.sh:212-325` checks member shape/types, traversal, 128 MiB member and aggregate bounds; `:366-425` stages binary/native files with rollback | **PASS, source and host-profiled tests only**: no Linux target release closure |
| Windows installer native closure | `scripts/install-oc2.ps1:54-67` extracts and copies only `oc2.exe` | **FAIL**: `opentui.dll`, bounds, native rollback, and closure proof are absent |
| Linux Landlock enforcement | `crates/security/src/os_backend.rs:112-181` hardcodes `enforcement_linked = false`; `engage()` returns `Blocked`; `crates/security/src/platform_matrix.rs:85-105,151-162` reports every OS as `isolated: false` and always fails closed | **BLOCKED**: detection/fail-closed policy is not enforcement |
| Inherited-capability closure | `crates/security/src/os_backend.rs:275-289` clears environment and nulls stdio, while `:281-282` states this is not an OS sandbox | **BLOCKED**: no linked kernel backend or actual Linux enforcement test |
| Installed Linux APP-012 | `tasks/completion/local.json:15` defines the journey; `worklog/APP-012.md:89-100` says restart/resume, denial, approval, and packaged proof remain open | **BLOCKED**: no target-specific installed test exists |
| Installed Windows GNU APP-012 | `worklog/APP-012.md:123-135` requires a Windows frozen PowerShell/ConPTY path and says current CI does not provide it | **BLOCKED**: no Windows GNU runtime or ConPTY test exists |
| Protected release workflow | `.github/workflows/ci.yml:15-60` and `.github/workflows/completion.yml:9-20` are the only workflows; neither is a release closure workflow | **BLOCKED**: required jobs and upload receipts are absent |
| Hosting protection and secrets | `.github/protection-policy.json:79-101` declares desired rules but `platformState.verified` is `false`; `docs/REPOSITORY_PROTECTION.md:150-167` states source cannot attest platform state | **BLOCKED**: no hosted readback, protected environment, runner, or secret proof |
| Signing/notarization | `worklog/APP-012.md:54-55,86-87` records unavailable identities | **BLOCKED, external**: unsigned evidence only |

## Executable job matrix

These are bounded job contracts, not existing jobs. They require separate
implementation, test-freeze, workflow review, and hosted execution.

| Proposed job | Target and observable contract | Required bounded checks | Current verdict |
|---|---|---|---|
| `linux-x64-unsigned-closure` | `x86_64-unknown-linux-gnu`; build native `oc2`, package `oc2` plus `native/lib/linux-x64/libopentui.so`, install in disposable HOME, run installed binary without `LD_LIBRARY_PATH`, then run Linux APP-012 | Record `uname`, OS, `rustc -Vv`, target; set `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`; `timeout 300 cargo build --release -p opencode-rk-cli --bin oc2 --features native --target x86_64-unknown-linux-gnu`; use `readelf` to prove loader closure; run installer security/native-closure suites and frozen `app012_linux_installed` | **BLOCKED**: no job, no target-specific installed test, no recorded Linux loader execution |
| `linux-landlock-enforcement` | Actual Linux kernel with linked Landlock syscall backend; allowed fixture access; outside read/write denial with absent marker; inherited secret/FD closure; child kill/reap; bounded repeated teardown | Verify `/proc/filesystems`; `timeout 300 cargo test -p opencode-rk-security --test phase1_landlock -- --test-threads=1`; receipt must say enforcement engaged, not detection-only | **BLOCKED**: `os_backend` has no linked backend and no `phase1_landlock` test |
| `windows-x64-gnu-unsigned-closure` | `x86_64-pc-windows-gnu`; build GNU native `oc2.exe`, package `oc2.exe` beside `opentui.dll`, install using repaired PowerShell installer, run version and ConPTY APP-012 | Verify GNU host/toolchain and target; set serial Cargo/test limits; `cargo build --release -p opencode-rk-cli --bin oc2 --features native --target x86_64-pc-windows-gnu`; verify DLL hash `0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15` and import archive hash `1dce093459b94a4d6bd0538c4fdd1681f50534930f3d59918b765bc87a982019`; run frozen `app012_windows_gnu_installed` | **BLOCKED**: no GNU target job, installer repair, frozen Windows installer test, or ConPTY test |

Required package layouts are deliberate. Linux must expose the native shared
object through the installed layout or use a proven static link; setting
`LD_LIBRARY_PATH` only for a smoke test is not installed closure evidence.
Windows must place `opentui.dll` beside `oc2.exe`; the GNU import archive is a
build input, not a runtime substitute. The Rust target paths and installer
profile paths differ and must be checked by the job, not assumed equivalent.

## Artifact and receipt matrix

| Item | Existing evidence | Missing closure evidence | Verdict |
|---|---|---|---|
| Linux native artifact | `artifacts.json:71-82` hash/size/type and independent verification | Target build, package member hash, installed path, loader output, no-environment run | **BLOCKED** |
| Windows GNU DLL | `artifacts.json:85-99` hash/size/type and pair verification | GNU executable load beside DLL, package/install member receipt, runtime output | **BLOCKED** |
| Windows GNU import archive | `artifacts.json:102-117` hash/size/type and reciprocal pair path | Target build consumed the archive; no runtime claim may use it as DLL evidence | **PASS, limited** |
| Runtime execution | `artifacts.json:163-169` records `7/7` integrity and says only macOS static bridge/parity/PTY passed | Linux and Windows target runtime receipts | **BLOCKED** |
| Source revision binding | `crates/cli/build.rs:1-4,23-47,55-60` validates a 40-character lowercase revision and emits `GIT_COMMIT` | Job must bind revision to package, archive, installed binary, and receipt; dirty-tree state still needs job control | **PARTIAL** |
| Unsigned receipt | No release receipt workflow exists | Bounded JSON with source revision, target, runner image, toolchain, native/archive hashes, test-log hash, daemon survival, `unsigned: true` | **BLOCKED** |
| Signing/notarization receipt | No identity or protected signing job is present; `APP-012.md:86-87` says unavailable | External identity, protected environment, platform receipt | **BLOCKED, external** |

Minimum receipt fields for each future target job:

```json
{
  "schema": 1,
  "source_revision": "40 lowercase hex",
  "target": "x86_64-unknown-linux-gnu or x86_64-pc-windows-gnu",
  "runner_os_image": "resolved image label/version",
  "rustc": "bounded exact rustc -Vv receipt",
  "native_artifacts": [{"path": "...", "sha256": "...", "size": 0}],
  "archive_sha256": "...",
  "archive_members": [{"path": "...", "sha256": "...", "size": 0}],
  "test_log_sha256": "...",
  "daemon_survival": "passed",
  "unsigned": true,
  "signing": "not-run-external-identity-unavailable"
}
```

Logs must be bounded and redact environments. Fresh HOME, data, temporary
directories, and disposable provider fixtures are mandatory. No secret is
needed for unsigned jobs; protected secrets must never be introduced merely to
run these closure jobs.

## Protection, ownership, and dependency matrix

| Work item | Owner boundary | Depends on | Exit evidence |
|---|---|---|---|
| `SEC-LANDLOCK` | Security backend implementer plus independent test author | Approved syscall dependency, real Linux host | Linked backend, frozen RED/GREEN enforcement test, inherited-capability receipt |
| `PKG-LINUX` | Packaging/integration lane | Loader decision, target build | Installed no-environment run and archive/member receipt |
| `PKG-WINGNU` | Windows packaging lane | PowerShell frozen RED, installer repair, GNU target | DLL-inclusive bounded install and rollback/negative receipts |
| `APP-012-E2E` | Independent E2E lane | Real caller wiring, fixture, persistence, approval/resume, platform harness | Linux PTY and Windows ConPTY frozen tests, exact hashes, process cleanup |
| `REL-WORKFLOW` | Integration owner plus `@rashidtvmr` CODEOWNER review | All job/test contracts | Reviewed workflow, canonical guard, hosted run, bounded uploads |
| `INDEPENDENT-VERIFY` | Independent verifier | L1, L2, W1 exact integrated revision | Downloaded artifact hash/install/runtime/E2E rerun |
| `SIGN-ADMIN` | External release administrator | Unsigned closure and protected identity | Signing/notarization receipt; never fabricated by a worker |

Dependency DAG:

```text
SEC-LANDLOCK  -> linux-landlock-enforcement
PKG-LINUX     -> linux-x64-unsigned-closure
PKG-WINGNU    -> windows-x64-gnu-unsigned-closure
APP-012-E2E   -> linux-x64-unsigned-closure + windows-x64-gnu-unsigned-closure
REL-WORKFLOW  -> all hosted jobs
all target jobs -> INDEPENDENT-VERIFY
unsigned closure -> SIGN-ADMIN -> signed/notarized release only
```

`.github/` changes are protected by `.github/CODEOWNERS` and the source policy;
`docs/REPOSITORY_PROTECTION.md:62-141` requires administrator activation and
readback. `.github/protection-policy.json:98-100` explicitly prevents treating
the source declaration as hosted proof. No runner label, image resolution,
artifact retention, environment protection, branch rule, secret, or reviewer
availability may be inferred from YAML.

## Blockers and exact exit conditions

| ID | Blocker | Exact exit condition |
|---|---|---|
| B1 | No linked Linux Landlock backend | Syscall-capable backend linked without weakening fail-closed policy; independent on-kernel enforcement test passes |
| B2 | No verified Linux enforcement runner | Hosted runner receipt proves kernel/filesystem support and backend test runs there |
| B3 | Linux loader closure absent | Static closure or target-appropriate relative loader path; installed no-environment execution passes |
| B4 | Windows installer omits DLL and bounds | Frozen PowerShell RED then GREEN with DLL closure, traversal/type rejection, 128 MiB member/aggregate limits, rollback |
| B5 | Windows GNU runtime and ConPTY absent | Actual `x86_64-pc-windows-gnu` build/install/load plus frozen ConPTY test |
| B6 | APP-012 parent open | Bare installed journey proves setup, real brokered tool, durable result, restart/resume, second client, denial/interruption, cleanup |
| B7 | Target jobs/workflow absent | Reviewed protected workflow runs all bounded jobs and uploads immutable unsigned receipts |
| B8 | Signing/notarization unavailable | External administrator supplies protected identity and platform receipt; until then claim remains unsigned only |

The parent application boundary remains open because
`docs/CONVERGENCE.md:8-28` requires the complete installed journey and forbids
parent completion while work is admitted as unwired, unproven, partial, or
missing. The existing macOS package evidence in `worklog/TUI-011.md:245-265`
explicitly says the bare packaged journey, tool/broker flow, restart/resume,
and second-client observation are not proven. It cannot discharge Linux or
Windows blockers.

## Verification performed

Commands required by the task were run against the verification worktree:

- Workflow inventory: `rtk git grep -n 'runs-on\\|windows\\|ubuntu\\|target\\|artifact\\|sign\\|notar' -- .github/workflows` equivalent source inspection. Result: only generic `planning`, `rust`, and `completion-specification` jobs; no release, target, package, install, Landlock, APP-012, or upload job.
- Source inventory: target/artifact/install/Landlock/APP-012 searches. Result: source hashes and fail-closed tests exist; required target-specific frozen tests and runtime receipts do not.
- `rtk git diff --check`: clean after this worklog is staged in the candidate.
- `rtk python3 tools/validate_repository.py`: **FAIL**, pre-existing repository-wide backlog exhaustion with 51 errors; no validator or policy file changed.
- `rtk python3 tools/convergence_gate.py`: **BLOCKED**, repository-wide ledger findings, including off-plan completed tasks and admitted incomplete work; no convergence state changed.

No Cargo command, network request, hosted runner, Windows execution, Linux
Landlock execution, signing operation, or artifact generation was performed.

## Final status

**REVISE/BLOCKED before release closure.** The target commit maps the missing
claims to sensible future jobs, but mapping is not execution. Integrators must
land the backend, installer, package/loader proof, frozen platform tests, and
protected workflow first. An independent verifier must then rerun every receipt
on the exact integrated revision. Until those conditions hold, report only
source integrity and generic CI capability, never cross-platform runtime,
installed APP-012, Landlock enforcement, signed release, or release acceptance.
