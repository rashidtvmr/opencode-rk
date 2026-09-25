# PHASE1-WGNU-RUNNER-CLOSURE scratchpad

## Claim

- Task: PHASE1-WGNU-RUNNER-CLOSURE
- Session: ses_f28c48536ffexdp7w0R7aq47jJ
- Branch: plan/PHASE1-WGNU-RUNNER-CLOSURE
- Base revision: 7262682e31c1f473912c9483804c9430eb4ee4c5 (also referenced as a039296 in parent task)
- Owned file: worklog/PHASE1-WGNU-RUNNER-CLOSURE.md (scratchpad) + docs/WGNU-RUNNER-CLOSURE.md (deliverable)
- Lane type: text/static specification only. No Cargo, workflow, product, or test edits.

## Source evidence

### Verified GNU artifacts (TUI-011, commit a039296 / 7262682)

- `crates/opentui-bridge/native/artifacts.json` lines 85-111: x86_64-pc-windows-gnu target entries.
  - DLL: `lib/x86_64-pc-windows-gnu/opentui.dll`, role `runtime-dll`
  - Import lib: `lib/x86_64-pc-windows-gnu/libopentui.dll.a`, role `import-library`
  - Pair ID: `x86_64-pc-windows-gnu-opentui`, reciprocal `paired_path` references
- `crates/opentui-bridge/native/sbom.json` lines 123-155:
  - DLL SHA-256: `0be2efd8a77330ba23b0f5eeb094a548f5fee4b7ad7b4c6f83e0e786dadd6b15`, size 6800384 bytes, PE32+ GUI x86-64
  - Import lib SHA-256: `1dce093459b94a4d6bd0538c4fdd1681f50534930f3d59918b765bc87a982019`, size 101524 bytes, ar archive, 431/431 export/import correspondence
- `crates/opentui-bridge/native/build_opentui.sh` lines 144-184: target-specific format mappings for x86_64-pc-windows-gnu (pe format, x86_64-windows-gnu triple, opentui.dll output name)
- TUI-011 worklog (integration tree): full builder verification exit 0, bounded PE parser confirmed 431 exports, pair --verify-only exit 0

### APP-012 controller decisions (worklog/APP-012.md lines 83-87)

- "Supported Windows ABI: x86_64-pc-windows-gnu only"
- "The verified GNU DLL and import library must never be represented as MSVC"
- "The pinned upstream has no MSVC target, so absence of an MSVC artifact is no longer a release blocker"
- "Signing/notarization remains blocked because only unsigned CI runners are available; no identities or protected-host receipts may be fabricated"

### APP-012 PowerShell installer gap (worklog/APP-012.md lines 123-129)

- `scripts/install-oc2.ps1:54-66` expands archive and copies only oc2.exe
- Does NOT validate or install the Windows native opentui.dll closure
- Unlike POSIX installer, has no member-count/member-size/aggregate expanded-payload bounds
- No pwsh exists on macOS host; unsigned Windows CI runner must run frozen PowerShell RED
- Must prove native closure, traversal rejection, and 128 MiB limits on Windows

### CROSS-PLATFORM-RUNNER-CLOSURE-VERIFY (integration tree worklog)

- Line 58: `windows-x64-gnu-unsigned-closure` task definition: build GNU native oc2.exe, package beside opentui.dll, install using repaired PowerShell installer, run version and ConPTY APP-012
- Status: BLOCKED (no GNU target job, installer repair, frozen Windows installer test, or ConPTY test)
- Line 142: B5 finding: "Windows GNU runtime and ConPTY absent"

### Security policy constraints (docs/SECURITY.md)

- Capability-based permissions, no unbounded queue/output
- Scoped cancellation, byte budgets, lazy services
- No shell-string concatenation
- No direct secret file access or unrestricted inherited environment

### TDD contract (docs/TDD.md)

- Frozen tests immutable to implementers
- RED must compile and fail for missing behavior
- Purely declarative tasks still need executable validators

## Target boundary

Produce a complete executable closure specification document covering:
1. DLL and import library loading contract for x86_64-pc-windows-gnu
2. ConPTY lifecycle management
3. Bounded process tree cancellation
4. Transactional PowerShell installer with payload bounds
5. Authenticated daemon/web transport
6. APP-012 installed journey on Windows GNU
7. Resource and receipt schemas
8. Exact commands and failure statuses
9. Explicit exclusion of MSVC targets and signing claims

## Decisions

- Text/static lane: no Rust code changes, no Cargo.toml edits, no CI workflow edits
- Closure doc is a specification that implementation lanes consume
- All hashes and sizes cited from verified TUI-011 evidence at base commits
- MSVC explicitly excluded per controller decision APP-012.md:83-85
- Signing explicitly excluded per controller decision APP-012.md:86-87

## Remaining unknowns

- Actual Windows CI runner availability for executing the frozen PowerShell RED
- ConPTY handle leak behavior under rapid cancellation (needs real Windows test)
- keyring 3.6.3 Windows Credential Manager backend proof (deferred to platform acceptance)
