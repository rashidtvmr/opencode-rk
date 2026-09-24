# APP012-PROTECTED-PATH-FILEOPS-GREEN

## Claim

- Task: APP012-PROTECTED-PATH-FILEOPS-GREEN
- Session: ses_f2b63b5b6ffe43SaxKJetZxysI
- Branch: lane/APP012-PROTECTED-PATH-FILEOPS
- Worktree: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/lane-app012-protected-path-fileops
- Owned product file: crates/tools/src/file_ops.rs ONLY
- Owned non-product: worklog/APP012-PROTECTED-PATH-FILEOPS-GREEN.md + own ledger row
- Inherited candidate: 7463dbe (broker policy half, crates/security/src/lib.rs)

## Inherited state verified

- HEAD 7463dbe74980b9a0098d245a275d2d03728925e5; branch lane/APP012-PROTECTED-PATH-FILEOPS tracks origin/lane/APP012-PROTECTED-PATH-GREEN.
- Frozen RED crates/server/tests/app012_protected_path_red.rs sha256 ed524ec1a239e8403c76d133e6929683a5a751f758f662b91b0a8d5a2b687d77 (matches RED worklog).
- Frozen APP-012 journey crates/server/tests/app012_tool_journey_red.rs sha256 945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50 (matches contract).
- crates/security/src/lib.rs sha256 abcc41205734a42f2a72c7689e5556194654a8cceee2c353247a51bbb4644e00 (inherited candidate, do not edit).
- crates/tools/src/file_ops.rs sha256 7e84624c89dcb05f45fc69017dfd7279687fbddf8d18d1b405668e8df0ec97f5 (pre-change).

## Observed failure (inherited)

- file_ops.rs:199 `FileResult::failure(format!("file operation denied: {reason}"))` wraps broker fixed reason.
- Broker read denials return reason exactly "file read denied" (lib.rs:261,268,291).
- Frozen test asserts `error.as_deref() == Some("file read denied")` (RED:40) -> 5 exact-string failures; all denials otherwise safe (empty content, one audit, no bytes).

## Compatibility review

- `file operation denied: {reason}` string appears in NO existing test/assertion outside file_ops.rs (repo-wide grep). Only worklogs cite it.
- `execute_authorized` non-read callers: none in tests; server executor only routes "read".
- Conclusion: read denial mapping can change without breaking existing tests; keep non-read wrapper unchanged for compatibility.

## Target boundary (denial wire)

- Denied/read (and List, which maps to FileAction::Read): `success=false`, `content=""`, `error=Some("file read denied")`, fixed, path-free, <=64 bytes, no broker reason/path/bytes exposed.
- Non-read Deny: preserve existing `file operation denied: {reason}` formatting.
- RequireHuman branch unchanged.

## Change implemented

- crates/tools/src/file_ops.rs execute_authorized Deny arm: map `FileAction::Read` to fixed literal `"file read denied"` (no broker reason passthrough, so a permission-rule deny cannot leak its pattern); all other actions keep `format!("file operation denied: {reason}")`.

## Tests / commands (CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1, shared green lane target)

- Frozen protected target: `cargo test -p opencode-rk-server --test app012_protected_path_red -- --test-threads=1` -> 7 passed; 0 failed. Log /tmp/app012_fileops_protected.log
- Security lib: `cargo test -p opencode-rk-security --lib` -> 148 passed; 0 failed. Log /tmp/app012_fileops_security.log
- Frozen APP-012 journey: `cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1` -> 1 passed; 0 failed. Log /tmp/app012_fileops_journey.log
- file_ops module: 5 passed; 0 failed; tool_allow 5/5; tool_audit 5/5.
- `cargo check -p opencode-rk-tools -p opencode-rk-security -p opencode-rk-server` -> 0 errors.
- Pre-existing unrelated failure: `opencode-rk-tools --lib mcp_spawn::tests::disc111_t04_crash_bounded_retry_no_silent_restart` fails `SpawnFailed("No such file or directory")` because fixture hardcodes `/bin/false`, absent on this host (macOS 27; only `/usr/bin/false`). 110 others pass. Reproduced FAIL on unmodified baseline file_ops sha 7e84624c; not caused by this change.

## Hashes

- file_ops.rs pre 7e84624c89dcb05f45fc69017dfd7279687fbddf8d18d1b405668e8df0ec97f5 -> post a98675aac6edb6055e58b32c7aaa4e77a9ac477bebd79f51ec04465336bbd1d1
- security/src/lib.rs abcc41205734a42f2a72c7689e5556194654a8cceee2c353247a51bbb4644e00 (unchanged)
- frozen protected RED ed524ec1a239e8403c76d133e6929683a5a751f758f662b91b0a8d5a2b687d77 (unchanged)
- frozen journey 945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50 (unchanged)
- git diff --check clean; diff +10 -1 in one file.

## lane_gate

- `tools/lane_gate.py` covers storage format-2 lanes only; APP-012 is not in its LANES table. No APP-012 gate row; frozen Cargo target used as the gate.

## Remaining gaps

- Hardlink identity and descriptor-bound TOCTOU remain open (parent APP-012 keeps those). Not in this lane.
- Non-read denial still embeds broker reason; out of scope, unchanged for compatibility.