# TOOL-012 timeout and output bounds integration

Status: candidate TOOL-012 behavior GREEN; not release acceptance.
Owner/session: `ses_f3c4de578ffelQv59xDXmOs03B`.
Candidate base: `39aaa57` on `lane/WEB-006-integration`.

## Frozen contract and evidence

- Frozen RED: `crates/tools/tests/shell_tool_bounds.rs`, SHA-256
  `140179dc85ee5cb57685205ab33b2242554a6b1ea3fa0828eac3ed23b735b0d2`.
- Before implementation, the bounded-output regression passed but public
  `ShellTool::execute` ignored `ShellConfig::timeout_secs`; its two-second test
  watchdog expired while the child remained in the pipe-read path.
- `tasks/TOOL-012.md:22-31` assigns timeout and output budgets to
  `ShellConfig` and requires drop cancellation. Commit `fba8686` had already
  established kill-on-drop and synchronous cancellation.

## Implemented boundary

- Public execution applies `ShellConfig::timeout_secs` (minimum one second) to
  the complete concurrent stdout/stderr drain plus child wait. Timeout returns
  `ShellError::Timeout` and owned drop cancellation kills/reaps the child.
- The legacy `ShellTool::timeout` builder remains compatible with the existing
  frozen wrapper-timeout unit; `ShellConfig` is the authoritative public
  execution budget specified by the task card.
- Both pipes are drained concurrently in fixed 8 KiB chunks. At most
  `limit + 1` bytes per stream are retained while excess bytes are discarded,
  preventing a blocked child without unbounded allocation. The existing visible
  truncation marker is then applied.
- No detached reader task, queue, inherited environment, shell-string
  concatenation or extra process was added.

## GREEN evidence

All commands used `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1` and
`--test-threads=1`.

- `cargo test -p opencode-rk-tools --test shell_tool_bounds` — 2/2.
- `cargo test -p opencode-rk-tools --test shell_tool_cancel` — 1/1.
- `cargo test -p opencode-rk-tools shell_tool --lib` — 9/9 after preserving
  the pre-existing external-wrapper timeout behavior.
- Both frozen tests remained byte-identical at their recorded hashes.
- `python3 tools/validate_repository.py` retained 51 pre-existing backlog
  exhaustion failures; protection-policy checks passed.
- Fixtures were direct disposable scripts under temporary directories with
  explicit argv and containment cleanup; no external network, user database or
  credential was used.

## Integration boundary

- This makes `ShellTool` suitable for cancellation-sensitive callers, but the
  WEB-009 HTTP stream still invokes the older `ToolExecutor`. Browser-disconnect
  cancellation remains a separate integration RED and WEB-009 stays blocked.
- Repository validation remains blocked by the pre-existing backlog/ledger
  reconciliation findings. This is candidate evidence, not acceptance.
