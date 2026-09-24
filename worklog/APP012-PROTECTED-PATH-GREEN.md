# APP012-PROTECTED-PATH-GREEN

## Claim and disposition

- Task: APP012-PROTECTED-PATH-GREEN
- Session: ses_f2b74f6feffeOuRDvMvz6TgZAK
- Role: implement protected-path read denial in crates/security/src/lib.rs (polic owner only)
- Branch: lane/APP012-PROTECTED-PATH-GREEN
- Owned file: crates/security/src/lib.rs ONLY (frozen RED at crates/server/tests/app012_protected_path_red.rs must not be edited)
- Status: **BLOCKED** (partial candidate - policy broker half complete, I/O formatting residual owned by file_ops)

## Source evidence (pre-implementation)

- crates/security/src/lib.rs:243-289: authorize_file baseline; read outside project_roots allowed by default
- crates/security/src/lib.rs:542-554: lexical_normalize (component-level but no symlink resolution)
- crates/security/src/lib.rs:555-568: is_secret_path (.env/.env.*, .ssh, .aws, gcloud, credentials)
- crates/security/src/lib.rs:569-583: is_system_path (/etc, /proc, /sys, /private/etc, C:\Windows, C:\Program Files)
- crates/tools/src/file_ops.rs:181-208: execute_authorized wraps Decision::Deny reason as format!("file operation denied: {reason}") at line 199
- crates/server/tests/app012_protected_path_red.rs:40: assert_eq!(result.error.as_deref(), Some("file read denied")) - exact string match

## RED reproduction (before changes)

Command: CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_protected_path_red -- --test-threads=1

Frozen RED SHA-256: ed524ec1a239e8403c76d133e6929683a5a751f758f662b91b0a8d5a2b687d77
Result: 2 passed, 5 failed (matches frozen worklog)

## Changes implemented (crates/security/src/lib.rs only)

1. Added use std::fs to imports (line 27)
2. authorize_file: changed secret-path denial reason to fixed "file read denied"
3. authorize_file: changed system-path read denial reason to fixed "file read denied"
4. authorize_file: added canonical-resolution containment check for FileAction::Read via is_within_readable_root
5. Added is_within_readable_root method: fs::canonicalize on path and each project root, checks containment, fails closed on any resolution error (broken symlink, permission denied, non-existent path)

## Test result (after changes)

Command: CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_protected_path_red -- --test-threads=1

Result: 2 passed, 5 FAILED
- app012_workspace_read_succeeds: PASS (safe workspace read allowed via canonical containment)
- app012_explicit_system_read_compatibility_remains_broker_visible: PASS (system_readable=true allows /etc/hosts)
- app012_absolute_outside_read_is_redacted: FAIL (content no longer leaks, denial occurs, but error = "file operation denied: file read denied" instead of expected "file read denied")
- app012_parent_traversal_read_is_redacted: FAIL (same - traversal denied, content blocked, string mismatch)
- app012_symlinks_inside_workspace_never_follow_outside_or_broken_targets: FAIL (same - symlinks resolved and denied, content blocked, string mismatch)
- app012_wildcard_and_human_allow_cannot_bypass_env_denial: FAIL (same - .env denied by secret check, content blocked, string mismatch)
- app012_workspace_env_read_is_redacted: FAIL (same - .env denied, content blocked, string mismatch)

All 5 failures share the identical root cause: file_ops.rs:199 prepends "file operation denied: " to the broker reason.

## Security lib regression

Command: CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-security --lib
Result: 148 passed, 0 failed

## Broker behavior verification (proving policy correctness)

For each of the 5 failing tests, the broker IS behaving correctly:

1. result.success == false: DENY decision returned (not Allow)
2. result.content == "": no file content returned (denial returns empty content)
3. result.error contains "file read denied" as substring (but with prefix wrapper)
4. canary/secret bytes NOT in result.content: content is empty
5. broker.audit_len() == 1: exactly one denial recorded per attempt
6. file on disk unchanged: no write side effects

The ONLY remaining gap is the exact string match at test line 40, which requires
file_ops.rs:199 to return the broker reason directly without the "file operation denied: " prefix.

## Residual gaps documented

### Hardlink identity (NOT addressed - separate owner boundary)
- contract: crates/security/src/lib.rs has no file-identity inspection API
- is_within_readable_root uses fs::canonicalize which resolves symlinks but
  does NOT detect hardlink aliases (same inode, different name)
- Hardlink denial requires file-identity comparison (inode/device on Unix,
  GetFileInformationByHandle on Windows) - cannot be done in lib.rs without
  platform-specific handle APIs
- Per contract: "Conservative no-read for a multi-link regular file unless a
  platform handle check proves the target is non-protected"
- This must be addressed in file_ops.rs (I/O identity owner)

### TOCTOU (NOT addressed - separate owner boundary)
- authorize_file canonicalizes path; file_ops.rs:228 opens raw path separately
- Race between canonicalization and open can be exploited (path swap)
- Per contract: "Never authorize path A then open path B"
- Requires descriptor binding in file_ops.rs: authorize on a bound no-follow
  descriptor, then read from that descriptor
- Cannot be fixed in security/src/lib.rs alone

### Denial message format (BLOCKED - one-file boundary)
- file_ops.rs:199 format!("file operation denied: {reason}")
- Test expects exact "file read denied" (no prefix)
- Changing file_ops.rs is outside allowed edit scope
- Integration proposal: file_ops.rs owner must change line 199 to pass broker
  reason directly, OR broker must signal that file_ops should use a different
  error-format path for read denials

## Blocked status justification

Per AGENTS.md and task brief: "If frozen 5 failures cannot be fixed solely in
lib.rs, stop BLOCKED with integration proposal."

The 5 failures are NOT fixable solely in lib.rs because:
1. The broker correctly denies all protected/outside/traversal/symlink paths
2. The broker returns the correct reason "file read denied"
3. file_ops.rs:199 wraps it as "file operation denied: file read denied"
4. No lib.rs change can alter file_ops.rs string formatting

Task status set to BLOCKED with residual integration proposal below.

## Integration proposal (for file_ops.rs owner)

File: crates/tools/src/file_ops.rs, line 199
Current: return Ok(FileResult::failure(format!("file operation denied: {reason}")));
Proposed: return Ok(FileResult::failure(reason));

This would allow the broker's redacted "file read denied" reason to pass through
unchanged, satisfying the exact string match at app012_protected_path_red.rs:40.

Alternative (if file_ops owner prefers to keep the prefix for non-read ops):
Add a dedicated deny reason class in the broker Decision that signals "use fixed
read denial message directly" - but this requires a Decision enum variant or
special reason marker, which is a cross-crate API change.

## Remaining unknowns

- None for this lane. The boundary is fully documented in
  APP012-PROTECTED-PATH-CONTRACT.md:219-238.
