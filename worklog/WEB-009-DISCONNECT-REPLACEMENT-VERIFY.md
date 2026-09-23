# WEB-009 disconnect replacement verification

## Claim

- Task: WEB-009
- Session: `ses_verifier_web009`
- Scope: verifier-only. No product/test edits. Verify replacement hash
  `80d8d505a48e018a292b132bde82208bb3afd285ad8b80342365be2f48d28300` at
  `c1eea8a` in branch `lane/WEB-009-test-replacement`.
- Scratchpad + ledger only.

## Environment (recorded at start)

- Host: macOS, 25,769,803,776 bytes (~25.7 GB) total RAM
- Page size: 16,384 bytes
- Initial vm_stat: Pages free 84,493 (~1.3 GB), pages wired down 177,280
  (~2.8 GB), pages throttled 0, swapins/swapouts 0
- Below the required 2 GiB free-memory reserve; no swap pressure was observed.
  Further heavy verification must wait for memory recovery.

## Original review reference

`worklog/WEB-009-DISCONNECT-CONTRACT-REVIEW.md` proved the original frozen
test at hash `95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8`
is nondeterministic: readiness file is written before `exec`, fixture PID
files are written after `exec`, and the client disconnected immediately on
`tool_call` before PIDs were guaranteed published.

## Replacement contract (test-only synchronization fix)

The replacement hash `80d8d505...` moves `wait_for_pids` to run BEFORE
`stream.shutdown(Shutdown::Both)`, confirming fixture PID publication before
disconnect. All original assertions preserved:

- tool_call observed (line 425-427)
- shared permit returned (line 429, wait_for_permits)
- provider socket closed (line 431)
- parent/descendant dead (line 436, wait_for_fixture_exit)
- sentinel absent (line 437)
- no durable tool/assistant success (lines 444-446)

## Source evidence

- `crates/server/tests/runtime_wiring_disconnect_http.rs:304-308`:
  `wait_for_pids` called inside `stream_until_tool_call` before disconnect
- `crates/server/tests/runtime_wiring_disconnect_http.rs:386-392`:
  `wait_for_permits` returns all permits
- `crates/server/tests/runtime_wiring_disconnect_http.rs:425-446`:
  assertions preserved
- `crates/server/src/lib.rs:119-120`: SHELL_STARTUP_WRAPPER writes readiness
  before exec
- `crates/server/src/lib.rs:1303-1407`: shell execution guard with
  wait_for_startup before tool_call emission
- `crates/tools/src/shell_tool.rs:278-310`: execute_with_startup select!
  between readiness and execution

## Verification plan

1. Run exact frozen test 3 serial times at c1eea8a with jobs=1/threads=1
2. Check git diff: zero product/test edits, test hash unchanged
3. Run focused regressions: server lib, TOOL startup/process-tree/cancel/bounds
4. Set WEB-009 blocked (not completed) with durable reference/accessibility gaps

## Verification record

- Replacement SHA-256 confirmed:
  `80d8d505a48e018a292b132bde82208bb3afd285ad8b80342365be2f48d28300`.
- Serial run 1: `1 passed; 0 failed` with jobs=1, threads=1.
- Serial run 2: `1 passed; 0 failed` with jobs=1, threads=1.
- Run 3 and focused regressions were not run because the starting free-memory
  reading was only about 1.3 GiB, below the repository's 2 GiB reserve.
- Preliminary status showed no product or test edits by the verifier; only the
  claim ledger and this scratchpad were changed.
- Verdict: verification remains incomplete and WEB-009 remains blocked.
