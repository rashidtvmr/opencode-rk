# APP-012-SERVER-READ-WIRING

- Task: APP-012-SERVER-READ-WIRING / session ses_f2e69526effe3GmfFlddGUowI9
- Owned file: crates/server/src/lib.rs only (+ scratchpad + ledger)
- Claim: ledger claim succeeded (in-progress) before edits
- Frozen: crates/server/tests/app012_tool_journey_red.rs sha256 945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50 verified unchanged pre-work

## Source evidence
- lib.rs:1585-1652 Executing stage: enabled_tools gate (permitted), shell one-shot path untouched, else-branch broker.authorize Tool intent -> Allow/Deny/RequireHuman
- lib.rs:1611-1631 drafted: Allow arm calls ToolExecutor::execute_authorized(ToolCall, &state.broker); Deny/RequireHuman produce explicit error strings, no file I/O
- lib.rs:1653 truncate_tool_output bounds every round output (MAX_TOOL_OUTPUT_CHARS 16_384, agent_loop.rs:30,187)
- lib.rs:1686-1738 NextRound: persists [name] output as Tool role, pushes ResponsesItem::FunctionCallOutput, records AssistantToolCall; matches frozen (tool_call/tool_output/assistant_message, round-two function_call_output, [user,tool,assistant] history)
- executor.rs:138-147 execute_authorized routes only name==read to execute_read; ordinary execute has no read arm (109-134 shell/echo/unknown)
- executor.rs:149-203 execute_read: strict path (missing/empty/NUL fail), strict offset/limit usize parse (negative/fraction/null/overflow fail), limit clamped to file_ops::MAX_READ_BYTES, FileOperation::read_with_range, spawn_blocking execute_authorized(op,&broker), timeout; ToolError::IoError mapped to generic "file read failed" (212-218), no path/content leak
- file_ops.rs:12-13 MAX_READ_BYTES 64KiB; 181-208 every op authorizes concrete path, Deny/RequireHuman return fixed failure pre-I/O; 234-257 bounded open/seek/take read, fixed errors ("File not found", "read offset exceeds file length", "file is not valid UTF-8"); 287-294 list_dir fixed errors (no path)
- security/src/lib.rs:215-216,259-270 broker reasons are fixed strings or operator rule-pattern debug; no secret material
- lib.rs:222 bounded_shell_error 1024B; shell wrapper/cancellation untouched

## Audit verdict
- Drafted lib.rs delta (execute -> execute_authorized + &broker) correct and minimal. Retains enabled-tool check + first concrete Tool authorization, adds second concrete FileAction::Read authorization inside executor/file_ops. Deny/RequireHuman never reach I/O at either layer. Success path bounded twice (64KiB read cap, 16KiB output truncate). No unimplemented-success advertising. No shell behavior change.
- No further edits to lib.rs required; working-tree delta is exactly the approved wiring.

## Tests
- PASS: CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1 => 1 passed 0 failed
- PASS: same env cargo test -p opencode-rk-server --lib broker_gate_tests => 1/1; --lib agent_loop => 6/6
- PASS: git diff --check (exit 0); frozen sha256 945236c4...2d3c8c50 unchanged; zero test edits
- rustfmt: owned hunk already canonical; crate-wide --check diffs pre-existing in other modules, no formatting churn applied; no commit/push per shared-worktree order

## Unknowns
- Frozen journey covers success only; deny/malformed/timeout paths source-audited, no frozen assertions (no test edits allowed)
- spawn_blocking timeout drops JoinHandle only; bounded 64KiB read keeps work finite, documented in executor
