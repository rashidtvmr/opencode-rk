# APP-012-LIVE-FILE-TOOL-ALLOW-RED

## Claim and scope
- Claim held by `ses_f1e6bf0f1ffeM9hWxOz8POJK6v`; assigned route xkiro/openai/gpt-6-luna is user-approved.
- Owned test: `crates/server/tests/live_file_tool_allowed.rs`; scratchpad and own ledger row only. No product implementation.
- Base: `ecec045471641eb50b8e59bed00003f1af943d4d`; branch `red/APP-012-LIVE-FILE-TOOL-ALLOW-W2`.

## Evidence / contract
- Copied fixture structure from read-only `crates/server/tests/live_file_tool_dispatch.rs` at `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-app012-live-file-tool-w2/`.
- Live HTTP path is `POST /api/sessions/{id}/turns/stream`; controlled provider fixture supplies two Responses SSE rounds. Enable exactly `OPENCODE_RK_TURN_TOOLS=write`.
- Expected success: file content is written under disposable `tempdir`; streamed `tool_output`, provider round-two `function_call_output`, and durable messages truthfully report success; public events don't disclose file content. Generic unknown/denied tool output is RED.
- Fixture bounds provider request to 256 KiB, response body to 64 KiB, stream read timeout 20s, and exactly two provider rounds. No user DB, network provider, or secret file used; fake API key and loopback provider only.
- Convergence gate was run before work; it is blocked by repository-wide backlog/off-plan ledger findings (total 84), recorded as pre-existing blocker.

## RED / freeze
- Frozen test SHA256: `c914953e4566b21eaa0960c2b249039adfb0c8ae68c031a67082c9251e230910`.
- RED command: `rtk python3 -c "import os,subprocess; e=os.environ|{'CARGO_BUILD_JOBS':'1','RUST_TEST_THREADS':'1'}; r=subprocess.run(['cargo','test','-p','opencode-rk-server','--test','live_file_tool_allowed','--','--test-threads=1'],env=e,timeout=180);raise SystemExit(r.returncode)`.
- Result: compiled; 1 test executed and failed as expected at line 291 because live route returned `Unknown tool: write`; exit 101. No product modifications.
- Intake: no task-card file exists at the anticipated `tasks/task-cards/APP-012-LIVE-FILE-TOOL-ALLOW-RED.md`; execution scope and exact observable assertions were supplied by the delegator. Gate failure precludes claiming parent readiness; this lane is RED-only.
- Resource: one Cargo test build/test process with `CARGO_BUILD_JOBS=1`, `RUST_TEST_THREADS=1`; no parallel builds or browser/database activity. Duration ~65s compile + <1s test. Memory telemetry unavailable.
- No implementation/product changes permitted in this RED lane.
