# APP-012-LIVE-FILE-TOOL-INROOT-W3 scratchpad

Claim: APP-012-LIVE-FILE-TOOL-INROOT-W3, session ses_f1e63517dffePCNFlr4qGy6zP4.
Route: 9router/oc/muse-spark-1.3-contributor-free (delegation allowlist verified).

## Source evidence (base ecec045)
- `crates/tools/src/executor.rs:106-119`: `ToolExecutor::execute` dispatches
  only `bash`/`shell`/`echo`; any other name (incl. `write`) returns
  `Unknown tool: {name}`. Genuine RED expectation.
- `crates/server/src/lib.rs:1242`: broker policy root is
  `std::env::current_dir()` via `SecurityPolicy::lean_default`.
- `crates/server/src/lib.rs:1505-1600`: Executing stage authorizes
  `OperationIntent::Tool` via broker then `executor.execute`; non-enabled
  tools get "not permitted by turn policy" (not Unknown tool), so the
  Unknown-tool RED proves executor has no `write` handler yet.
- `crates/security/src/lib.rs:244-247`: `OperationIntent::Tool` baseline is
  `Decision::Allow` (file-root gates do not apply to bare Tool intents);
  frozen test `live_file_tool_allowed.rs` (commit 299bbbf) therefore disputes:
  tempfile path lies outside project root.
- `crates/tools/src/registry.rs:109-110`: `write` is registered/discoverable
  but NOT executable in executor. Registry != execution.

## Observable contract
- Success: live stream `tool_output` contains success/written, exact file
  content at `<cwd>/.oc2-test-<pid>/allowed.txt` equals fixture, 2 provider
  rounds, round-2 input carries `function_call_output` with success text,
  transcript persists success without payload, `tool_call` args redacted
  (no payload in public events/messages), fixture dir cleaned up.
- Failure/RED: `tool_output` carries `Unknown tool: write`; assertions on
  success/file-content fail.
- Bounds: provider request <=256KiB, response body 64KiB, 20s read timeout,
  exactly 2 provider rounds, no secrets in events/transcript.

## Tests
- New: `crates/server/tests/live_file_tool_allowed_inroot.rs`
  `live_write_tool_inroot_is_allowed_and_persisted`. No product/test edits.
- Frozen disputed `live_file_tool_allowed.rs` (c914953e/299bbbf) untouched.

## Decisions
- Copied bounded HTTP fixture shape from `299bbbf:...allowed.rs`; replaced
  `tempfile` target with `CwdFixture` under exact `current_dir()` + pid dir.
- `OPENCODE_RK_TURN_TOOLS=write` enables advertisement; executor RED still
  fires `Unknown tool: write` on base.
- Env guards restore vars on drop; single-threaded jobs for determinism.

## Remaining unknowns
- None for RED. Implementation lane must add real `write` execution honoring
  broker file-root gates (FileAction::Write) and keep redaction/transcript
  honesty.
