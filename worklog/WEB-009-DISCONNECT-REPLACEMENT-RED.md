# WEB-009 disconnect replacement RED

## Claim

- Task: WEB-009
- Session: `ses_f3307a451ffe0GjbJwCcXIdkxB`
- Scope: replacement test only, `crates/server/tests/runtime_wiring_disconnect_http.rs`; ledger row and this scratchpad.
- Parent WEB-009 remains IN PROGRESS / NOT ACCEPTED. This lane covers only the browser-disconnect test sub-contract.

## Authorization and historical TDD deviation

The original frozen test hash was `95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8`. Independent review `worklog/WEB-009-DISCONNECT-CONTRACT-REVIEW.md` proved an ordering race: startup readiness precedes fixture PID publication, while the client disconnects immediately after `tool_call`. User explicitly authorized replacing that disputed frozen hash.

Product implementation already exists at candidate `dd7670d` / product commit `732c046`, so normal RED-before-implementation ordering is historical and cannot be reproduced on the candidate tree. This lane records the deviation and establishes RED against pre-implementation base `9ded2b9` using a disposable temporary worktree with this replacement test copied in, without modifying or committing that base.

## Contract

After observing the real HTTP `tool_call`, the fixture parent and descendant PIDs must be published before the client shuts down the HTTP stream. The test then preserves all original assertions: `tool_call` observed, shared permit returned, provider socket closed, parent and descendant dead, delayed sentinel absent, and no durable tool or assistant success. Synchronization uses the existing bounded `wait_for_pids` polling only; no arbitrary delay added.

## Source evidence

- `crates/server/tests/runtime_wiring_disconnect_http.rs:258-303`: client reads `tool_call`; replacement waits for both fixture PIDs before `Shutdown::Both`.
- `crates/server/tests/runtime_wiring_disconnect_http.rs:311-320`: bounded four-second PID publication polling.
- `crates/server/tests/runtime_wiring_disconnect_http.rs:381-387`: shared permit return assertion.
- `crates/server/tests/runtime_wiring_disconnect_http.rs:421-438`: provider closure, process cleanup, sentinel, and durable-role assertions.
- `crates/server/src/lib.rs:119-120`: startup readiness is written before the opaque fixture command executes.
- `worklog/WEB-009-DISCONNECT-CONTRACT-REVIEW.md:135-227`: exact race and why product code cannot synchronize test-specific PID files.

## Verification record

- Original frozen test hash: `95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8`.
- Replacement hash, stable after final edit: `80d8d505a48e018a292b132bde82208bb3afd285ad8b80342365be2f48d28300`.
- RED: disposable detached worktree `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/web009-red.1eMDXF` at `9ded2b9`; replacement test copied into that worktree; `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test runtime_wiring_disconnect_http -- --test-threads=1`; compiled, failed 1 test at `runtime_wiring_disconnect_http.rs:353:5: tool parent survived browser disconnect`.
- GREEN run 1: same command at candidate `dd7670d` / product `732c046`; `1 passed, 0 failed`; 0.07s.
- GREEN run 2: same command; `1 passed, 0 failed`; 0.09s.
- GREEN run 3: same command; `1 passed, 0 failed`; 0.07s.
- All runs serialized with `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`. Existing bounded polling only; no added sleeps. No product edits.

## Remaining boundary

WEB-009 durable tool-call/reference persistence and accessibility gaps remain unresolved. This replacement does not claim parent completion.
