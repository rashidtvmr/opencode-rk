# V2 Native Fixture Maintenance — crates/cli/tests/native_daemon_flow.rs

Package/gate ID: V2-NATIVE-FIXTURE-MAINTENANCE
Role: independent test-owner mechanical repair (no semantic assertion changes)
Worktree: /Users/mymac/Projects/opencode-rk-v2-native-fixture-maintenance
Branch: v2/native-fixture-maintenance
Base SHA: 7fe4656fa46206db3cddd5fd4442df449a4e0fca
Candidate SHA: (recorded at commit below)

## Granted paths
- crates/cli/tests/native_daemon_flow.rs (edit)
- worklog/V2-NATIVE-FIXTURE-MAINTENANCE.md (write)

No product source, manifest, Cargo.lock, or canonical branch edits. No #[ignore]/skip/assertion weakening.

## Original compiler RED (from main's canonical workspace run)
crates/cli/tests/native_daemon_flow.rs:190 E0277
`expected_len = Some(header_end + 4 + content_length?);`
The `?` operator was used inside the `thread::spawn` closure that returns `()`, on an
`Option<usize>` produced by `.transpose().expect("valid content length")`.

Original file hash: 5c513cedf90bbee078f29cb0252c654888f09ca8a8ca97b14d85855a2d7c7a1d
Corrected file hash: 2eef639c9966c04f962836b1892bafbe2ca43ae8ec4ae8ecf692e4041c14507d

## Mechanical repairs (no asserted behavior changed)
1. Framing body-read mechanics (compiler fix): replaced `content_length?` with
   `.expect("provider request content length")` on the already-`transpose()`d Option, and
   dropped the `?`. Preserves complete-body semantics: a missing content-length header now
   fails loudly (same as the previous `?`-on-None intent) and `header_end + 4 + content_length`
   math is unchanged.
2. Bounded owned fixture accept cleanup: `spawn_openai_fixture` no longer blocks forever on
   `listener.accept()`. Listener is set nonblocking with a 5s accept deadline; on timeout the
   task returns and releases the port. This preserves all asserted daemon/auth/native behavior
   for tests that DO submit a prompt, and prevents `provider_task.join()` hanging on tests that
   never submit one (T01/T02/T03-style fixtures). Stream is set back to blocking before the read
   loop; read timeout (10s) unchanged.
3. Mechanical unused-binding fixes: `_provider_base` (3 call sites) and `_stdout` in T01.

## Verification
Command (focused, no workspace; exclusive heavy slot, jobs=2 threads=1):
`rtk /usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 DYLD_LIBRARY_PATH=.../crates/opentui-bridge/native/lib/aarch64-apple-darwin TUI015_NATIVE_LIB_DIR=.../crates/opentui-bridge/native/lib/aarch64-apple-darwin cargo test --offline --locked -p opencode-rk-cli --test native_daemon_flow`

Result: test target COMPILES and RUNS (compiler RED resolved). 0 passed; 4 failed.

Remaining observed PRODUCT failure (NOT maintenance, NOT fixed here):
All 4 tests fail with the binary emitting on stderr:
`error: stdin and stdout are redirected; interactive TUI requires a terminal and raw mode is refused; pipe a prompt to the headless command or rerun under a TTY`
- native_daemon_spawns_when_none_running (line 269 assertion)
- native_no_tty_still_takes_native_path (line 309 assertion)
- status_frame_carries_live_daemon_values (line 418 assertion)
- tui_attaches_to_running_serve_daemon (line 108 wait_for timeout)

This is a product-side TTY/native-path regression: `--native --once` with piped I/O takes the
headless-refusal path instead of the native TUI path. Out of scope for this test-owner package.

## Status
PREVERIFIED (compiler) / remaining RED is a product failure requiring a separate package.
Does NOT constitute G1/G5 acceptance.