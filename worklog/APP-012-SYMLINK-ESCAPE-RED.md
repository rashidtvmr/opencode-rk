# APP-012-SYMLINK-ESCAPE-RED

## Claim

- Task: `APP-012-SYMLINK-ESCAPE-RED`
- Role: test-author
- Route: `9router/xk/openai/gpt-6-luna`; allowlisted restored pool verified.
- Candidate base: `2f4f7056517d6013bafa8ef2e2ddd5eca84dcda1`
- Ownership: `crates/server/tests/live_file_tool_symlink_escape.rs` only, plus this scratchpad and the task claim.

## Source evidence

- `crates/server/src/lib.rs:1065-1084` parses the live `write` function call into `FileOperation::write` without resolving symlinks.
- `crates/server/src/lib.rs:1567-1579` dispatches `write` through `FileTool::execute_authorized` and feeds its result back to the provider.
- `crates/server/src/lib.rs:1265-1268` builds the broker from the lexical current directory root.
- `crates/tools/src/file_ops.rs:173-199` authorizes before I/O, then calls `execute`; `crates/tools/src/file_ops.rs:250-267` uses `fs::write`, which follows a destination symlink on Unix.
- `crates/security/src/lib.rs:256-282,331-336` checks lexical path containment only; symlink target resolution is absent at candidate base.

## Contract

Unix live `write` receives a lexical path below a unique directory under the current working directory. That path is a symlink to an outside disposable sentinel. The broker/file layer must deny before I/O; the sentinel remains byte-for-byte unchanged; the streamed tool output and provider feedback truthfully report denial; at most two provider rounds occur; neither the sentinel content nor write content leaks into stream or persisted messages.

Ownership is test-local: `TempDir` removes the lexical fixture and outside sentinel after the test; provider thread owns its bounded listener and joins before return. Request/body and read timeouts are bounded.

## RED evidence

- Test source compiles.
- Focused RED command:
  `rtk sh -c 'CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test live_file_tool_symlink_escape -- --test-threads=1 --nocapture'`
- Result: exit 101, expected failure at `live_write_symlink_escape_denied_before_target_io`; outside sentinel changed from `outside-sentinel-exact-bytes\\n` to `symlink-target-must-not-change`. This is genuine `fs::write` symlink-follow behavior at candidate base.
- Frozen test SHA-256: `fe8d24946c8ce98aae095b6a2f92b7f3c19cc32532ae6263f9898d8a5699753c`.
- Status: blocked. Product implementation required; no product files touched.
- Ledger: `blocked` via `tools/completion_claims.py`; verifier must keep test frozen and implement symlink-target denial in broker/file layer.

## Unknowns

- Non-Unix symlink semantics deferred; test is explicitly skipped outside Unix.
