# x86_64-pc-windows-gnu Real-Runner Closure Specification

## Scope and authority

This document defines the executable closure for the `x86_64-pc-windows-gnu` target
only. It is a text/static specification produced by lane PHASE1-WGNU-RUNNER-CLOSURE
at base revision 7262682 (also referenced as a039296 in parent task context). No
Cargo, workflow, product source, or test file edits are made by this lane.

**Explicit exclusions:**
- MSVC targets (`x86_64-pc-windows-msvc`) are NOT in scope. The verified GNU DLL
  and import library must never be represented as MSVC artifacts. The pinned upstream
  has no MSVC target; absence of an MSVC artifact is not a release blocker
  (source: worklog/APP-012.md:83-85, controller decision 2026-09-23).
- Code signing and notarization are NOT claimed. Only unsigned CI runners are
  available; no signing identities or protected-host receipts may be fabricated
  (source: worklog/APP-012.md:86-87, SECURITY.md section 5).

---

## 1. DLL and Import Library Loading Contract

### Verified artifacts (TUI-011 evidence)

| Artifact | Path (repo-relative) | Size (bytes) | SHA-256 | Type |
|---|---|---|---|---|
| Runtime DLL | `crates/opentui-bridge/native/lib/x86_64-pc-windows-gnu/opentui.dll` | 6800384 | `0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15` | PE32+ GUI x86-64 DLL |
| Import library | `crates/opentui-bridge/native/lib/x86_64-pc-windows-gnu/libopentui.dll.a` | 101524 | `1dce093459b94a4d6bd0538c4fdd1681f50534930f3d59918b765bc87a982019` | ar archive (GNU import lib) |

Source: `crates/opentui-bridge/native/artifacts.json:85-111`,
`crates/opentui-bridge/native/sbom.json:123-155`, worklog/TUI-011.md:56,100-120.

### Pair verification

- Pair ID: `x86_64-pc-windows-gnu-opentui`
- Reciprocal `paired_path` references confirmed in artifacts.json
- Builder verification command:
  ```
  bash crates/opentui-bridge/native/build_opentui.sh \
    --verify-only crates/opentui-bridge/native/lib/x86_64-pc-windows-gnu/opentui.dll \
    --import-lib crates/opentui-bridge/native/lib/x86_64-pc-windows-gnu/libopentui.dll.a \
    --target x86_64-pc-windows-gnu
  ```
- Exit code: 0
- Bounded PE parser reported: 431 exports, 431/431 export/import correspondence
- Source: worklog/TUI-011.md:119-120

### Loading contract

The Rust bridge crate (`opentui-bridge`) loads `opentui.dll` at runtime via
`LoadLibraryW` on the Windows GNU target. The import library `libopentui.dll.a`
is used only at link time for the `x86_64-pc-windows-gnu` toolchain to resolve
symbols. Both files MUST be co-located in the installation directory beside
`oc2.exe`. The loader must:

1. Resolve `opentui.dll` from the application directory first (not system PATH)
   to prevent DLL hijacking. Use `SetDllDirectoryW` with the executable's
   directory before calling `LoadLibraryW`.
2. Verify the loaded DLL's SHA-256 matches
   `0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15` before
   resolving function pointers. Mismatch is a fatal error (exit code 65,
   EX_DATAERR), not a fallback.
3. Close inherited capabilities after loading per SECURITY.md section 3.

### Failure statuses

| Condition | Exit code | Behavior |
|---|---|---|
| DLL not found in app directory | 69 (EX_UNAVAILABLE) | Terminate before daemon start |
| DLL hash mismatch | 65 (EX_DATAERR) | Terminate, log mismatch details |
| Import symbol resolution failure | 70 (EX_SOFTWARE) | Terminate |
| LoadLibraryW returns NULL | 71 (EX_OSERR) | Terminate with GetLastError |

---

## 2. ConPTY Lifecycle Management

### Contract

Windows GNU runner uses the ConPTY API (available since Windows 10 1809) for
pseudo-terminal allocation. The lifecycle must be bounded and deterministic:

1. **Creation**: `CreatePseudoConsole` with explicit size (default 80x24).
   Store the returned `HPCON` handle and the two pipe pairs (input/output).
2. **Process attachment**: Pass the ConPTY handle via `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`
   in `STARTUPINFOEXW.lpAttributeList` when spawning child processes (shell,
   PowerShell installer scripts).
3. **Bounded output**: Read from the ConPTY output pipe into a bounded buffer.
   Maximum retained output: 1 MiB per session. Excess bytes are discarded with
   a logged truncation marker. This satisfies the non-negotiable rule against
   unbounded retained output (AGENTS.md).
4. **Resize**: `ResizePseudoConsole` on terminal window change. Rate-limit to
   one resize per 100ms to prevent rapid-resize DoS.
5. **Teardown sequence** (in order):
   a. Send `CTRL_CLOSE_EVENT` to attached process group via `GenerateConsoleCtrlEvent`.
   b. Wait up to 5000ms for process exit.
   c. If timeout, call `TerminateProcess` on the process handle.
   d. Call `ClosePseudoConsole(hPCON)`.
   e. Close all four pipe handles.
   f. Verify handle count delta is zero (no leaked handles).

### Failure statuses

| Condition | Exit code | Behavior |
|---|---|---|
| CreatePseudoConsole fails | 71 (EX_OSERR) | Fall back to anonymous pipes, log degraded mode |
| Resize rate limit exceeded | N/A (logged) | Drop intermediate resize events |
| Teardown timeout (5s) | N/A (forced) | TerminateProcess + close handles |
| Handle leak detected post-close | 70 (EX_SOFTWARE) | Log leaked handle types, abort session |

---

## 3. Bounded Process Tree Cancellation

### Contract

Every spawned process belongs to a Windows Job Object that enforces resource bounds
and enables tree-wide cancellation. No detached task without an owner (AGENTS.md).

1. **Job Object creation**: `CreateJobObjectW(NULL, NULL)` per turn/session.
2. **Limits applied** via `SetInformationJobObject`:
   - `JobObjectExtendedLimitInformation`:
     - `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: ensures all children die if the
       parent crashes or the job handle is closed unexpectedly.
     - `JOB_OBJECT_LIMIT_PROCESS_MEMORY`: 512 MiB per process (configurable,
       must be explicit).
     - `JOB_OBJECT_LIMIT_ACTIVE_PROCESS`: 32 concurrent processes max.
   - `JobObjectCpuRateControlInformation`: optional CPU rate cap.
3. **Process assignment**: `AssignProcessToJobObject` immediately after
   `CreateProcessW` succeeds. If assignment fails, terminate the orphaned
   process and return error.
4. **Cancellation**: On user interrupt (Ctrl+C), broker denial, or timeout:
   a. `TerminateJobObject(hJob, exitCode)` kills entire tree atomically.
   b. Wait for all process handles in the job to signal exited.
   c. Close the job handle.
5. **Byte budget enforcement**: Total stdout/stderr captured across all
   processes in a job must not exceed the configured byte budget (default
   16 MiB per turn). Use overlapped I/O with a counting wrapper.

### Failure statuses

| Condition | Exit code | Behavior |
|---|---|---|
| AssignProcessToJobObject fails | 71 (EX_OSERR) | Kill orphan, return error |
| Process memory limit exceeded | 137 (SIGKILL equiv) | Job terminates process |
| Active process limit exceeded | 71 (EX_OSERR) | New spawn denied |
| Byte budget exhausted | 73 (EX_CANTCREAT) | Truncate output, signal caller |
| TerminateJobObject fails | 70 (EX_SOFTWARE) | Enumerate and kill individually |

---

## 4. Transactional PowerShell Installer

### Current gap

`scripts/install-oc2.ps1:54-66` expands an archive and copies only `oc2.exe`.
It does NOT validate or install the Windows native `opentui.dll` closure. Unlike
the POSIX installer, it has no member-count, member-size, or aggregate
expanded-payload bounds. (Source: worklog/APP-012.md:123-129)

### Required installer behavior

The repaired `install-oc2.ps1` must implement transactional installation with
payload validation:

1. **Pre-flight checks**:
   - Verify PowerShell 5.1+ or pwsh 7+ is running.
   - Confirm target directory exists and is writable.
   - Check sufficient disk space (minimum 128 MiB free).

2. **Archive extraction with bounds**:
   - Maximum member count: 64 files.
   - Maximum single member size: 64 MiB.
   - Maximum aggregate expanded payload: 128 MiB.
   - Reject any archive member whose path contains `..` traversal components.
   - Reject symbolic links in archives (Windows GNU does not require them).

3. **Required closure members** (all must be present):
   - `oc2.exe` (main binary)
   - `opentui.dll` (SHA-256 must match `0be2efd8...dadd6b15`)
   - `libopentui.dll.a` (SHA-256 must match `1dce0934...982019`)

4. **Transactional semantics**:
   - Extract to a temporary staging directory first.
   - Validate all hashes and bounds in staging.
   - If validation passes, move files atomically to target directory using
     `Move-Item` (which calls `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`).
   - If any validation fails, remove the staging directory entirely and report
     the specific failure. Leave the previous installation untouched.
   - Write a receipt file (see Section 7) to the target directory on success.

5. **Failure statuses**:

| Condition | Exit code | Behavior |
|---|---|---|
| Member count exceeds 64 | 65 (EX_DATAERR) | Abort, clean staging |
| Single member exceeds 64 MiB | 65 (EX_DATAERR) | Abort, clean staging |
| Aggregate exceeds 128 MiB | 65 (EX_DATAERR) | Abort, clean staging |
| Path traversal detected | 73 (EX_CANTCREAT) | Abort, clean staging |
| DLL hash mismatch | 65 (EX_DATAERR) | Abort, clean staging |
| Missing required member | 69 (EX_UNAVAILABLE) | Abort, clean staging |
| Disk space insufficient | 69 (EX_UNAVAILABLE) | Abort, no changes |
| Staging cleanup fails | 70 (EX_SOFTWARE) | Log, leave staging for manual removal |

### Exact commands

```powershell
# Install from local archive
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/install-oc2.ps1 `
  -ArchivePath .\release-x86_64-pc-windows-gnu.zip `
  -InstallDir "$env:LOCALAPPDATA\opencode-rk" `
  -ExpectedDllHash "0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15"

# Verify existing installation
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/install-oc2.ps1 `
  -VerifyOnly `
  -InstallDir "$env:LOCALAPPDATA\opencode-rk"
```

---

## 5. Authenticated Daemon/Web Transport

### Contract

The daemon listens on loopback only (127.0.0.1) using authenticated versioned
transport (PLAN.md ADR-002). On Windows GNU:

1. **Named pipe fallback**: If TCP loopback binding fails (port conflict),
   fall back to Windows named pipes (`\\.\pipe\opencode-rk-<session-id>`).
   Named pipes use `PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE` to
   ensure singleton behavior.

2. **Authentication**: Every client connection must present a bearer token
   generated at daemon startup. Token is stored in the data directory with
   restrictive ACLs (only the owning OS user can read). The web UI transport
   uses the same token via WebSocket upgrade header.

3. **Versioned protocol**: All messages carry a protocol version field.
   Mismatched versions receive a structured error response, not a hang or crash.

4. **Resource bounds**:
   - Maximum concurrent client connections: 8.
   - Maximum message size: 4 MiB.
   - Idle connection timeout: 300 seconds.
   - Per-connection send queue bound: 256 KiB (byte budget, not just count).

### Failure statuses

| Condition | HTTP status / Exit code | Behavior |
|---|---|---|
| Missing auth token | 401 Unauthorized | Close connection |
| Invalid auth token | 403 Forbidden | Close connection |
| Protocol version mismatch | 400 Bad Request | Return version error JSON |
| Message exceeds 4 MiB | 413 Payload Too Large | Close connection |
| Connection limit reached | 503 Service Unavailable | Queue with 10s timeout |
| Named pipe already exists | 71 (EX_OSERR) | Daemon singleton violation |

---

## 6. APP-012 Installed Journey (Windows GNU)

### Parent contract

APP-012 requires: install -> bare `oc2` -> setup -> coding turn -> real broker
approval/tool edit -> concurrent client -> daemon restart/resume, using only a
deterministic provider fixture and disposable data. (Source: worklog/APP-012.md:9-11)

### Windows GNU specific journey steps

1. **Install**: Run the transactional PowerShell installer (Section 4) with a
   test archive containing `oc2.exe`, `opentui.dll`, `libopentui.dll.a`.
   Target: disposable temp directory, not the user's real LOCALAPPDATA.

2. **DLL verification**: After install, confirm `opentui.dll` SHA-256 matches
   `0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15`.

3. **Bare launch**: Execute `oc2.exe` with no subcommands under ConPTY (Section 2).
   Must reach the default TUI view. Uses `DefaultLaunch` routing
   (source: crates/cli/src/main.rs::run, worklog/APP-012.md:19-20).

4. **Setup flow**: First-run account setup through the real onboarding path
   (source: crates/providers/src/account_setup.rs, APP-005 ledger-completed).
   Uses mock credential builder per controller decision (APP-012.md:73-77).

5. **Coding turn**: Submit prompt through the loopback provider fixture.
   Receive streamed response. Verify transcript persistence in SQLite
   (source: crates/server/src/lib.rs:1192-1223 for registry tools,
   crates/tools/src/executor.rs:105-119 for actual execution).

6. **Broker authorization**: Tool execution goes through the real permission
   broker (source: crates/server/src/lib.rs:1388-1438,1585-1643).
   `RequireHuman` decisions must route through an approval channel, not
   convert directly to terminal errors (known gap: APP-012.md:113-115).

7. **Concurrent client**: Second client attaches to the same daemon via
   authenticated transport (Section 5). Both observe consistent state.

8. **Daemon restart/resume**: Stop daemon gracefully (SIGTERM equivalent via
   `GenerateConsoleCtrlEvent`), verify SQLite WAL checkpoint completes,
   restart, confirm session resume from persisted state.

### Test constraints

- Disposable HOME and data directory only. No user database mutation.
- Loopback-only fixtures. No network access.
- Bounded captures and timeouts (30s per step, 120s total journey).
- Deterministic child/daemon cleanup via Job Objects (Section 3).
- Provider fixture is the only substitute; no mocked renderer, daemon, broker,
  tool executor, or persistence success is acceptable (TDD.md section 7).

---

## 7. Resource and Receipt Schemas

### Installation receipt schema

Written by the transactional installer on success. File: `install-receipt.json`
in the installation directory.

```json
{
  "schema_version": 1,
  "installed_at": "2026-09-24T00:00:00Z",
  "target": "x86_64-pc-windows-gnu",
  "artifacts": {
    "oc2.exe": {
      "size_bytes": 0,
      "sha256": ""
    },
    "opentui.dll": {
      "size_bytes": 6800384,
      "sha256": "0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15"
    },
    "libopentui.dll.a": {
      "size_bytes": 101524,
      "sha256": "1dce093459b94a4d6bd0538c4fdd1681f50534930f3d59918b765bc87a982019"
    }
  },
  "installer_version": "",
  "verification_passed": true
}
```

### Runtime resource receipt schema

Written by the daemon on graceful shutdown. File: `resource-receipt.json` in
the data directory.

```json
{
  "schema_version": 1,
  "session_id": "",
  "started_at": "",
  "stopped_at": "",
  "peak_memory_bytes": 0,
  "total_turns": 0,
  "total_tool_calls": 0,
  "total_bytes_captured": 0,
  "conpty_resizes": 0,
  "processes_spawned": 0,
  "job_object_terminations": 0,
  "handle_leak_detected": false,
  "truncation_events": 0
}
```

Both schemas enforce bounded integer fields. No unbounded arrays or strings.
Maximum string length for identifiers: 256 characters.

---

## 8. Build Commands

### Cross-compilation (from Linux/macOS host)

```bash
# Install target
rustup target add x86_64-pc-windows-gnu

# Build with serial limits per AGENTS.md 8GB budget
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 \
  cargo build --release -p opencode-rk-cli --bin oc2 --features native \
  --target x86_64-pc-windows-gnu

# Verify DLL hash
sha256sum target/x86_64-pc-windows-gnu/release/opentui.dll
# Expected: 0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15

# Verify import library hash
sha256sum target/x86_64-pc-windows-gnu/release/libopentui.dll.a
# Expected: 1dce093459b94a4d6bd0538c4fdd1681f50534930f3d59918b765bc87a982019
```

### Package command

```bash
# Create release archive with exactly 3 members
zip -j release-x86_64-pc-windows-gnu.zip \
  target/x86_64-pc-windows-gnu/release/oc2.exe \
  crates/opentui-bridge/native/lib/x86_64-pc-windows-gnu/opentui.dll \
  crates/opentui-bridge/native/lib/x86_64-pc-windows-gnu/libopentui.dll.a
```

---

## 9. Explicit Exclusions

### MSVC exclusion

This closure specification covers `x86_64-pc-windows-gnu` only. The following
are explicitly out of scope and must not be claimed as supported:

- `x86_64-pc-windows-msvc` target triple
- MSVC-linked `opentui.dll` or `.lib` import libraries
- Visual Studio Build Tools as a build dependency
- Any MSVC-specific CRT linkage (`/MD`, `/MT`, `msvcrt.lib`)

Rationale: The pinned upstream OpenCode commit `95daf90670b7c039c436c85537da5fbfe2205b41`
has no MSVC target. The verified artifacts were built with Zig targeting
`x86_64-windows-gnu`. Representing these as MSVC-compatible would be false.
(Source: worklog/APP-012.md:83-85, worklog/TUI-011.md:56)

### Signing exclusion

Code signing and notarization are not performed by this closure:

- No Authenticode signatures on `oc2.exe` or `opentui.dll`
- No Microsoft EV certificate usage
- No Windows Hardware Dev Center attestation
- SmartScreen warnings will appear on first download until reputation is established

Rationale: Only unsigned CI runners are authorized. Signing identities are not
available in this environment and must remain an external release blocker.
Fabricating signing receipts violates SECURITY.md section 5 and AGENTS.md
non-negotiable rules. (Source: worklog/APP-012.md:86-87)

---

## 10. Verification checklist

For the implementation lane consuming this specification:

- [ ] `opentui.dll` loads from application directory only (not PATH)
- [ ] DLL SHA-256 verified before symbol resolution
- [ ] ConPTY created, attached, bounded, and torn down without handle leaks
- [ ] Job Object enforces KILL_ON_JOB_CLOSE, memory, and process limits
- [ ] PowerShell installer rejects traversal, oversized payloads, hash mismatches
- [ ] Installer writes `install-receipt.json` with correct schema
- [ ] Daemon authenticates all clients via bearer token
- [ ] Named pipe fallback works when TCP loopback unavailable
- [ ] APP-012 journey completes end-to-end on Windows GNU with disposable fixtures
- [ ] No MSVC artifacts or claims present
- [ ] No signing operations attempted or claimed
- [ ] Resource receipt written on graceful shutdown
- [ ] All exit codes match the tables in Sections 1-5
