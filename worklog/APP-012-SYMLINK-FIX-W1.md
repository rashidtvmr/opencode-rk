# APP-012-SYMLINK-FIX-W1

- Claim: APP-012-SYMLINK-FIX-W1; session ses_f1e4ee663ffecv8YN3eSpQF1x9.
- Route: 9router/xk/openai/gpt-6-luna; approved restored pool verified.
- Scope: one product file crates/tools/src/file_ops.rs; no test/server/security edits.
- RED frozen hash: fe8d24946c8ce98aae095b6a2f92b7f3c19cc32532ae6263f9898d8a5699753c; RED commit c6091e1.

## Intake
- convergence_gate.py blocked by pre-existing off-plan completed claims; recorded, not modified.
- Contract: reject target symlinks and symlinked parent components before I/O; preserve regular in-root writes/read/list; bounded redacted errors.
- Lifetime: path validation and operation occur within one call; no retained state or spawned task.
- Resource bound: existing operation byte/path limits; no new unbounded storage.
- Security: policy broker remains caller boundary; this lane only hardens filesystem resolution.

## Evidence

## Implementation
- `crates/tools/src/file_ops.rs:251-281`: write path checks all existing components with `symlink_metadata` before parent creation or file open; denies symlink target and symlinked parents with bounded generic error.
- `crates/tools/src/file_ops.rs:258-282`: writes use `OpenOptions`, append/truncate semantics preserved; Unix `O_NOFOLLOW` blocks final-component races where supported.
- `crates/tools/src/file_ops.rs:289-337`: macOS `/var` and `/tmp` aliases normalized to physical paths; avoids denying ordinary temp writes.
- Residual: metadata walk plus ordinary parent creation is not fully race-free against concurrent rename; descriptor-relative `openat2`/`openat` integration would require a shared platform abstraction. Final open uses existing-dependency `O_NOFOLLOW`.

## Verification
- `rtk env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib file_ops -- --test-threads=1`: PASS, 5 passed.
- `rtk env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test live_file_tool_symlink_escape -- --test-threads=1 --nocapture`: PASS, 1 passed; denial HTTP and unchanged sentinel.
- `rtk sha256sum crates/server/tests/live_file_tool_symlink_escape.rs`: `fe8d24946c8ce98aae095b6a2f92b7f3c19cc32532ae6263f9898d8a5699753c` unchanged.
- `rtk rustfmt --edition 2024 --check crates/tools/src/file_ops.rs`: PASS.
- `rtk git diff --check`: PASS.
- Pre-existing unrelated `mcp_spawn::tests::disc111_t04_crash_bounded_retry_no_silent_restart` failure observed during broad tools lib run; focused file_ops target passes.
