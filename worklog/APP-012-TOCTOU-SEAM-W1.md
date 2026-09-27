# APP-012-TOCTOU-SEAM-W1 scratchpad

Claim: APP-012-TOCTOU-SEAM-W1; session 9router-vyce-deepseek-v4-free-combined.

## Source evidence

**File:** `crates/tools/src/file_ops.rs` (commit f386fae)

**Write path (lines 251-296):**
```rust
fn write_file(path: &Path, content: &str, append: bool) -> Result<FileResult, ToolError> {
    if path_has_symlink_component(path)? {
        return Ok(FileResult::failure("write denied: path contains a symlink component"));
    }
    // ... parent creation ...
    let mut file = options.open(path)?;
    file.write_all(content.as_bytes())?;
}
```

**execute function (lines 203-218):**
```rust
pub fn execute(op: FileOperation) -> Result<FileResult, ToolError> {
    match op {
        FileOperation::Read {...} => read_file(&path, offset, limit),
        FileOperation::Write {...} => write_file(&path, &content, append),
        // ...
    }
}
```

**Live server dispatch (lib.rs:1226-1227):**
```rust
if call.name == "write" {
    execute_write(&call.arguments, &state.broker)
}
```

**execute_write (lib.rs:903-922):**
```rust
fn execute_write(arguments: &str, broker: &PermissionBroker) -> String {
    let operation = match file_write_operation(arguments) {...};
    match FileTool::new().execute_authorized(operation, broker) {...}
}
```

**FileTool::execute_authorized (file_ops.rs:158-164):**
```rust
pub fn execute_authorized(&self, op: FileOperation, broker: &PermissionBroker) -> Result<FileResult, ToolError> {
    execute_authorized(op, broker)
}
```

**execute_authorized (file_ops.rs:178-200):**
- Write ops authorize BEFORE any filesystem I/O
- Deny/RequireHuman returns Ok(failure) with ZERO filesystem operations
- Non-write ops pass through to execute()

## Target boundary

Goal: Insert testability seam at pre-open boundary for write operations:
- After symlink check passes but before OpenOptions::open()
- Allow observation/injection for testing without mutating filesystem
- Default implementation must preserve byte-for-byte current behavior

## Observable contract

The seam must:
1. Expose a hook/callback point at the exact pre-open boundary
2. Default callback: no-op (normal write proceeds)
3. Test callback: invoked at pre-open, can observe but NOT mutate filesystem
4. Preserve: broker API, path checks, parent creation, append/truncate, O_NOFOLLOW leaf, errors, bounds

## Remaining unknowns

- Whether to use function pointer, FnOnce, or trait for the hook
- Test scope (in-file unit tests only per requirements)

## Continuation (session ses_f1e0b18b7ffeMqcSNyFxOJ8QV4)

Reclaimed from terminated owner ses_f1e132a8bffegIZTdxTCYrGZ9d (max-steps;
orchestrator-delegated continuation). Treated partial seam as untrusted.

### Why partial tests compiled but were not discovered / dead code
- `PreOpenHook` pub type and `write_file_with_hook` generic were defined but
  `write_file`/`execute` never threaded the seam: fn-pointer default wrapped by
  a generic `P: Fn` made `write_file` call it, however the crate `--lib` test
  binary compiled the file only via `pub mod file_ops`; discovery failure was
  actually a COMPILE FAILURE (E0631: `map_err(ToolError::IoError)` fed a
  `String` into an `io::Error` from; E0382: moved `file_path` in closure).
  Broken lib-test build => zero file_ops tests listed. Errors reported at
  file_ops.rs:290 and :564.

### Final seam design (validation against constraints)
- `execute_with_preopen_hook(op, Option<fn(&Path,bool)->Result<(),ToolError>>)`
  `pub(crate)`; `execute` delegates with `None` => production path byte-for-byte
  unchanged, no global mutable state, no sleeps, no unsafe (crate
  `#![forbid(unsafe_code)]`).
- Hook fires after symlink-component check AND after parent creation,
  immediately before `open_and_write` (the leaf O_NOFOLLOW open) => exact
  pre-open boundary per target. Hook `Err` aborts before open.
- Plain non-capturing fn pointer IS sufficient for later deterministic
  disposable-fixture mutation: hook derives its fixture paths from its `path`
  argument and writes marker/swap files inside the tempdir (no globals).
  Demonstrated by `hook_swap_leaf_to_symlink` +
  `write_file_hook_fixture_mutation_leaf_open_rejects_symlink` (O_NOFOLLOW open
  fails ELOOP/EFTYPE; sentinel untouched). Generic `P: FnOnce` removed;
  `pub type PreOpenHook` removed (unused public API, no security claim).
- No authorization bypass: broker check stays earlier in `execute_authorized`;
  `execute_authorized` untouched. Seam is observation/fixture-injection only;
  NO TOCTOU fix claimed; parent remains blocked.

### Tests (file_ops.rs, in-file, all real behavior on tempdir fixtures)
write_file_symlink_denied | write_file_preopen_hook_invoked |
write_file_append_hook_sees_flag | write_file_no_hook_production_default |
write_file_symlink_denied_before_hook (order: denial preempts hook) |
write_file_hook_cancel_aborts_before_open |
write_file_hook_fixture_mutation_leaf_open_rejects_symlink.
Partial Arc/AtomicBool tests replaced (globals-in-tests + referenced dead
generic); moved-value and String/io::Error mismatch fixed by Option<fn> seam.

### Verification
- `cargo test -p opencode-rk-tools --lib -- --list` => 118 tests incl 12
  file_ops (discovery proven; was 0 while lib-test compile failed).
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib
  file_ops -- --test-threads=1` => 12 passed 0 failed.
- frozen server suites serial, one at a time, all ok:
  live_file_tool_dispatch 1 passed; live_file_tool_allowed_inroot 1 passed;
  live_file_tool_symlink_escape 1 passed.
- frozen SHA256 unchanged:
  dispatch 0366416c4863940221c74aac08deeeba3c9404cd92ff18c9411d2cab5b570829;
  allowed_inroot 6e3841384c7a477ed226f0bf5e28f5b40e06fe683fc9683059e699fd3768aa12;
  symlink_escape fe8d24946c8ce98aae095b6a2f92b7f3c19cc32532ae6263f9898d8a5699753c.
- zero file_ops warnings from `cargo test --lib --no-run` (grep clean).
- diagnostics removed: none present (no dbg!/eprintln!/println!/todo!).
- Pre-existing unrelated fail: mcp_spawn::tests::disc111_t04 fails identically
  at baseline with lane changes stashed; not owned, not fixed here.

### Status
Seam candidate GREEN, behavior-preserving. Parent APP-012 TOCTOU remains OPEN;
no fix claimed. Ledger status: completed (lane scope = seam + tests only).
