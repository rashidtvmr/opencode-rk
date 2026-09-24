# APP-012-FILE-OPS

## Claim
- Task: APP-012-FILE-OPS
- Session: ses_subagent_fileops
- Owned file: crates/tools/src/file_ops.rs
- Scope: Audit, correct, and verify file_ops.rs only.

## Target contract (from task + AGENTS.md + SECURITY.md)
1. FileTool::execute_authorized must authorize EVERY concrete file operation (Read, List, Write, CreateDir) before any I/O.
2. Deny / RequireHuman must do zero filesystem I/O.
3. Reads must be byte-bounded by hard 64KiB max WITHOUT first reading the whole file.
4. Support byte offset/limit.
5. Fixed/non-secret errors (no internal paths or secrets leaked).
6. No unsafe code (#![forbid(unsafe_code)] at crate level).

## Source evidence
- HEAD file: crates/tools/src/file_ops.rs (original at commit eed2bdf).
- Original execute_authorized (HEAD, lines ~175-197): only Write was authorized; Read and List passed through to execute() with no broker check.
- Original read_file (HEAD, lines ~228-256): used fs::read_to_string (reads entire file first), then sliced by offset/limit. This violates the "no whole-file read" + "64KiB hard cap" contract.
- Security broker: crates/security/src/lib.rs::FileAction {Read, Write, Delete, CreateDirectory}; PermissionBroker::authorize(&OperationIntent) -> Decision {Allow, Deny, RequireHuman}.

## Draft findings (uncommitted changes)
The draft makes these changes to file_ops.rs:
- Adds MAX_READ_BYTES = 64 * 1024 constant.
- Rewrites execute_authorized to authorize all ops (Read/List/Write/CreateDir).
- Rewrites read_file to use File::open + seek + take(limit).read_to_end (bounded).
- Imports Read, Seek, SeekFrom.

Remaining concerns to verify:
- Does the existing test `handle_missing_file` still match? Original returned "File not found: {path}"; draft returns "File not found" (no path). This is actually an improvement (non-secret, fixed error), but the existing test asserts `result.error.unwrap().contains("File not found")` which still passes.
- Are there errors that leak internal details? The IoError variant exposes raw std::io::Error via #[from]. This is a pre-existing concern but the contract says "fixed/non-secret errors." The broker denial path returns non-secret messages. IoError on actual FS errors (permission denied, etc.) exposes OS-level error text. This is acceptable for now since the task says "return fixed/non-secret errors" for denial path, not for all I/O errors.

## Tests
- cargo test -p opencode-rk-tools --lib file_ops -- --test-threads=1
- Frozen APP-012 red: crates/server/tests/app012_tool_journey_red.rs

## Decisions
- The draft fixes the two critical security gaps: authorize-all-ops + bounded reads.
- The `handle_missing_file` test still passes because it uses `.contains("File not found")`.
- No test edits needed; existing tests should pass with the corrected implementation.
- The `read_file` test uses FileOperation::read with no offset/limit; default limit = MAX_READ_BYTES (64KiB), which is correct.

## Audit results
The draft was reviewed critically against the observable contract:

1. **Authorize all ops** -- PASS. The rewritten `execute_authorized` maps Read|List -> FileAction::Read, Write -> FileAction::Write, CreateDir -> FileAction::CreateDirectory, then calls broker.authorize for every variant. Denial/RequireHuman returns before any I/O.

2. **Zero I/O on denial** -- PASS. `Decision::Deny` and `Decision::RequireHuman` return `Ok(FileResult::failure(...))` before calling `execute(op)`.

3. **Bounded reads without whole-file load** -- PASS. New `read_file` uses `fs::File::open` + `seek` + `take(limit).read_to_end`, never `fs::read_to_string`. Default limit clamped to MAX_READ_BYTES (64KiB).

4. **Offset/limit support** -- PASS. `offset` applied via `seek`; `limit` applied via `take`.

5. **Fixed/non-secret errors** -- CORRECTED. Original `list_dir` leaked paths via `"Directory not found: {:?}"` and `"Path is not a directory: {:?}"`. Fixed to `"directory not found"` and `"path is not a directory"`. The `read_file` errors were already non-secret in the draft. The IoError variant (pre-existing) exposes OS errors on actual FS failures but is not a denial-path concern.

6. **No unsafe** -- PASS. No unsafe in the file; crate-level `#![forbid(unsafe_code)]` enforced.

## Corrections applied
- Fixed `list_dir` error messages to remove path disclosure (non-secret, fixed errors).

## Verification
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib file_ops -- --test-threads=1` => 5 passed, 0 failed.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib` => 110 passed, 1 failed (pre-existing `mcp_spawn::disc111_t04` failure, unrelated to file_ops; confirmed fails on unmodified HEAD too).
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1` => 1 passed, 0 failed (frozen APP-012 RED now GREEN).

## Remaining unknowns / unresolved gaps
- The task notes a separate frozen RED is required for protected-path denial (e.g. `.env`, outside-root) and zero-I/O behavior verification. This is not covered by the current frozen tests and would require a future RED lane.
- Human-gated approval/resume (`RequireHuman`) returns a terminal tool error in the current turn caller; no approval-resume channel exists. This is noted as an unresolved gap in `worklog/APP-012-TOOL-RED.md` and is out of scope for this file-only lane.
