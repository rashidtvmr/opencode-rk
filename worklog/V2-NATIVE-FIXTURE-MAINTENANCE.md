# V2-NATIVE-FIXTURE-MAINTENANCE

Role: independent native fixture contract evaluator/test owner; not product
implementer, integrator, or acceptor. Only authorized writes: this report, the
contract evaluation, and `crates/cli/tests/native_daemon_flow.rs`.

## Candidate status

Base supplied by parent: `f8c08e1759c0d10d3634ca3566ee767600c4e1e7`.
Candidate is currently source-only, **NOT PREVERIFIED / NOT ACCEPTED**. No Cargo,
Docker, build, network-install, or runtime gate was run (heavy-gate grant absent).
`rustfmt --edition 2024 crates/cli/tests/native_daemon_flow.rs` and
`git diff --check` succeeded. Exact test SHA-256:
Current test hash is recorded after final formatting and helper self-checks.

The candidate test source is uncommitted, preserving supplied branch history.
No files outside the three-path grant were changed.

## Assertion authority / dispositions

- **T01** remains real PTY bare/default entrypoint, testing fresh native default
  launch, daemon-owned real descriptor publication, loopback health, `/exit`,
  and owned-daemon teardown. Authority: local `docs/CONVERGENCE.md` G1/G5
  (`7-19,24-28`), `crates/cli/src/main.rs:221-258`, `chat.rs:48-86` and
  `daemon.rs:299-312`. This is the real default product entrypoint (without
  `--native`, which selects the legacy `tui_entry` branch).
- **T02** tests redirected default `--native` exits 2, gives the frozen refusal,
  publishes no descriptor. Authority: `app_start.rs:116-146,580-627`; pinned
  upstream `95daf90670b7c039c436c85537da5fbfe2205b41`,
  `packages/opencode/src/cli/cmd/run.ts:319-320,416` and `cmd/tui.ts:60`.
  This preserves the app_startTTYexit2/no-raw/no-owner contract; not a piped
  raw-mode bypass.
- **T03 remains genuine RED**: it launches `opencode-rk tui` on a real PTY,
  without `--origin`, and expects live state from a real pre-existing `serve`
  descriptor, including same pid/origin/bearer and healthy daemon after exit.
  It does not substitute `chat` or add `--origin`. Evidence for expected failure:
  `tui_entry.rs:512-557` resolves no bearer and enters the no-live path absent
  origin; `interactive_loop` (`:394-429`) renders `unset`/offline and `:q` exits
  (`:408-409`). User's explicit instruction preserves the original T03 intent;
  no test-side repair should conceal this product gap.
- **T04** binds a scriptable `tui --origin <real-origin> --once` to a real
  daemon, creates a real session over authenticated HTTP, and asserts fetched
  live session + context/tokens. Authority: `tui_entry.rs:526-545,564-583`,
  `daemon_client.rs:709-762`, `daemon.rs:299-312`. No synthetic pid, token,
  model, or descriptor. A model-field assertion remains explicitly blocked:
  this fixture has no authority or product API to choose a configured model.
  Source currently passes literal `unset` to `render_frame` (`tui_entry.rs:530`)
  and there is no model-selection input in this `tui --once` flow. Do not invent
  a catalog model or declare `unset` a defect without a user/product authority.

## Safety/resource construction

- Stdout/stderr are continuously drained with retained text capped at 256 KiB.
  PTY roots use `/usr/bin/python3` directly; `pty.fork` creates the CLI session.
  The driver records its known child PID privately, handles TERM, kills the PTY
  child group in `finally`, and reaps the leader. Rust cleanup requests cooperative
  driver termination before the recorded CLI-group fallback. Drain joins wait for
  `is_finished` within a finite deadline and refuse an unbounded join.
- `serve` uses piped stdout/stderr, and pipe extraction has cleanup on each
  fallible branch.
- Child exit waits use `try_wait` with deadline, then kill and reap. `Proc::drop`
  also kills/reaps and joins both drains.
- Test environment is cleared; HOME and XDG dirs are disposable; TMPDIR points
  to the fixture and the fixture prefix is `pp-<pid>-<id>`. Construction rejects
  a runtime `opencode-rk.sock` path over 100 bytes.
- Daemon cleanup only signals when the currently published descriptor matches
  the tracked pid/origin/bearer, the bearer is wellformed, loopback origin is
  correct, health is good, and pid is alive. Bearer is not formatted in output.
- The repository contains `tests/e2e/native_interactive_pty.py`; this Rust
  fixture uses `/usr/bin/python3` and an embedded Python stdlib PTY helper with
  explicit argv (no shell string concatenation), no unsafe, and no dependency
  changes. The helper's `pty.fork` creates the PTY child session, records its PID in a
  private fixture file, handles SIGTERM as cancellation, and reports a normal
  already-reaped child status without double-waiting.

## Actual commands and limits

Read-only inspection included `PLAN.md`, `docs/CONVERGENCE.md`, `docs/TDD.md`,
the test, `chat.rs`, `tui_entry.rs`, `main.rs`, existing Python PTY test, and
pinned upstream `run.ts`. Executed only:

```text
rustfmt --edition 2024 crates/cli/tests/native_daemon_flow.rs
git diff --check
shasum -a 256 crates/cli/tests/native_daemon_flow.rs
git rev-parse HEAD
```

No Cargo command, so the Rust fixture has neither compiling RED evidence nor
runtime evidence yet. Parent needs heavy-gate grant and must independently run
the focused native test. T03 is an expected product RED by code path; T01/T02/T04
outcomes are unverified. Source readiness is not acceptance.

## Parent recovery of interrupted fixture preparation

The first helper handoff failed portability/argv review; the subsequent repair
ended without a complete handoff. Main preserved the dirty three paths and both
stray PID files at
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/native-fixture-interrupted-7v7fzxae`.
Main, as independent native fixture owner, completed the mechanical corrections:
explicit `-c DRIVER RECORD -- EXE ARGS` indexes, normalized signalled status,
disposable working directory, and `Proc::spawn_pty` at both actual PTY call sites.

Executable checks extracted the exact embedded driver and ran:

```text
/usr/bin/arch -arm64 /usr/bin/python3 /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/native-fixture-interrupted-7v7fzxae/selfcheck.py
```

Results: `/usr/bin/true` exit 0; `/usr/bin/false` exit 1; blocking CLI and owned
descendant both gone after TERM, with driver reaped before the deadline. No new
`-c` or `--` PID files appeared. `selfcheck-result.json` retains results. Rustfmt
and diff check passed. This proves helper behavior only; the focused Cargo target
and real default/discovery contracts still need compiling/runtime verification.
