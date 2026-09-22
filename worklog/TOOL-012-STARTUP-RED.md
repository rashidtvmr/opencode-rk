# TOOL-012 startup-readiness RED

## Claim

- Task: TOOL-012
- Session: ses_f350b2ee1ffeev3X7fXp9IbaOX
- HEAD at claim: 88ae9a2e9c3f4f9bbecd4ca975bcf90468a58ae8
- Owned product test: `crates/tools/tests/shell_tool_startup.rs`
- Prior TOOL-012 history: process-tree cancellation surface was previously marked completed by another process. This reopening covers the newly discovered startup-readiness integration contract only.

## Source evidence

- `crates/tools/src/shell_tool.rs:161-283`, `ShellTool::execute`: public async API performs allowlist, broker, spawn, bounded output, wait, and result return; no startup sender, callback, readiness handle, or child-wrapper handshake.
- `crates/tools/src/shell_tool.rs:212-223`: process group configured, `Command::spawn()` called, child retained internally; parent-side spawn is not an externally observable child-executed readiness event.
- `crates/tools/src/shell_tool.rs:286-313`: `cancel` and `Drop` kill the recorded process group.
- `crates/tools/tests/shell_tool_process_tree.rs:13-134`: existing disposable PID fixture demonstrates cancellation tree cleanup, but only by polling fixture files; it has no startup notification contract.
- `worklog/WEB-009-DISCONNECT-STARTUP.md:15-19,32-51`: disconnect repair requires an owned task and a startup signal after child spawn; existing API exposes no start/readiness handle.

## Contract under test

1. A bounded startup notification is distinct from `Command::spawn` and arrives only after the child wrapper writes its readiness marker.
2. Child execution remains owned by the cancellation task. Abort after readiness kills recorded parent and descendant and suppresses delayed sentinel creation.
3. Startup instrumentation does not enter stdout/stderr; normal output is byte-faithful.
4. Startup failures are explicit and bounded. Channels have finite capacity.

## RED adapter decision

No production startup API exists at this revision. The test-local `LegacyStartupAdapter` drives the real `ShellTool::execute` callable API. It emits only `Completed` after execute returns, intentionally not pretending that completion is startup. A blocking child fixture writes `child-wrapper-ready` before waiting, then the RED test proves the child began while no distinct startup event can be delivered. This is a semantic RED, not a mocked success.

Intended additive production shape for the implementation lane:

```text
ShellTool::execute_with_startup(config, bounded startup sender)
```

The implementation must send a typed startup event from the child-wrapper handshake, not from the parent immediately after `Command::spawn`, while preserving the existing `ShellResult` output contract and owned cancellation.

## Verification

- Focused command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test shell_tool_startup -- --test-threads=1` (outer tool timeout 120s; macOS image has no `timeout` executable)
- Observed RED: compile succeeds; 4/5 tests pass; `startup_notification_requires_child_wrapper_readiness` fails at line 223 with `child wrapper became ready, but no distinct startup notification arrived: Err(Elapsed(()))`. The adapter can emit only `Completed` after execute returns; the fixture is intentionally blocked, so no false startup event exists.
- Frozen test SHA-256: `b083736af8a9b540252efd3ad75e6471fe8444ab8fd4967c61f4a190b2e9b9ff`
- No product source, manifest, frozen test, policy, or dylib changed.

## Remaining blocker

Production API absent from `shell_tool.rs`; implementation lane must add the bounded child-wrapper startup handshake. TOOL-012 remains blocked, not completed, until the frozen startup test passes on the integrated tree.
