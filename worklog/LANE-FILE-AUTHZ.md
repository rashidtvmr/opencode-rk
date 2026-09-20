# LANE-FILE-AUTHZ scratchpad

claim: LANE-FILE-AUTHZ, session ses_f423cd0dcffeKV0Mtlfh4TMwp2
owned: crates/tools/src/file_ops.rs only

source evidence:
- crates/tools/src/file_ops.rs:149-151 `FileTool::execute` -> `execute(op)`, no broker
- crates/tools/src/file_ops.rs:161-176 `execute(op)` Write arm -> `write_file` direct
- crates/tools/src/file_ops.rs:209-233 `write_file` does create_dir_all + fs::write/OpenOptions, zero authorize
- crates/security/src/lib.rs:205 `PermissionBroker::authorize(&OperationIntent) -> Decision`
- crates/security/src/lib.rs:33-44 `FileAction::Write`, `OperationIntent::File{action,path}`
- crates/tools/src/mcp_spawn.rs:379-389 pattern: broker.authorize Process intent, Deny/RequireHuman -> Err before spawn
- crates/tools/src/shell_bounds.rs:453 `gated_write_file(root,path,content,permitted)` — bool-gated, not broker-typed; not reused (would widen owned surface + change semantics)
- docs/SECURITY.md §4: denied write leaves no file (absence of side effects)

observed: any caller of execute(Write) gets unconditional fs mutation.
target boundary: add `execute_authorized(op, broker)` + `FileTool::execute_authorized`; Write arm authorizes File{Write,path} first, Deny/RequireHuman -> Ok(failure) with zero I/O. Read/List/CreateDir/legacy execute untouched (frozen tests use execute directly).

tests: frozen = existing 5 in-file tests (read_file, write_file, list_dir, create_dir, handle_missing_file). No test edits. VERIFY cmd per task.
decisions: failure-result (not Err) for denial — matches read-missing convention (Ok(failure)), keeps callers total; zero I/O before verdict.
evidence:
- VERIFY 2026-09-20: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 110 cargo test -p opencode-rk-tools --lib file_ops` => 5 passed 0 failed (101 filtered).
- impl already in tree via ancestor 36ae2db (execute_authorized + FileTool::execute_authorized, zero-I/O deny); this session restored it from stash@{1} after tree moved, re-ran VERIFY, zero test edits.
- deny-no-file probe: attempted via external probe crate but /tmp quota blocked link; relied on code inspection (deny returns before execute/write_file, no fs calls) + frozen suite.
remaining: commit lane files (scratchpad + claims) + push. No product-code commit needed (36ae2db ancestor of HEAD ddd7156).
