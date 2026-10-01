# LIVE-PERMISSION-CONTRACT candidate handoff

## Scope and authority

This candidate owns only `crates/server/tests/session_permission_live.rs` and
this worklog.  It is a test-owner fixture for the live, authenticated session
turn permission contract, not a product implementation or an acceptance claim.

The pinned upstream authority is OpenCode commit
`95daf90670b7c039c436c85537da5fbfe2205b41` in
`/Users/mymac/Projects/opencode-upstream-reference`.  The reference behavior
used here is: pending permission GET returns a JSON array; reply is singular
`POST /api/session/:sessionID/permission/:requestID/reply` with HTTP 204 for a
valid decision; wrong session and missing request are 404; missing and wrong
bearer credentials are 401/403.  The current Rust server still has the
intentional runtime RED because it does not yet publish and await
execution-owned permission requests.

## Fixture proof and security boundary

The server's actual broker root is `std::env::current_dir()`, not
`OPENCODE_PROJECT_DIR`.  Each fixture writes its ordinary target in a disposable
`tempdir()` outside that current-directory root, and `broker_requires_human_for_target`
asserts that the real `PermissionBroker` returns `RequireHuman`.  The
`OPENCODE_PROJECT_DIR` value is retained only as provider/session fixture
configuration; it is not treated as caller authority.  The mandatory `.env`
case remains inside the disposable workspace and verifies that wildcard
permissions cannot override protection or create a file.

## Observable cases

The five public tests retain the meaningful live cases:

* pending request pauses before a file write/provider continuation; `once`
  executes one write, one typed tool result, one continuation, and duplicate
  reply cannot replay it;
* rejection has no file side effect and no continuation;
* missing/wrong bearer leaves the request pending (401/403);
* wrong-session, stale, and duplicate replies cannot consume/replay another
  request;
* wildcard permission and forged replies cannot approve a mandatory protected
  `.env` write.

## Bounds and cleanup

Requests are capped at 128 KiB, response bodies at 256 KiB, and decoded wire
responses at 272 KiB.  Each case is bounded to 20 seconds and the suite to 90
seconds.  Provider accept/read/write work uses finite deadlines and a bounded
nonblocking accept loop.  Client connection setup retries transient macOS
listener readiness errors under one absolute five-second deadline.

Fixture setup explicitly releases the environment guard and stops/joins the
owned provider on fallible setup paths after the provider has started.  Normal
case teardown aborts and awaits Axum, joins the owned turn client, then stops
and joins the provider before assertions after cleanup.  No Cargo build, runtime
server execution, PTY, network, dependency, product, canonical, controller,
ledger, or reference changes were made by this candidate.

## Proposed verification

The integrator/test verifier should run the focused frozen command from the
canonical branch with the repository's configured Cargo limits, for example:

```text
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-server --test session_permission_live -- --nocapture
```

This worker did not run Cargo/build/runtime/network gates.  Static verification
performed here is standalone formatting/diff inspection only.  The expected
result remains runtime RED until the server wires execution-owned permission
publication and reply resumption.
