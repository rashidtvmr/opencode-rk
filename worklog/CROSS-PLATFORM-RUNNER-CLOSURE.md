# CROSS-PLATFORM-RUNNER-CLOSURE

## Claim

- Task: `CROSS-PLATFORM-RUNNER-CLOSURE`
- Session: `ses_f2cd8fdacffesguWdejUaeGXhR`
- Branch: `plan/CROSS-PLATFORM-RUNNER-CLOSURE`
- Worktree HEAD: `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b`
- Owned file: `worklog/CROSS-PLATFORM-RUNNER-CLOSURE.md`
- No product, workflow, test, controller, policy, or verifier file changed.
- Research only. No runner, Cargo, network, signing, or notarization run.

## Executive result

Current CI proves generic Rust checks on `ubuntu-latest` and `windows-latest`, not
release closure, native runtime loading, installed APP-012, Linux Landlock, or
Windows GNU runtime execution. Current native artifacts are source-vendored and
hash-verified, but runtime execution is recorded as `not-run` for Linux and
Windows. The only honest candidate boundary is unsigned, platform-specific,
source-revision-bound evidence after the prerequisites below land.

No hosted runner settings, branch protection, secret availability, or signing
identity may be inferred from checked-in YAML or policy. `.github/protection-policy.json`
explicitly records `platformState.verified: false`.

## Source evidence

All line references below are from worktree HEAD
`5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b`, unless a historical commit is named.

### Runner and protection capability

- `.github/workflows/ci.yml:15-27`: `planning` uses `ubuntu-latest`, 10-minute
  timeout, pinned checkout, repository validator and bootstrap tests.
- `.github/workflows/ci.yml:29-60`: `rust` matrix uses only `ubuntu-latest` and
  `windows-latest`, 30-minute timeout, generic `cargo check`, workspace tests,
  clippy and format. No target selection, native release build, package, install,
  artifact upload, Landlock probe, APP-012 journey, or receipt publication.
- `.github/workflows/completion.yml:9-20`: completion specification uses only
  `ubuntu-latest`; it is not product or release evidence.
- `.github/protection-policy.json:3,79-101`: desired external state is active
  `main` protection, but `platformState.verified` is false. Source policy is not
  proof of GitHub rules, protected environments, runner labels, or secrets.
- `.github/CODEOWNERS:4-6`: `.github/` and workflow changes require
  `@rashidtvmr` ownership review when hosting rules enforce CODEOWNERS.
- `docs/REPOSITORY_PROTECTION.md:3-4,24-33,150-167`: local policy and fixture
  validation cannot prove hosting settings; administrator readback is required.

### Native target and artifact capability

- `crates/opentui-bridge/build.rs:17-21,28-63`: native build selects artifacts
  under `native/lib/<Rust target triple>`. Linux accepts `libopentui.so`; Windows
  requires both runtime DLL and import library. Missing artifacts fail closed.
- `crates/opentui-bridge/native/artifacts.json:57-117,143-175`: current source
  manifest records builder/integrity verification, not target runtime execution.
  It records:
  - `x86_64-unknown-linux-gnu` `libopentui.so`, 26,627,832 bytes,
    SHA-256 `9f074adf1e3c67bb027433d44da05b9e285a37ae58cf0b2ed76504304450c79e`.
  - `x86_64-pc-windows-gnu` `opentui.dll`, 6,800,384 bytes,
    SHA-256 `0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15`.
  - `x86_64-pc-windows-gnu` `libopentui.dll.a`, 101,524 bytes,
    SHA-256 `1dce093459b94a4d6bd0530f3d59918b765bc87a982019`.
  - Windows GNU pair verification is `431/431` export/import correspondence.
  - `x86_64-pc-windows-msvc` is explicitly unsupported by the pinned builder.
  - Manifest line 169 says runtime execution passed only for macOS static
    bridge/parity/PTY; other targets were not run.
- `crates/opentui-bridge/native/build_opentui.sh:19-31,101-132`: pinned fork
  commit `c01292fd0837bafd07ce458c74416b2b375a41ab`, Zig `0.16.0`, supports
  Windows GNU but no MSVC. Builder is fail-closed and its only pinned download
  archive is Darwin/arm64, so the current local host cannot stand in for Linux or
  Windows runtime evidence.

### Packaging and installed runtime

- `scripts/install-oc2.sh:15-20,212-322`: POSIX installer enforces four-member
  archive shape, traversal/type checks, 128 MiB per-member and aggregate bounds,
  checksum-before-write, and bounded extraction.
- `scripts/install-oc2.sh:366-425`: POSIX install places `oc2` in `bin` and
  `libopentui.so` or `.dylib` in sibling `lib`, with rollback and cleanup.
- `scripts/install-oc2.ps1:40-67`: Windows installer currently verifies only
  AMD64, archive hash, and `oc2.exe`; it expands/copies only `oc2.exe`. It does
  not validate or install `opentui.dll`, does not enforce member or expanded
  payload bounds, and has no native-closure rollback proof.
- `worklog/TUI-011.md:207-243`: macOS disposable package proved installer,
  static native PTY, and shared daemon survival only. It explicitly does not
  prove Linux or Windows.
- `worklog/APP-012.md:45-87,123-135`: APP-012 remains open; unsigned runners
  are allowed, signing identities are unavailable; Windows GNU is the only
  supported Windows ABI; current generic Windows CI does not build/package/
  install the native closure and no release workflow exists.
- `crates/cli/build.rs:1-47`: `GIT_COMMIT` is a compile-time source receipt,
  selected from valid `OC2_BUILD_REVISION` or repository `HEAD`. It identifies
  source base only, not dirty state or package identity. Release jobs must bind
  this value to archive and installed-binary receipts.
- `crates/cli/src/install_commands.rs:131-164`: public install platform list
  includes `WindowsX64`, but that label alone does not prove GNU ABI or runtime
  closure.

### APP-012 and security evidence

- `tasks/completion/local.json:13-15`: APP-012 requires clean install, real
  terminal, provider fixture, real UI/broker/tools/storage, second client,
  failure injection, terminal/process/resource recordings, and exact artifact /
  revision hashes.
- `worklog/APP-012.md:89-100`: current read-tool slice is only 1/1; restart,
  protected-path denial, human approval/resume, and packaged proof remain open.
- `worklog/APP-012.md:123-129`: Windows PowerShell RED must prove native closure,
  traversal rejection and 128 MiB limits on Windows; local macOS cannot prove it.
- `crates/security/src/os_backend.rs:9-37,112-181`: `enforcement_linked` is
  hardcoded false; `engage()` always returns `Blocked`. Path checks and process
  hygiene are explicitly not an OS sandbox.
- `crates/security/src/platform_matrix.rs:85-105,151-162`: Linux, macOS and
  Windows rows all report `isolated: false`; `enforce()` always fails closed.
- `worklog/DISC-106.md:5-13,26-34`: current host has no `/proc/filesystems`
  Landlock entry; raw syscall backend is absent because the security crate
  forbids unsafe code and has no syscall dependency. Existing safe tests prove
  fail-closed behavior, not enforcement.
- Historical blocker commit `bd8ddc9557e9422e2bc1ff46aba86e8794bdd2cd`
  (`SEC-RED-OS`) records the honest local macOS blocker. It cannot be promoted
  to Linux evidence.
- `docs/SECURITY.md:38-56`: a prompt/regex is not a sandbox; actual backend
  execution on the target platform must close inherited capabilities and verify
  denied side effects.

## Capability matrix

| Claim | Current repository evidence | Current proof boundary | Closure state |
|---|---|---|---|
| Generic Linux Rust CI | `ci.yml:29-50` | Matrix check/test only; no release/native target receipt | Runnable now, insufficient |
| Generic Windows Rust CI | `ci.yml:29-50` | Matrix check/test only; ABI unspecified | Runnable now, insufficient |
| Linux x86_64 native bytes | `artifacts.json:71-82` | Hash/size/builder verification; runtime `not-run` | Source-built, needs runner |
| Linux installer | `install-oc2.sh:212-425` | POSIX security/closure tests are host-profiled; no Linux release E2E | Runnable after package/rpath prerequisite |
| Linux Landlock | `os_backend.rs:112-181` | Detection/fail-closed only; no syscall backend | Blocked: backend + actual Linux runner |
| Windows GNU native bytes | `artifacts.json:85-117` | Pair hash and 431/431 export correspondence | Source-built, needs Windows GNU runtime |
| Windows GNU Rust binary | `build.rs:47-51` | No current target build receipt | Job proposed |
| Windows installer closure | `install-oc2.ps1:54-67` | Copies `oc2.exe` only | Blocked: installer repair + frozen PowerShell RED |
| Windows GNU runtime load | none | No Windows execution evidence | Job proposed after installer repair |
| APP-012 Linux | `local.json:15`, `APP-012.md:45-55` | No packaged Linux journey | Blocked: parent product journey + Linux runner |
| APP-012 Windows GNU | `APP-012.md:123-135` | No PowerShell/ConPTY journey | Blocked: installer, Windows test harness, runner |
| Daemon survival | `TUI-011.md:239-243` | macOS installed client only | Must rerun per target in each package job |
| Signed/notarized release | `APP-012.md:54-55,86-87` | No identities/protected receipt/workflow | External unavailable; unsigned only |

## Executable closure job proposal

The following is a proposal for an integrator-owned workflow change. This lane
does not edit `.github/`. Jobs must be added only after the protected-path review
and canonical repository guard. Every heavy build/test step is serial with
`CARGO_BUILD_JOBS=1`, `RUST_TEST_THREADS=1`; each job has a hard timeout and
disposable HOME/data/temp roots. No secrets are needed for unsigned jobs.

### Job L1: `linux-x64-unsigned-closure`

Runner: ordinary `ubuntu-24.04` or an administrator-verified equivalent for
build/package/install. The Landlock stage must use a runner whose kernel and
backend are independently verified; a generic label is not proof.

Preflight and target identity:

```sh
set -eu
uname -a | tee receipts/linux.uname.txt
cat /etc/os-release | tee receipts/linux.os-release.txt
rustc -Vv | tee receipts/linux.rustc.txt
cargo -V | tee receipts/linux.cargo.txt
rustup target list --installed | tee receipts/linux.targets.txt
test "$(rustc -vV | sed -n 's/^host: //p')" = x86_64-unknown-linux-gnu
test -r /proc/filesystems
grep -w landlock /proc/filesystems | tee receipts/linux.landlock-filesystem.txt || true
```

Native and application build, one target only:

```sh
export CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1
export OC2_BUILD_REVISION="$(git rev-parse HEAD)"
test "$(printf %s "$OC2_BUILD_REVISION" | wc -c)" -eq 40
timeout 300 cargo build --release -p opencode-rk-cli --bin oc2 \
  --features native --target x86_64-unknown-linux-gnu
```

Before packaging, prove the loader closure without `LD_LIBRARY_PATH` masking:

```sh
BIN=target/x86_64-unknown-linux-gnu/release/oc2
test -x "$BIN"
readelf -h "$BIN" > receipts/linux.binary-elf.txt
readelf -d "$BIN" > receipts/linux.binary-dynamic.txt
grep -E 'RUNPATH|RPATH' receipts/linux.binary-dynamic.txt
grep -F '$ORIGIN/../lib' receipts/linux.binary-dynamic.txt
```

If the binary is deliberately static, replace the two `readelf` assertions with
an exact static-link receipt proving no `libopentui.so` dependency. A package
that needs `LD_LIBRARY_PATH` is not an installed closure and must fail.

Package layout and identity:

```text
oc2
native/lib/linux-x64/libopentui.so
```

The package stage must copy the binary and the manifest-declared Linux artifact,
then emit a deterministic archive. The archive, member list, member SHA-256 and
size, source revision, target, toolchain, native artifact SHA, and unsigned
status are one receipt. The package contains no signing material. The native
artifact must match `artifacts.json` hash
`9f074adf1e3c67bb027433d44da05b9e285a37ae58cf0b2ed76504304450c79e` before
archive creation.

Example bounded package staging on GNU tar:

```sh
rm -rf "$RUNNER_TEMP/oc2-linux-stage"
mkdir -p "$RUNNER_TEMP/oc2-linux-stage/native/lib/linux-x64"
cp "$BIN" "$RUNNER_TEMP/oc2-linux-stage/oc2"
cp crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so \
  "$RUNNER_TEMP/oc2-linux-stage/native/lib/linux-x64/libopentui.so"
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime='UTC 1970-01-01' \
  -czf "$RUNNER_TEMP/oc2-linux-x64.tar.gz" -C "$RUNNER_TEMP/oc2-linux-stage" oc2 native
sha256sum "$RUNNER_TEMP/oc2-linux-x64.tar.gz" > "$RUNNER_TEMP/oc2-linux-x64.tar.gz.sha256"
sha256sum "$RUNNER_TEMP/oc2-linux-stage/oc2" > "$RUNNER_TEMP/oc2-linux-x64.oc2.sha256"
```

Install and smoke checks in fresh disposable state:

```sh
export HOME="$RUNNER_TEMP/oc2-home"
export TMPDIR="$RUNNER_TEMP/oc2-tmp"
mkdir -p "$HOME" "$TMPDIR" "$RUNNER_TEMP/oc2-install/bin"
bash scripts/install-oc2.sh --archive "$RUNNER_TEMP/oc2-linux-x64.tar.gz" \
  --checksum "$(cut -d' ' -f1 "$RUNNER_TEMP/oc2-linux-x64.tar.gz.sha256")" \
  --install-dir "$RUNNER_TEMP/oc2-install/bin"
test "$(sha256sum "$RUNNER_TEMP/oc2-install/bin/oc2" | cut -d' ' -f1)" = "$(cat "$RUNNER_TEMP/oc2-linux-x64.oc2.sha256")"
test "$(sha256sum "$RUNNER_TEMP/oc2-install/lib/libopentui.so" | cut -d' ' -f1)" = 9f074adf1e3c67bb027433d44da05b9e285a37ae58cf0b2ed76504304450c79e
timeout 30 "$RUNNER_TEMP/oc2-install/bin/oc2" --version | tee receipts/linux.installed-version.txt
```

Run existing bounded installer suites, then the exact integrated APP-012 Linux
test supplied by the independent E2E lane:

```sh
timeout 120 python3 tests/bootstrap/test_tui011_installer_security.py
timeout 120 python3 tests/bootstrap/test_tui011_installer_native_closure.py
timeout 300 cargo test -p opencode-rk-cli --test app012_linux_installed \
  -- --test-threads=1
```

`app012_linux_installed` is a required future frozen test, not a claim that the
file currently exists. It must use a real PTY, loopback provider fixture, fresh
HOME/data, bounded output, broker authorization, tool result persistence,
restart/resume, second client observation, process-tree capture, and cleanup.

### Job L2: `linux-landlock-enforcement`

Runner: administrator-verified Linux runner with Landlock enabled and a linked
syscall-capable backend. This is not satisfied by `ubuntu-latest` merely running
the generic matrix. No secrets. The job must fail closed if backend availability
or enforcement receipt is absent.

Required commands after the security backend lane lands:

```sh
set -eu
uname -a | tee receipts/landlock.uname.txt
grep -w landlock /proc/filesystems | tee receipts/landlock.filesystems.txt
export CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1
timeout 300 cargo test -p opencode-rk-security --test phase1_landlock \
  -- --test-threads=1
```

`phase1_landlock` must be an independently frozen test. Required assertions:

1. real ruleset creation and restriction on the running Linux kernel;
2. allowed fixture access succeeds;
3. outside read/write is denied, marker remains absent;
4. inherited extra FD and parent-only secret environment are unavailable to the
   child, with child killed/reaped on violation;
5. backend receipt says enforcement engaged, not detection-only;
6. repeated bounded setup/teardown leaves stable FD/process counts.

Upload only bounded test log, uname/filesystem receipt, backend metadata, and
SHA-256 manifest. A pass from `os_backend`'s current fail-closed unit tests is
not a substitute.

### Job W1: `windows-x64-gnu-unsigned-closure`

Runner: `windows-2022` with the GNU Rust target and MinGW linker available.
Use no protected secret. Do not use `windows-latest` as a provenance claim
without recording its resolved image/toolchain versions.

Preflight and target identity in PowerShell:

```powershell
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force "$env:RUNNER_TEMP\oc2-receipts" | Out-Null
rustc -Vv | Tee-Object "$env:RUNNER_TEMP\oc2-receipts\rustc.txt"
cargo -V | Tee-Object "$env:RUNNER_TEMP\oc2-receipts\cargo.txt"
rustup target list --installed | Tee-Object "$env:RUNNER_TEMP\oc2-receipts\targets.txt"
rustup target add x86_64-pc-windows-gnu
if ((rustc -vV | Select-String '^host:').ToString() -notmatch 'x86_64-pc-windows') { throw 'wrong Rust host' }
```

Build exactly the supported ABI:

```powershell
$env:CARGO_BUILD_JOBS = '1'
$env:RUST_TEST_THREADS = '1'
$env:OC2_BUILD_REVISION = (git rev-parse HEAD)
cargo build --release -p opencode-rk-cli --bin oc2 --features native --target x86_64-pc-windows-gnu
$bin = 'target\x86_64-pc-windows-gnu\release\oc2.exe'
if (!(Test-Path -LiteralPath $bin)) { throw 'GNU oc2.exe missing' }
```

Verify the native pair before package creation. The DLL must match
`0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15`; the
import archive must match `1dce093459b94a4d6bd0538c4fdd1681f50534930f3d59918b765bc87a982019`.
The import archive is a build receipt/input, not a runtime DLL substitute.

Package layout required by the Windows loader and repaired installer:

```text
oc2.exe
opentui.dll
```

The runtime DLL must be beside `oc2.exe`. The package receipt records the
executable hash, DLL hash/size, target triple, PE architecture, export count,
source revision, Rust toolchain, archive hash, and `unsigned: true`. No MSVC
labels, `.lib` relabeling, or signing claims are permitted.

After the native pair check, stage exactly those two regular files and create a
ZIP with a pinned archive tool. `Compress-Archive` is acceptable only if its
resulting member list, modes, sizes, and SHA-256 are recorded; the verifier must
recompute the archive hash rather than trust a generated filename:

```powershell
$stage = Join-Path $env:RUNNER_TEMP 'oc2-windows-stage'
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item $bin (Join-Path $stage 'oc2.exe')
Copy-Item 'crates\opentui-bridge\native\lib\x86_64-pc-windows-gnu\opentui.dll' (Join-Path $stage 'opentui.dll')
$archive = Join-Path $env:RUNNER_TEMP 'oc2-windows-x64-gnu.zip'
Remove-Item $archive -Force -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -CompressionLevel Optimal
Get-FileHash $archive -Algorithm SHA256 | ForEach-Object Hash | Set-Content "$archive.sha256"
```

The Windows installer lane must first land a frozen PowerShell RED and repair
`scripts/install-oc2.ps1` to validate bounded ZIP members, traversal/type safety,
128 MiB per-member and aggregate limits, required `oc2.exe` plus `opentui.dll`,
checksum-before-write, atomic staging/rollback, and no writes outside the
selected install directory. Then run:

```powershell
$root = Join-Path $env:RUNNER_TEMP 'oc2-windows-gnu'
$home = Join-Path $root 'home'
$install = Join-Path $root 'installed path with spaces\bin'
New-Item -ItemType Directory -Force $home, $root | Out-Null
$env:USERPROFILE = $home
$env:HOME = $home
pwsh -NoLogo -NoProfile -NonInteractive -File scripts/install-oc2.ps1 `
  -Archive "$root\oc2-windows-x64-gnu.zip" `
  -Checksum (Get-Content "$root\oc2-windows-x64-gnu.zip.sha256") `
  -InstallDir $install
if (!(Test-Path -LiteralPath (Join-Path $install 'oc2.exe'))) { throw 'oc2.exe not installed' }
if (!(Test-Path -LiteralPath (Join-Path $install 'opentui.dll'))) { throw 'opentui.dll not installed' }
& (Join-Path $install 'oc2.exe') --version | Tee-Object "$root\receipts\version.txt"
```

The final line is only a smoke test. Full Windows GNU APP-012 requires a
ConPTY-backed frozen test supplied by the E2E lane:

```powershell
cargo test -p opencode-rk-cli --test app012_windows_gnu_installed -- --test-threads=1
```

It must cover fresh HOME, real console/ConPTY, fixture provider, setup, brokered
tool action, second client, restart/resume, daemon PID/liveness, cleanup and
bounded process/output receipts. No current test or local macOS run proves this.

## Receipt and artifact contract

Each L1/L2/W1 run emits one bounded `receipt.json` plus logs. Required fields:

```json
{
  "schema": 1,
  "source_revision": "40 lowercase hex",
  "target": "x86_64-unknown-linux-gnu or x86_64-pc-windows-gnu",
  "runner_os_image": "resolved image label/version",
  "rustc": "exact rustc -Vv output hash or bounded text",
  "native_artifacts": [{"path": "...", "sha256": "...", "size": 0}],
  "archive_sha256": "...",
  "archive_members": [{"path": "...", "sha256": "...", "size": 0}],
  "test_log_sha256": "...",
  "daemon_survival": "passed",
  "unsigned": true,
  "signing": "not-run-external-identity-unavailable"
}
```

Receipt rules:

- Bind every hash to the exact Git revision used by the build. Do not use
  `OC2_E2E_REVISION=$(git rev-parse HEAD)` as a substitute for package metadata.
- Include native member hashes from `artifacts.json` and archive member hashes.
- Keep logs bounded; redact environment and never upload secrets or full process
  environments. Use fresh disposable HOME/data/TMP roots.
- Upload unsigned archive, checksum, receipt, SBOM/native manifest, and bounded
  logs as separate immutable artifacts. Do not call them signed or release-final.
- Independent verifier downloads the artifact and reruns hash, target, install,
  E2E, daemon-survival and receipt-binding checks on the exact revision.

## External blockers

| ID | Blocker | Evidence | Owner / exit condition |
|---|---|---|---|
| B1 | No real Linux Landlock backend | `os_backend.rs:112-181`, `DISC-106.md:11-13,26-34` | SEC backend lane; linked syscalls plus frozen on-kernel enforcement test |
| B2 | No verified protected Linux enforcement runner | `ci.yml:29-35`, `protection-policy.json:98-101` | Admin; runner/kernel receipt and independent readback |
| B3 | Linux dynamic native loader closure not yet proven | `artifacts.json:169`, `TUI-011.md:198-205`, `build.rs:44-45` | Packaging lane; static link or `$ORIGIN/../lib` plus installed no-env run |
| B4 | Windows PowerShell installer omits DLL and bounds | `install-oc2.ps1:54-67`, `APP-012.md:123-129` | Windows packaging lane; frozen RED then GREEN native closure tests |
| B5 | Windows GNU runtime and ConPTY APP-012 absent | `APP-012.md:130-135`, no Windows-specific installed test | E2E lane; real `windows-2022` GNU/ConPTY run and receipt |
| B6 | APP-012 parent journey incomplete on every non-macOS target | `APP-012.md:45-55,89-100`, `TUI-011.md:245-265` | Product integration lane; bare installed setup/tool/restart/second-client GREEN |
| B7 | Protected workflow/release job not present | `.github/workflows` inventory; `ci.yml` only generic jobs | Integrator + CodeOwner; reviewed workflow, canonical guard, hosted run |
| B8 | Signing/notarization identities unavailable | `APP-012.md:54-55,86-87` | Administrator/release owner; protected secret and platform receipt |

B8 does not block an explicitly unsigned candidate evidence bundle. It blocks
any claim of signed, notarized, production release, or protected signing proof.
MacOS evidence cannot discharge B1, B4, or B5.

## Ownership and dependency DAG

```text
SEC-LANDLOCK (backend + frozen Linux test)
        |
        +--> L2 linux-landlock-enforcement

PKG-LINUX (rpath/static closure + package receipt)
        |
        +--> L1 linux-x64-unsigned-closure

PKG-WINGNU (PowerShell native closure + frozen RED/GREEN)
        |
        +--> W1 windows-x64-gnu-unsigned-closure

APP-012-E2E (Linux PTY + Windows ConPTY tests, fixture, bounded receipts)
        ^                         ^
        |                         |
        +---------- L1 -----------+---------- W1

REL-WORKFLOW (protected .github change, runner labels, artifact upload)
        ^
        +--> L1, L2, W1 (after CodeOwner review and repository guard)

INDEPENDENT-VERIFY (exact integrated revision rerun, hash/receipt audit)
        ^
        +--> L1 + L2 + W1 + APP-012-E2E

SIGN-ADMIN (external protected identity, optional after unsigned closure)
        |
        +--> signed/notarized release only; never required by unsigned jobs
```

Recommended serialized order: B1 and B3/B4 prerequisites, then L1/L2/W1 on
their actual targets, then independent APP-012 reruns, then receipt verification.
Do not run Linux and Windows heavy builds concurrently on a constrained host;
the hosted jobs may be separate but each job remains serial internally.

## Validation performed

- `rtk git grep -n 'runs-on\|windows\|ubuntu\|target\|artifact\|sign\|notar' -- .github/workflows`
  -> only current generic planning/rust/completion jobs; no release workflow.
- `rtk git diff --check` -> run after this worklog edit; expected clean.
- `rtk python3 tools/validate_repository.py` -> run after this worklog edit;
  expected pre-existing repository failure at backlog exhaustion, reported
  verbatim if reproduced.
- No product test applies: research lane has no runner and must not invent RED or
  GREEN. Existing hashes cited above are preserved, not regenerated.

## Unknowns and acceptance boundary

- GitHub runner image resolution, protected labels, branch rules, artifact
  retention, and secret/environment availability are external and unverified.
- Exact Linux backend crate/API and Windows installer test names require separate
  implementation/RED lanes. Proposed commands intentionally remain blocked until
  those lanes land frozen artifacts.
- Current worktree has no `tests/e2e/local_application.rs`; the task card's path
  is a contract, not evidence that the journey exists.
- Status is a closure plan only. No platform claim, release acceptance, signing
  claim, or convergence completion is made.
