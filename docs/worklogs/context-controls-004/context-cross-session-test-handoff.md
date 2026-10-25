# Context controls 004 — cross-session integrity test handoff

## Scope

Added `crates/storage/tests/context_cross_session_integrity.rs` only. The
disposable file-backed SQLite fixture creates owner session A and victim
session B, writes a valid typed round in A, then uses a separate SQLite handle
to repoint both A call/output records to B's tool display. The update remains
foreign-key-valid while violating session ownership.

The tests assert the raw bounded history, context bounded history, and tool
preview APIs return typed-history integrity errors rather than exposing the
foreign-session payload. They also retain a pre-corruption valid-round setup
before the SQL fault injection.

## Verification

No Cargo commands, builds, tests, or Git operations were run per task
instructions. Parent/controller must establish the compiling RED and freeze
the artifact. This handoff is not acceptance.

## Artifact hash

SHA-256 of test artifact: `34f2201db9f97da5d61ab91260488ab5a8c417678eec0ca951ed497809353f21`

SHA-256 of this handoff: compute after this final write; the handoff does not
self-embed its own digest.

## Follow-up artifact

Added `crates/storage/tests/context_cross_session_display.rs`. It covers
`display_tool_history` for owner and victim after a foreign display link,
preserves a valid pre-corruption display read, checks unrelated-session
ordinary history isolation, and separately corrupts `tool_rounds.turn_message_id`
to a foreign user and requires both typed readers to return
`StorageError::TypedHistoryIdentity`. No Cargo/build/test/Git commands were run.

## Actual validation

Ran the requested storage validation with the specified native target,
single-job, single-test-thread environment. Cargo compiled successfully and
all requested targets passed: lib 122, compaction checkpoint 1, compaction
tail 2, display follow-up 2, original integrity 2, boundary regression 2,
typed component 10; total 141 passed, 0 failed, 0 ignored. The shell wrapper
then attempted to assign zsh's readonly `status` variable and exited 1 after
Cargo had already returned 0; this wrapper issue is recorded in the log.

Log: `docs/worklogs/context-controls-004/context-cross-session-indexed-green.log`

Test SHA-256: `1a86735090d8f3a15adc107c3775bb2d1c6a4aa7ed5a592ff1d8d821b535188f`
Log SHA-256: `4aba1bbddede0a05ce69a17e4e65d139baaffa2bf0e5562ac72155c98147202a`

## Index-schema follow-up

Added `crates/storage/tests/context_index_integrity.rs`. It verifies that a
same-name but wrong-column `typed_tool_message_idx` is rejected on reopen, and
that a valid index/database reopens with unchanged ordinary history. No Cargo,
build, test, Git, UI, or user-database commands were run; RED/GREEN status is
intentionally not claimed.
