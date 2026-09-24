# APP-001-REPAIR-RED

## Claim

Task claimed by `ses_f2ebc132cffe34eVmCawNBFKae`. Owned files are
`crates/cli/tests/app001_repair_e2e.rs`, this scratchpad, and this task's claim
ledger row.

## Source evidence

- `crates/cli/src/main.rs:227-269`: bare launch plans from TTY, daemon
  presence, and environment credentials, then calls `chat::prepare_daemon` and
  enters `tui_entry::run_with_dir` without passing a startup view or session.
- `crates/cli/src/daemon_client.rs:775-790`: provider env presence is the
  credential probe used by the bare launch planner.
- `crates/cli/src/tui_entry.rs:1340-1411`: an origin launch fetches a live
  session; an empty daemon reports an offline/no-session error and continues,
  rather than creating/selecting a first session.
- `crates/cli/src/chat.rs:666-698`: the bare client starts a daemon child when
  absent and owns that child in `DaemonLease`, so dropping the lease can kill
  the daemon after the client exits.
- `crates/cli/src/tui_entry.rs:870-1155`: native PTY renderer setup and
  `restore_terminal_modes` are scoped to the interactive loop.

## Observable contract tested

The test is explicitly scoped to macOS because it uses the BSD `/usr/bin/script`
PTY adapter syntax. Linux and Windows require their own platform PTY runners;
this target does not silently claim coverage for them. It uses `OC2_E2E_BIN` with the
Cargo binary fallback, unique loopback ports, disposable HOME/data, bounded
64 KiB captures, bounded waits, and RAII cleanup. It does not invoke a session
command before the bare client. It asserts setup instead of offline/manual
instructions with no provider key, automatic first-session availability with a
fixture key, terminal restoration on `:q`, and descriptor PID/health reuse
across sequential clients.

## RED command and evidence

Command required by the task:

```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test app001_repair_e2e --features native -- --test-threads=1
```

Focused run result: compiling RED, 1/3 tests passed and 2/3 failed. The
frozen test SHA-256 is
`046c88df4d4cd755ed244e04ff8e781ee569e15d236da1a0d81ebdce8703c26d`.
<<<<<<< HEAD
=======
After the harness-only cleanup repair, the intermediate frozen test SHA-256 was
`3ef9c67facf918b2042409396ccb0e1e32a125f6cda877b4a62a20e34a1d6ea2`.
The final macOS-scoped frozen test SHA-256 is
`f5689f0f5b53fd7aacd0934b104a4e0fd5c38bdcdf1943f9f16f03e1535283ab`.
>>>>>>> 4c953e4 (APP-001 repair: scope PTY test to macOS)

Exact command:

```text
rtk env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test app001_repair_e2e --features native -- --test-threads=1
```

Observed failures:

- `bare_without_provider_opens_setup_and_restores_terminal`: output contained
  `daemon offline: daemon is reachable but has no sessions yet; create one with
  opencode-rk session create`, then rendered `OpenCode RK TUI — offline`; it
  did not contain the required setup view. The PTY did exit successfully and
  emitted `ESC [?1049l` restoration bytes.
- `bare_with_provider_creates_and_selects_first_session`: descriptor became
  healthy and the TUI rendered, but `/api/sessions` stayed empty through the
  bounded wait, so no first session was automatically created/selected.
- `sequential_bare_clients_reuse_daemon_and_first_exit_keeps_it_healthy`:
  passed on this revision, documenting that this particular current tree did
  not reproduce the lease-kill failure in the sequential path.

The terminal assertion requires the alternate-screen exit sequence
`ESC [?1049l`, not the ordinary paint reset `ESC [0m`. The repaired
`Root::drop` reads only the disposable `runtime/backend.json`
descriptor, sends TERM to its recorded PID, waits up to two seconds for the
recorded loopback health endpoint to stop answering, then sends KILL and waits
up to 500 ms before deleting the disposable root. No broad process matching or
unsafe code is used. This prevents the test-owned daemon from being orphaned
before descriptor/data cleanup.

The RED is attributable to product behavior, not a harness compile issue. The
first compile attempt found and fixed one owned-test borrow error before the
frozen run; no product or existing test was edited. Current bare `main.rs` does
not route the computed `StartupView::Setup`, and the live TUI path does not
create a first session for an empty authenticated daemon.

The final focused run remained compiling RED: 1/3 passed and 2/3 intended
product failures. The target is `cfg(target_os = "macos")` because
`/usr/bin/script -q /dev/null <cmd>` is BSD/macOS syntax; Linux and Windows
need separate platform PTY runners and are not claimed by this test.

## Remaining unknowns

The native bridge/build availability and exact renderer reset bytes must be
confirmed by the focused command. If compilation fails in an unrelated native
dependency, record that as an environment/product build blocker rather than
weakening this frozen test.

## Integrated GREEN candidate

- Product revision under test wires `DefaultLaunch.view` through
  `main.rs::run` into `tui_entry::run_default`.
- Setup plans enter the native onboarding-gated surface and do not fetch an
  empty session or print offline/manual server instructions.
- Main plans use the authenticated daemon API to create `New session` only
  when the session list is empty, then fetch the durable snapshot.
- Frozen test SHA-256 remained
  `f5689f0f5b53fd7aacd0934b104a4e0fd5c38bdcdf1943f9f16f03e1535283ab`.
- GREEN command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-cli --test app001_repair_e2e --features native --
  --test-threads=1`; result `3 passed, 0 failed` in 1.10s.
- Regressions: `app_start` unit subset `16/16`; frozen native parity `13/13`.
- This is candidate evidence only. The setup surface does not yet persist a
  credential through an OS secure store, and simultaneous first-client
  create-if-empty atomicity is not proven. APP-012 remains open.
