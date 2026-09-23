# TOOL-012 startup-readiness implementation

## Claim

- Task: TOOL-012
- Session: `ses_f338c638dffeNOgL4RCjogVLIi`
- Candidate HEAD at claim: `cb7b8f6c77a60e32228be829bd034e104bdf6f35`
- Owned files: `crates/tools/src/shell_tool.rs`, this scratchpad, TOOL-012 ledger row
- Frozen startup test SHA-256: `4dae909cd1db9c724a42ab731ef5d0074f8a423561392a6b321cb9ecde442691`

## Source evidence

- `crates/tools/src/shell_tool.rs`: existing `ShellTool::execute` owns allowlist, broker, spawn, bounded output drain, timeout, wait, cancellation and process-group cleanup.
- `crates/tools/tests/shell_tool_startup.rs`: frozen caller supplies a readiness path and bounded `mpsc::Sender<u32>`; valid readiness content must equal the spawned wrapper PID and produce exactly one event.

## Implementation contract

- Preserve `execute` behavior through a private shared `run(config, Option<(PathBuf, Sender<u32>)>)`.
- Startup readiness is caller-owned path data, bounded to 20 bytes, UTF-8 decimal digits, positive, and equal to the child PID.
- Readiness polling remains local and owned by the execution future. No detached task. Event delivery uses one nonblocking `try_send`; full or closed channels do not fail execution.
- Normal output excludes probe content. Invalid/missing probe, spawn failure, and execution completion before valid readiness produce no event unless the final bounded read is valid.

## Verification

- Startup frozen GREEN: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test shell_tool_startup -- --test-threads=1` -> 4 passed.
- Process-tree GREEN: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test shell_tool_process_tree -- --test-threads=1` -> 1 passed.
- Cancellation GREEN: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test shell_tool_cancel -- --test-threads=1` -> 1 passed.
- Bounds GREEN: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test shell_tool_bounds -- --test-threads=1` -> 2 passed.
- Shell unit GREEN: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib shell_tool -- --test-threads=1` -> 9 passed, 102 filtered.
- Frozen test hash rechecked: `4dae909cd1db9c724a42ab731ef5d0074f8a423561392a6b321cb9ecde442691`.
- `git diff --check` -> pass.
