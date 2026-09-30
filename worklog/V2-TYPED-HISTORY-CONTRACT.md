# V2 typed history HTTP/restart contract

- Base SHA: `0088fbf2ea65251f344915c93bd8679c0c467320`.
- Test owner scope: `crates/storage/tests/typed_history_http_restart.rs` only,
  plus this worklog.
- Fixture: installed `OC2_TEST_BINARY`, loopback-only provider, cleared
  environment, disposable `HOME`/project/database, bounded HTTP reads and
  owned child/provider guards.
- Positive flow exercises a real provider function call, brokered write,
  typed call/output persistence, provider continuation, second user turn,
  daemon kill/restart, and replay of typed history in the next provider
  request. It does not insert typed rows as positive evidence.
- Fault flow requires transactional absence of typed rows, ordinary tool
  messages, file side effects, and provider continuation after a deterministic
  write fault. The accepted contract is late HTTP failure with headers already
  sent represented by terminal NDJSON `internal_error`, per approved historical
  correction `61d473ab`; write success output is intentionally only
  `write success` and must not expose path/content (security-905).
- Upstream contract evidence inspected read-only at
  `/Users/mymac/Projects/opencode-upstream-reference` pinned `95daf906`:
  message/provider/tool boundaries under `packages/protocol/src/groups/message.ts`,
  `packages/core/src/session/runner/to-llm-message.ts`, and the provider
  request/response path. Existing installed fixture helpers supplied the exact
  SSE function-call shape and daemon route.
- Repair evidence: provider accept is nonblocking with stop/deadline polling;
  `ProviderGuard` wakes and joins the thread; the positive fixture supplies all
  five provider responses needed through restart; restart waits for a changed
  descriptor PID; the fault path installs a deterministic SQLite trigger before
  the provider call; database discovery is bounded recursive inspection under
  the disposable home rather than guessed fixed paths.

## RED evidence

Command:

```text
rtk env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 OC2_TEST_BINARY=<fixture> /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-storage --test typed_history_http_restart
```

Compile command/result:

```text
rtk env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 OC2_TEST_BINARY=<fixture> /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-storage --test typed_history_http_restart --no-run
=> Finished test profile successfully; existing storage warnings only.
```

Runtime RED on the supplied installed fixture:

```text
rtk env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 OC2_TEST_BINARY=<fixture> /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-storage --test typed_history_http_restart
=> timed out at 120s while the fault fixture awaited its provider request;
   earlier bounded run also observed the baseline fixture's provider/typed
   history failure (`QueryReturnedNoRows` in typed_tool_history_http.rs).
```

Latest source-only fixture correction (not yet Cargo-validated per slot
constraint): canonicalizes the disposable project before broker arguments and
daemon startup; asserts the successful file content; decodes bounded chunked
HTTP bodies before parsing terminal NDJSON; and validates provider `input`
arrays for exact call/output ordering, arguments, non-disclosing output, and
per-call uniqueness across second turn/restart. The prior RED errors were
positive `target.is_file()` failure and fault `terminal NDJSON record:
expected value`; those assertions are now corrected without changing the
product contract.

Focused validation at preservation base `0759e01` (fixture `fc2d201`):

```text
rtk env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 OC2_TEST_BINARY=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-fc2d201-http-odzd04nk/bin/oc2 /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-storage --test typed_history_http_restart --no-run
=> PASS; test executable produced.

rtk env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 OC2_TEST_BINARY=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-fc2d201-http-odzd04nk/bin/oc2 /usr/bin/arch -arm64 cargo test --offline --locked -p opencode-rk-storage --test typed_history_http_restart
=> RED, 2 tests failed in 0.14s.
```

Exact first assertion failures:

1. Fault test: terminal error `code` was `bad_gateway`, expected the approved
   late-stream `internal_error`.
2. Positive test: `typed_tool_records` query returned `[]`, expected the exact
   call/output pair for `call-1`.

These are clean product RED failures after the canonical project reached the
live request path; no provider timeout, empty request, or cleanup deadlock
occurred. The test source SHA-256 is:
`1ca3559f49e7f85cb2f7d483372a7ed8104665d43577888a44fc55445d4ab928`.

Source-only fixture arithmetic correction after that runtime RED: the expected
byte length for exact output `write success` is derived as
`"write success".len() as i64` (13), rather than the erroneous literal 12.
No runtime rerun was performed because the observed clean product RED was the
empty typed-row result, before that expected output-row tuple could match.

This target is independent of the existing frozen `typed_tool_history_http.rs`
test.
