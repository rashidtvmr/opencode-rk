# SEC-RED-OS

## Claim and stale-claim recovery

- Task: `SEC-RED-OS`
- Session: `ses_f2db129e9ffeliWnPz0Avf6axT`
- Branch: `lane/SEC-RED-OS-RETRY`
- Reclaimed prior owner `ses_f2dc11dd3ffeWw31mT4dyuJYOM` after verifier evidence in
  `worklog/SEC-RED-OS-VERIFY.md` at `62f43ce`: candidate test, author worklog,
  commit, and push were absent.

## Source evidence and contract decision

- Host: `aarch64-apple-darwin` / macOS.
- `crates/security/src/os_backend.rs:112-146` (`platform_support`) reports
  `macos`, no Landlock detection, `enforcement_linked = false`, and
  `available = false`.
- `crates/security/src/os_backend.rs:149-166` (`require_supported`) returns
  `Err(Blocked)` when no real backend is linked.
- `crates/security/src/os_backend.rs:168-181` (`engage`) always returns
  `Blocked`; the module explicitly states at lines 31-37 that non-Linux
  platforms are unavailable and fail closed.
- `crates/security/src/lib.rs:2` forbids unsafe code. The security crate has
  no syscall backend dependency (`crates/security/Cargo.toml:9-13`).
- `docs/SECURITY.md:38-56` requires actual platform execution and inherited
  capability closure; `docs/SECURITY.md:51-54` rejects tests that do not run
  the real backend.
- `.github/workflows/ci.yml:29-35` provides Linux and Windows CI runners, but
  this lane has no supported-platform runner. Installed targets contain only
  `aarch64-apple-darwin`.

## Decision

Blocked. No legitimate local RED exists. A macOS test asserting OS enforcement
would fail because macOS is intentionally unsupported, not because a supported
contract is missing. A Linux-only `#[cfg]` test would execute zero tests here.
Cross-compiling cannot prove runtime isolation. Even Linux detection currently
has `enforcement_linked = false`, so an actual supported-platform RED requires
a Linux runner with a syscall-capable backend contract and public API proving
the expected enforcement behavior. Do not create `crates/security/tests/phase1_os_isolation.rs`.

## Validation

- Existing API test: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 180 cargo test -p opencode-rk-security --lib os_backend -- --test-threads=1`
- Candidate RED command intentionally not run: absent target would be Cargo
  target-resolution failure, not behavioral RED.
- Required final check: `git diff --check`.

## Remaining unknowns

- Need an authorized Linux runner and a linked syscall-capable OS backend before
  authoring a supported-platform RED. This lane must not add dependencies or
  change production policy.
