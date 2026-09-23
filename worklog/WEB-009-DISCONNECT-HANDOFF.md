# WEB-009 disconnect handoff

Claimed by `ses_f35a98f43ffeJyP0Q9B9YSkB0R` after ledger claim from `not-started`.

History preserved: prior failed provider session left the unverified candidate in
`crates/server/src/lib.rs`; `worklog/WEB-009-DISCONNECT-IMPLEMENTATION.md` records
the PID-publication failure and remains evidence, not acceptance.

Status: BLOCKED. Candidate restored exactly to `221b8ff` in
`crates/server/src/lib.rs`; no product diff remains. Frozen test hash:
`95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8`.

Initial evidence: frozen RED reported `tool parent survived browser disconnect`; prior implementation attempt failed PID publication. No test edits.

Audit target: remove duplicate broker authorization, uncancellable shell fallback,
noop-waker polling, and lossy shell failures while preserving the frozen contract.

Failure 1: the directly owned dormant `ShellTool::execute` future remained pending
after the `tool_call` chunk and the frozen test failed with
`tool fixture PID publication deadline exceeded`.

Failure 2: replacing it with an abort-on-drop spawned Tokio task still failed with
`tool fixture PID publication deadline exceeded`; no fixture PIDs were published
before the deadline.

Root blocker: `crates/tools/src/shell_tool.rs:161-228` exposes only opaque
`ShellTool::execute`; it exposes no process-start/readiness handle. The server cannot
establish a non-timing startup boundary in `crates/server/src/lib.rs` alone. No
arbitrary sleep, test edit, tools-layer change, or unowned fallback was added.

Verification: frozen disconnect command failed in both attempts; test source stayed
unchanged. After rollback, `crates/server/src/lib.rs` matches `HEAD 221b8ff` exactly.
