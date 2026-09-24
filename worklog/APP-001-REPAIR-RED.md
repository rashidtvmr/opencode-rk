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

The test uses `/usr/bin/script` as the Unix PTY adapter, `OC2_E2E_BIN` with the
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

The RED is attributable to product behavior, not a harness compile issue. The
first compile attempt found and fixed one owned-test borrow error before the
frozen run; no product or existing test was edited. Current bare `main.rs` does
not route the computed `StartupView::Setup`, and the live TUI path does not
create a first session for an empty authenticated daemon.

## Remaining unknowns

The native bridge/build availability and exact renderer reset bytes must be
confirmed by the focused command. If compilation fails in an unrelated native
dependency, record that as an environment/product build blocker rather than
weakening this frozen test.
