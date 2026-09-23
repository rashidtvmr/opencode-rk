# TOOL-012 startup-readiness RED

## Claim

- Task: TOOL-012
- Session: `ses_f33ab1b02ffeqfziBahkwvDgb5`
- HEAD at claim: `1d20224ecca26e341c15783089cd5573b63c8de8`
- Owned test: `crates/tools/tests/shell_tool_startup.rs`
- Reopening explicitly authorized by the user after independent review. Previous replacement worker stopped without changes; orchestrator reclaimed the ledger claim before this session.
- Ownership boundary: startup test, this scratchpad, TOOL-012 ledger row only. No product edits.

## Review and replacement

- Invalid prior RED: `b083736af8a9b540252efd3ad75e6471fe8444ab8fd4967c61f4a190b2e9b9ff`.
- Review reason: prior test used a `StartupEvent` enum and a completion-only adapter, rather than the required compile-compatible standard API. It did not exercise the caller-owned readiness path or exact production method-resolution contract.
- Replacement keeps semantic RED and removes the invalid event abstraction. Fallback invokes existing `ShellTool::execute` only and never emits a valid startup PID before completion.

## Source evidence

- `crates/tools/src/shell_tool.rs:161-284`, `ShellTool::execute`: allowlist, broker, spawn, bounded output, wait, result; no startup sender or readiness probe.
- `crates/tools/src/shell_tool.rs:212-227`: process group configured, `Command::spawn()` called, child retained internally; parent-side spawn is not child-wrapper readiness.
- `crates/tools/src/shell_tool.rs:286-313`: `cancel` and `Drop` kill the recorded process group.
- `crates/tools/tests/shell_tool_process_tree.rs:13-134`: disposable parent/descendant fixture and cancellation containment; no startup notification.
- `worklog/WEB-009-DISCONNECT-STARTUP.md` was referenced by the prior review; this replacement does not broaden ownership into WEB files.

## Exact RED adapter contract

Test-local fallback signature:

```text
execute_with_startup(self, config: ShellConfig, readiness_path: PathBuf, startup: tokio::sync::mpsc::Sender<u32>) -> Pin<Box<dyn Future<Output=Result<ShellResult,ShellError>> + Send>>
```

Fallback body discards `readiness_path` and `startup`, then awaits existing `execute`. This is compile-compatible and semantic RED. A future inherent `ShellTool::execute_with_startup` with the same public signature wins Rust method resolution over the test-local trait method, requiring no test edits and avoiding a private event type.

## Contract under test

1. Caller supplies a unique disposable readiness path. Fixture atomically writes its bounded numeric PID there only after its wrapper is released, then starts a descendant and blocks.
2. No startup event before readiness. Exactly one matching `u32` PID after readiness. Bounded channel; no duplicate.
3. Abort after event kills wrapper and descendant; delayed sentinel never appears.
4. Readiness instrumentation is absent from normal stdout/stderr; output remains byte-faithful.
5. Nonexistent command returns bounded `ShellError::Spawn` and emits no event.
6. All startup-specific cases call `execute_with_startup`; disposable fixtures use exact argv paths, no shell-string concatenation, bounded waits, explicit failure containment.

## Verification

- Command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test shell_tool_startup -- --test-threads=1`
- Compile: pass.
- Semantic RED: 2/4 pass; `startup_notification_requires_child_wrapper_readiness` fails at line 221 with `readiness must publish exactly the wrapper PID`, `left: None`, `right: Some(<fixture PID>)`; `startup_instrumentation_is_absent_and_normal_output_is_faithful` fails at line 256 with `startup event missing`. Both failures are missing production readiness events; spawn, bounded sender, cancellation/containment cases pass.
- Test SHA-256: `4dae909cd1db9c724a42ab731ef5d0074f8a423561392a6b321cb9ecde442691`.
- `git diff --check`: pass.
- No product source, manifest, policy, or verifier changed.

## Blocker

Production `ShellTool::execute_with_startup` is absent from `crates/tools/src/shell_tool.rs`. TOOL-012 remains blocked, not completed, until implementation adds the bounded caller-path readiness handshake and passes this frozen test hash.
