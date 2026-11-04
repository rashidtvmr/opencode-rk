# TOOL-006 verified state (lane F3-MCP, base 6353b755)

The full live background-shell path is landed on this lane (`0b150dea` transport
plus consumer, `af2d3140` execute argument rows). This note records the
re-verified state at base `6353b755` after later commits; line numbers drifted
from the original handoff, behavior did not.

## Verified wiring (drift noted where moved)

- Routes `crates/server/src/lib.rs:243-245` `/api/shell`, `/api/shell/{id}`,
  `/api/shell/{id}/output`.
- Handlers `list_shells` (`lib.rs:922`), `get_shell` (`:928`, 404 missing),
  `get_shell_output` (`:935`, cursor+limit, 400 for limit 0 or >1048576).
- Registry `crates/server/src/shell_registry.rs`: `MAX_SHELL_OUTPUT_BYTES`
  8 MiB (`:5`), `MAX_OUTPUT_LIMIT` 1 MiB (`:6`), char-boundary-safe
  cursor/limit slicing (`:78-97`), tail trim on bounded insert (`:48-58`).
- Producer `native_shell_stream` (`crates/server/src/lib.rs:947`) seeds/
  appends/completes registry output.
- CLI consumer `crates/cli/src/tui_entry.rs`: `shell_scan_at_ms` (`:8842`),
  `live_shells` (`:8845`), tick (`:9416`), `parse_live_shell_list` (`:9424`),
  `live_shell_scan_step` (`:9456`), fetch closure + `set_live_shell_rows`
  (`:9488`).
- Models `crates/cli/src/native_shell.rs`: `SHELL_DISPLAY_LIMIT` 1 MiB tail
  bound (`:1007`, trim `:2183-2184`), `SHELL_POLL_INTERVAL_MS` 1s (`:1009`),
  `parse_live_shell_list` (`:1853`), `apply_live_shell_output` (`:1866`),
  `parse_live_shell_entries` (`:1874`), `LIVE_DRAIN_FETCH_BUDGET` 32 (`:1888`),
  `live_shell_scan_step` (`:1901`), `live_output_request` (`:1956`),
  `BackgroundShell` (`:1974`).
- Paint `crates/cli/src/native_view.rs`: `live_shell_rows` (`:271-274`),
  `set_live_shell_rows` (`:1792`), Shell tab body via `dock_block_for`.

## Clause verification

1. Moved-session cwd: shell cwd is captured once at create in
   `ShellRegistry::start` (`shell_registry.rs:36-37`) into `ShellRecord.cwd`;
   the registry exposes no rename/move hook and `rename_session`
   (`crates/server/src/lib.rs:1202`) never touches shell records. A session
   move cannot re-home a shell.
2. Bounded output: client output is trimmed to a `SHELL_DISPLAY_LIMIT` (1 MiB)
   tail in `apply_live_shell_output` (`native_shell.rs:2183-2184`) and the
   server caps both stored bytes (8 MiB tail) and per-request limit (1 MiB).
3. Collapsed-no-poll: `BackgroundShell::sync`/`poll` return `None` and clear
   `next_poll_ms` while `background && !expanded`
   (`native_shell.rs:2132-2135,2145-2147`); covered by
   `tool006_collapsed_background_shell_never_polls_until_expanded`.

Focused tests: `tool006` 4/4 passed, `tool010` 1/1 passed
(`--test-threads=1`, bin oc2).

## TOOL-010 blocker (verbatim)

No `execute` (codemode) tool exists in canonical:
- `crates/tools/src/registry.rs` `register_builtins` registers only
  bash/shell, glob, grep, file, read, write, edit, websearch, webfetch, echo.
- `crates/tools/src/executor.rs` `execute()` match (`:572-592`) has no
  `"execute"` arm.
- No codemode crate exists under `crates/`.
- `native_result_payload` (`crates/server/src/lib.rs:2317`) emits only
  `{format,output,success}` with no `metadata` field, so
  `native_chat_projection.rs:345-350` always reads `metadata=""`.
- Per-child toggle is unwired: the transcript hit-test maps every painted
  execute row to the same part index (`native_view_chat.rs:2816-2841`,
  `tool_out.push((row, tool_state_index))`), and clicks open `DialogExecute`
  (`tui_entry.rs` execute dialog openers).

Upstream contract (`packages/codemode/src/interpreter/execute.ts:44,75,195,197`
writes `metadata.toolCalls`; `packages/tui/src/routes/session/index.tsx:3401-3449`
`Execute`, `:3348-3395` `ExecuteCallView`): each child call renders collapsed as
`executeCallSummary(call)` (summary row) with a per-call local `expanded`
signal; the expanded child shows the child tool name and a left-bordered block
of `key: value` input rows. Without a producer there is no reachable runtime
seam; a click-to-toggle would also require per-child state through the shared
flattened-row path. No render changes landed; upstream painting of argument
rows is present (`af2d3140`).
