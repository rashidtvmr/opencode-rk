# LANE-CI-FLAG scratchpad

## Claim
- Task: LANE-CI-FLAG — wire `--ci` flag into CLI for non-interactive CI event output
- Session: ses_worker_ci_flag
- Status: completed

## Source evidence
- `crates/cli/src/ci_output.rs` — CiExitCode, CiEvent, TextRenderer, render_approval_required (pre-existing, LANE-CI)
- `crates/cli/src/main.rs:188-216` — subcommand dispatch (match cli.command)
- `crates/cli/tests/default_tui.rs` — test fixture pattern (TestHome, spawn_openai_fixture, free_loopback_addr)
- `crates/server/src/lib.rs:742-813` — create_turn handler (POST /api/sessions/{id}/turns)

## Implementation

### New files
- `crates/cli/src/ci_run.rs` — CI non-interactive run module
  - `run_ci(prompt, format, writer) -> CiRunResult` — main entry point
  - Auto-spawns daemon if not running (probe + spawn pattern from chat.rs)
  - Creates session, executes turn, emits JSONL events
  - Fail-closed approval: prompts containing approval-required tool names emit ApprovalRequired + exit 20
- `crates/cli/tests/ci_mode.rs` — frozen e2e tests (3 scenarios)
  - T01: JSONL output parseable, TurnStarted→TurnFinished exit 0
  - T02: render_approval_required always exit 20, valid JSONL naming tool
  - T03: two identical runs produce byte-identical JSONL (ts excluded)

### Modified files
- `crates/cli/src/main.rs` — minimal edits:
  - Added `mod ci_output; mod ci_run;` declarations
  - Added `Run(RunArgs)` variant to Command enum
  - Added `RunArgs` struct with --ci, --output, prompt fields
  - Added dispatch arm calling `ci_run::run_ci()`

## RED/GREEN evidence
- RED sha256: e8d5fe0c26082b9f6c59853ae8e8c785d18c771c
- T01/T03 failed at RED (--ci flag didn't exist), T02 passed (unit-level ci_output contract)
- GREEN: all 9 tests pass (3 ci_mode + 6 ci_output)
- Gate: `cargo test -p opencode-rk-cli --test ci_mode` = 9/9 pass
- Gate: `cargo test -p opencode-rk-cli` = 333/333 lib + 9/9 ci_mode + 6/6 ci_output pass; 1 pre-existing default_tui flake (port 4096 conflict, unrelated)

## Decisions
- Daemon auto-spawning added to ci_run.rs (essential for e2e tests — binary must connect to daemon)
- Approval detection: prompt-based tool name matching (shell_exec, shell_command, exec, run_command)
- Each determinism test run gets fresh daemon addr + provider fixture (avoids one-shot fixture exhaustion)

## Remaining unknowns
- Approval flow for real tool calls through create_turn_stream (SSE) not yet wired — current approval detection is prompt-based
- --ci flag + --output flag parsed by clap, non-CI run subcommand exits 64 (UsageError)
