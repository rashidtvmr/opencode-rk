# DISC-106 — Real OS sandbox backend with inherited-capability close

Claim: session ses_f41f2c276ffeEYSUeSJGzV14cz, status in-progress, HEAD 1614754.

## Source evidence
- `crates/security/src/sandbox_real.rs:36-54` — `is_available()` (kernel>=5.13 + `/proc/filesystems` landlock entry), `backend_name()`; docs constraint: crate `#![forbid(unsafe_code)]` (`lib.rs:2`) blocks raw Landlock syscall wrappers here.
- `crates/security/src/platform_matrix.rs:123-162` — fail-closed `UnsupportedSandbox` receipt; `:87-105` matrix rows all `isolated:false` with honest limits.
- `crates/security/src/app_policy.rs:241-243,362-369` — `PolicyDeny::UnsupportedSandbox`, `require_os_sandbox` always Err; test `:708-717` frozen fail-closed.
- `crates/security/src/sandbox.rs:1` — policy only, no Landlock syscalls; `resolve_path` canonicalization pattern reused.
- `crates/security/src/lib.rs:2` — `#![forbid(unsafe_code)]`; `lib.rs:3-23` mod list (no `sandbox_real`, no `os_backend` yet).
- `crates/security/Cargo.toml` — deps: contracts, serde, serde_json, thiserror. NO libc. Integrator-only Cargo edits → no new deps.
- Host: Linux 7.0.0-31-generic, `/proc/filesystems` has NO landlock entry → `is_available()==false` here. Landlock engage on this host = BLOCKED-with-evidence (orchestrator-authorized honest path).
- `docs/SECURITY.md:38-44,49-56` — regex/prompt never a sandbox; backend must close inherited caps, tested on real platform; denial asserts absence of side effects.

## Target boundary
Owned: `crates/security/src/os_backend.rs` + ONE wiring line `pub mod os_backend;` in `lib.rs`. Pure std, `#![forbid(unsafe_code)]`, thiserror only.
API: `platform_support()` detection; `engage()` → always `Err(Blocked)` with evidence (no syscall backend linked in this crate — never fake); `FsGrant` path confinement; `restricted_spawn()` (env_clear, stdio null, kill_on_drop — documented NOT a sandbox); `run_confined()` pre-exec grant gate (violation → typed err, child never starts); `kill_on_violation()`; `count_open_fds()` via `/proc/self/fd`.

## Tests (in-module, frozen after RED)
- T01 engage/confine: unsupported host → `engage()` Err(Blocked) w/ evidence strings; grant confines fixture access (in-grant ok, outside denied).
- T02 inherited caps closed: extra parent FD absent from child `/proc/self/fd` listing; parent-only secret env absent from child env.
- T03 unsupported → explicit BLOCKED, never silent allow (`platform_support().available==false` consistent with /proc; `require_supported()` Err).
- T04 violation kills child → typed error, marker file absent, child reaped (no partial effects).
- T05 50 setup/teardown cycles → FD count stable, temp dirs removed.

## Decisions
- No libc dep (can't edit Cargo.toml); no unsafe (forbid). Landlock-engage honestly BLOCKED everywhere until a syscall crate exists — module docs say so.
- Tests in-module (ownership: single owned file; filter `os_backend` matches).

## Landing (ses_land_disc106)
- Reclaimed dead session ses_f41f2c276ffeEYSUeSJGzV14cz, claimed ses_land_disc106.
- Compile fix in T05 only (impl untouched): removed `as_raw_fd`/`from_raw_fd`+`unsafe` (violated `#![forbid(unsafe_code)]`), replaced with safe open+drop probe. Zero test-assertion edits.
- GREEN: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 cargo test -p opencode-rk-security os_backend` → 5 passed 0 failed (t01..t05 ok). No-regress `--lib` → 148 passed 0 failed.
- Owned: `crates/security/src/os_backend.rs` (615L, 5 tests) + `pub mod os_backend;` wiring in `lib.rs:13`.

## Unknowns: none blocking.
