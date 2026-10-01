# V2-DISC-101 — native discovery identity contract (source repair)

State: **CANDIDATE DONE**; runtime RED/freeze/product acceptance PENDING.

## Supersession

This source-only repair supersedes the rejected eed9f891 fixture per the
independent evaluator's findings recorded in `discovery-contract-evaluation.json`.
The old three Rust `todo!()` assertions and the thin Python wrapper that used
urllib instead of the actual native caller are replaced by a composite Python
fixture exercising four cases through the real installed binary.

The evaluator's frozen contract is preserved:
- Stale/empty/legacy descriptors are denied before any authenticated API trust;
  no provider/store mutation and no origin contact on denial.
- Empty-token descriptor at the server reader level is a loud
  `Err(DaemonError::Descriptor(...))`, not `None`.
- Foreign live PID identity must be checked through the real discovery/health/
  attach handshake, not local PID equality.
- Owned children must have RAII cleanup with kill+wait before assertion paths.

## Cases

| Case | Descriptor PID | Token | Expected |
|------|---------------|-------|----------|
| VALIDIDENTITY | Actual wrapper PID (setsid+execv preserves) | Well-formed 64-hex | Daemon starts, /health public 200, /api/models Bearer 200, missing Bearer 401, wrong Bearer 403 |
| STALEPID | 99999999 (dead) | Well-formed 64-hex | read_backend_descriptor returns None; ZERO contacts to endpoint |
| LIVEFOREIGNPID | Real owned sleep(60) child PID | Well-formed 64-hex | Health probe allowed; ZERO /api/provider/store effects |
| EMPTYTOKEN | Wrapper PID | Empty "" | read_backend_descriptor returns Err(Descriptor); ZERO contacts |

## Auth controls

Bearer 200/401/403 verified against the actual controlled HTTP endpoint matching
the Rust public router's `require_bearer` middleware behavior
(`daemon_auth.rs:149-176`). `/health` stays public per `daemon_auth.rs:156`.
These are NOT fake endpoint re-enforcements; they match the exact response shapes
from `lib.rs:271-273` (health) and `daemon_auth.rs:164-173` (401/403).

## Attestation

`BUILD_JSON` source_sha is the Rust candidate/source SHA (40 hex), NOT the
upstream TypeScript pin `95daf90670b7c039c436c85537da5fbfe2205b41`. Requires
`native=true`, `profile=release`, matching `binary_sha256` and
`native_library_sha256`. Missing attestation = FAIL, not SKIP.

## PTY wrapper

Python `subprocess.Popen([sys.executable, '-c', WRAPPER_SCRIPT, binary, ...])`
calls `os.setsid()`, `fcntl.ioctl(0, termios.TIOCSCTTY, 0)`, then
`os.execv(sys.argv[1], sys.argv[1:])`. No C compilation, no preexec_fn,
no start_new_session double-call. PID preserved across execv.

## CLI invocation

Exact flags from `native_provider_setup.py`: `[binary, "--data-dir", data_dir]`.
Environment: HOME, XDG_CONFIG_HOME, XDG_DATA_HOME, XDG_RUNTIME_DIR,
OPENCODE_RK_HOME, OPENCODE_RK_DAEMON_ADDR, OPENCODE_PROJECT_DIR, PATH, TERM,
OC2_OFFLINE. No invented `--native` flag.

## Resource bounds

1 HTTP server thread, max 8 requests/case, 128 KiB wire, 256 KiB PTY capture,
512 KiB total evidence, <=20 s per case, <=80 s suite. Absolute monotonic
deadlines via `time.monotonic()`. All socket/select/wait calls finite.

## Cleanup

Ctrl-C (`\x03`) for normal exit attempt; bounded EIO/EOF drain on macOS.
Saved termios compared WITHOUT `tcsetattr` restoration. SIGTERM to owned
process group only; SIGKILL as last resort explicitly FAILs the case.
Only owned Popen children and foreign_child reaped; no `waitpid(-1)`.
`server.shutdown()` + `server.server_close()` + `thread.join(timeout=2)`
on every path including exceptions.

## Evidence

- Upstream: `packages/cli/src/services/daemon.ts:66-72` (healthy),
  `daemon.ts:31-33` (sameRegistration), `packages/server/src/auth.ts:52-63`
- Rust: `daemon.rs:167-208` (read_backend_descriptor), `daemon.rs:54-76`
  (pid_alive), `daemon_auth.rs:149-176` (require_bearer),
  `daemon_client.rs:691-698` (discover), `lib.rs:181-244` (router_with_auth)
- Evaluator: `discovery-contract-evaluation.json` required_frozen_contract
- Existing fixture: `tests/e2e/native_provider_setup.py` CLI/env/attestation
  shapes reused exactly

## Integrator commands

```text
git diff --check
python3 -m py_compile tests/e2e/discovery_identity.py
rustfmt --check crates/server/tests/discovery_auth_red.rs
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 \
  OC2_TEST_BINARY=/absolute/oc2 NATIVE_LIBRARY=/absolute/libopentui.dylib \
  BUILD_JSON=/absolute/build.json \
  cargo test -p opencode-rk-server --test discovery_auth_red -- --nocapture
```

Base `1bd286a9a4ac1193185a11d8443f68a8adc50b21`.
Exact changed paths: this worklog, `crates/server/tests/discovery_auth_red.rs`,
`tests/e2e/discovery_identity.py`.
Historical candidate `eed9f8918c2939c5cc4e020f82440d0d46f65283` preserved
in Git ancestry as SUPERSEDED_BY this successor.
