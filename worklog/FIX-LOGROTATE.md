# FIX-LOGROTATE scratchpad

claim: FIX-LOGROTATE, session ses_fix_logrot, HEAD 62f7eb1
owned: crates/cli/src/diagnostics.rs only

## Source evidence
- crates/cli/src/diagnostics.rs:14-16 caps MAX_EXPORT_FIELDS=128, MAX_EXPORT_VALUE_BYTES=8KiB, MAX_EXPORT_TRANSCRIPTS=256
- crates/cli/src/diagnostics.rs:281-316 redacted_export enforces field-count, value-bytes (fields only), transcript-count; transcript ENTRY bytes NOT capped (gap)
- crates/cli/src/main.rs:210-212 tracing to stderr (operator boundary anchor; NOT editing)
- crates/cli/src/session_export.rs:1-8 separate HEAD-002 export, caller-owned, NOT owned (do not touch)
- docs/SECURITY.md: no secret logging; docs/TDD.md: frozen tests immutable

## Target boundary
- No file-rotation daemon in-process. Operator owns rotation: app logs to stderr only; collector (systemd/journald/logrotate/file) rotates.
- Caps enforced on EVERY redacted_export path incl. transcript entry bytes.

## Tests
- Frozen: in-file mod tests (must not edit). Verify: CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-cli --bin oc2 diagnostics

## Decisions
- Add LOG_ROTATION_CONTRACT const + module docs (operator boundary).
- Truncate each transcript entry to MAX_EXPORT_VALUE_BYTES, set truncated flag.

## Unknowns
- None. Verified.

## Verify (2026-09-20, HEAD 62f7eb1 + lane edit)
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-cli --bin oc2 diagnostics`
  tail: 7 passed (disabled_workers, precedence, export_redact, export_transcripts, last_valid x2, sandbox_name); 0 failed; 295 filtered.
- Caps self-check (disposable /tmp/opencode/diag_check, copy of owned file + main harness): 300x9KiB transcripts -> 256 entries, each <=8KiB, truncated=true; 200 fields -> 128, truncated=true. PASS.
- Zero test edits. No commit/push per lane bounds. Other lanes' dirty files (ci_run, native_shell, rules_globs, storage/lib, install-oc2.sh, claims.json races) untouched.
- Status: completed.
