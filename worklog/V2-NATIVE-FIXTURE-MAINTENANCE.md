# V2-NATIVE-FIXTURE-MAINTENANCE

Role: independent native contract/test owner (NOT implementer/integrator/acceptor).
Base: worktree HEAD 459c531 over 7fe4656; canonical main c185ab4 (typed accepted).
Owned paths only: `crates/cli/tests/native_daemon_flow.rs`,
`worklog/V2-NATIVE-FIXTURE-MAINTENANCE.md`, `worklog/V2-NATIVE-CONTRACT-EVALUATION.md`.
Product/deps/locks/canonical read-only. Preserve Git history; no drop/ignore/skip.

## Status
Source-preparation only. NO Cargo/Docker run. NOT PREVERIFIED, NOT ACCEPTED.
Candidate is **committed** on the worktree branch (owned paths only; commit
hash changes if this worklog is amended — content is pinned by the test-file
sha256 below). Return state: candidate-source READY with two BLOCKED product gaps.

## Actual hashes (re-measured on the written bytes)
- `crates/cli/tests/native_daemon_flow.rs` — sha256
  `482b78a0aedb2493f3228ac9a7c379939608cc0efbb9c3547217459635b5ce8d`
  (`git diff --stat`: 278 insertions, 231 deletions, 1 file). The test file is
  the content authority; commit hash is reported in the parent handoff because
  further worklog edits re-hash the commit.
- `git status --porcelain` after commit: clean (all three owned paths committed;
  no staged/unstaged/untracked left). Prior reported hash `2eef639…` was stale
  (pre-review bytes); corrected here.

## Which entrypoint owns which behavior (verified, not assumed)
- **Implicit discovery / attach / daemon ownership = default no-subcommand launch.**
  `main.rs:221-258` TTY probe → `NativeTui` → `chat::run` (`chat.rs:48-86`):
  probes `/health`, attaches if live, else auto-spawns `serve` as owned child and
  kills it on exit (`chat.rs:80-84`). Reads `/exit` (`chat.rs:383`).
- **`tui` subcommand has NO implicit discovery.** `tui_entry.rs:512`
  `resolve_origin_bearer(args.origin.as_deref(), …)`; with `origin=None`
  `resolve_origin_bearer:564-566` returns `None`, `live=None` (`:544-557`). Its
  bound path is `--origin` (`docs/USER_GUIDE.md:108`). Native quit `:q`
  (`tui_entry.rs:408-409`). `feature = "native"` only affects `print_native_or_legacy`
  (`:602-615`); no earlier cfg branch changes input parsing.

Consequence: the original T03 intent ("`tui` attaches to a running `serve`
**without `--origin`**") is **not implemented** — a genuine product gap, NOT a
fixture defect. It is recorded as BLOCKED (below), not silently satisfied. T03 in
this file asserts the default launch's real implicit discovery instead (a different,
implemented entrypoint), with no `--origin`.

## The four contracts (authority per assertion)

- **T01 `native_daemon_spawns_when_none_running`** — default bare launch under a
  real PTY owns + publishes exactly one authenticated daemon. Asserts: descriptor
  exists; `pid != std::process::id()`; `pid_alive(pid)`; `/health` 200; exit 0 on
  `/exit`; owned daemon reaped after exit.
  Authority: `main.rs:221-258`, `chat.rs:48-86,80-84`; `daemon.rs:299-312`
  (`publish_backend_descriptor_with_auth`, 64-hex bearer); `daemon_auth.rs:3`
  (`/health` public). G1 `docs/CONVERGENCE.md:24`.
- **T02 `native_no_tty_entry_routes_headless_without_raw_mode_or_daemon`** —
  corrected inverse assertion: redirected no-subcommand `--native` exits
  `HEADLESS_EXIT_CODE` (2), stderr contains `raw mode is refused`, and **no
  descriptor** is published (side-effect absence).
  Authority: `app_start.rs:116-146`, frozen `app001_t4:580-600`,
  `none_arm_headless_both_non_tty_carries_no_role_or_view:614-627`; pinned upstream
  `run.ts:319-320,416`, `cmd/tui.ts:60`.
- **T03 `default_launch_attaches_to_running_serve_daemon_without_origin`** —
  default launch attaches to a pre-existing `serve` daemon with NO `--origin`:
  asserts no `[offline]`, `daemon:` banner, descriptor `pid/origin/token` unchanged,
  `/health` 200 after attach, exit 0 on `/exit`, and the reused daemon **stays
  alive** (not owned).
  Authority: `chat.rs:48-86` (probe+attach), `chat.rs:99-108` (`banner` prints
  `daemon: <origin>`), `chat.rs:80-84` (owned-only kill).
- **T04 `status_frame_carries_live_daemon_values`** — `tui --origin <real daemon>
  --once` renders real live session state. No forged descriptor: reads the daemon's
  own descriptor, creates a session via authenticated `POST /api/sessions`, asserts
  the frame contains the real title + `(live)` + `[context:`/`tokens`.
  Authority: `daemon.rs:299-312`; `tui_entry.rs:564-583,300-311`; `daemon_client.rs:709-762`.

## Resource/lifetime guards actually implemented
- `Drain` reader: continuously drains stdout AND stderr; retains at most
  `MAX_RETAINED_BYTES = 256 KiB`; sets `truncated` instead of growing unbounded.
- `Proc::wait_deadline`: `try_wait` loop bounded by `EXIT_DEADLINE = 30s`, then
  kill+wait. `Drop` also kills+waits.
- `DaemonGuard`: re-validates origin + 64-hex token + `/health` 200 + `pid_alive`
  before signalling; never kills an arbitrary/stale pid. Owned daemon reaped by
  `chat` itself (T01 asserts this).
- Env: `env_clear` + disposable `HOME` and `XDG_CONFIG/DATA/CACHE_HOME` under the
  test temp dir; no user HOME, no secret inheritance.
- No `unsafe`, no new Cargo dependency, no lock edit (`#![forbid(unsafe_code)]`).

## BLOCKED (genuine product gaps, not fixture work)
1. **T03 original intent** — `tui` (subcommand) does not implicitly discover a
   running daemon without `--origin` (`tui_entry.rs:512,544-557`). Fixture can only
   assert the default launch's discovery. Product entrypoint needed: implicit
   origin resolution in `tui_entry::run_with_dir` from the validated descriptor.
2. **T04 model field** — `tui_entry.rs:530/545` hardcodes model `"unset"`; no
   product entrypoint selects a daemon model, so a live-model assertion has no
   configured state. Recorded rather than asserted against a fabricated value.

## Commands actually run (source-prep only)
`read`/`grep` of test + `app_start`/`main`/`tui_entry`/`chat`/`daemon_client`/
`daemon.rs`/`daemon_auth.rs`/`server/src/lib.rs`/`Cargo.toml`; `git log -1` upstream
pin `95daf906`; `git show c01292fd…` OpenTUI pin; `write` (test + 2 worklogs);
`shasum -a 256`; `git diff --stat`; `git status --porcelain`. No `cargo`/`docker`.

## Handoff
- Role/package: independent native contract/test owner / V2-NATIVE-FIXTURE-MAINTENANCE.
- Base 459c531 over 7fe4656; canonical c185ab4. Candidate committed on the
  worktree branch (owned paths only); exact SHA reported in the parent handoff
  because further worklog edits re-hash the commit.
- Changed paths + hash: test sha256 `482b78a0aedb2493f3228ac9a7c379939608cc0efbb9c3547217459635b5ce8d`;
  two worklogs.
- 4 original intents present (T02 corrected, T03 re-homed to the implemented
  entrypoint); no weakened discovery, no forged state, bounded proc output, no
  inherited HOME/secret access.
- Status: source READY with two BLOCKED product gaps. NOT PREVERIFIED / NOT ACCEPTED.
  If `script(1)` PTY or the `native` feature is unavailable at build time, that is a
  concrete blocker for the parent, not a reason to weaken assertions.