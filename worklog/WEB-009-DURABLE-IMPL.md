# WEB-009 durable structured activity implementation

## Claim and boundary

- Task: `WEB-009`; implementation owner `ses_f3c4de578ffelQv59xDXmOs03B`.
- Branch: `lane/WEB-009-durable-impl-20260923`; base `d70e2cc25a311b177f6e79b7466a95aeb17a2c83`.
- Frozen test: `crates/server/tests/web009_durable_persistence_red.rs`, SHA-256 `c0acaf32cbf235452499acbe3eb5f617b7e319f8e6043a7b3cd54720a5b81bb8`.
- Product files: contracts, provider Responses parser, server turn caller, sessions adapter, branch compatibility projection, and storage transaction/migration. No test was edited.

## Source and observed behavior

- Upstream pin: `sources/upstream.lock.json` OpenCode commit `95daf90670b7c039c436c85537da5fbfe2205b41`.
- `tasks/WEB-009.md:19-30` previously admitted that provider reasoning persisted but native tool calls and durable answer references had no producer/persistence contract.
- `crates/providers/src/responses.rs::ResponsesStreamParser::parse_event_impl` previously rejected `response.output_text.annotation.added` as unsupported.
- `crates/server/src/lib.rs::create_turn_stream` streamed tool calls/results but persisted tool output only as display text; it also cleared the accumulated reasoning summary before a second provider round.
- `crates/storage/src/lib.rs::append_message_with_reasoning` atomically persisted only message text plus reasoning summary; `list_assistant_activity` could therefore return no tool lifecycle or references after restart.

## Implemented contract

- The Responses parser admits only bounded `url_citation` annotations with nonempty control-free label/URL fields and an `https://` target. Unknown annotation types and malformed or over-bound values still fail closed.
- The production authenticated router enables structured citation projection. The explicitly legacy unauthenticated test router preserves its frozen fail-closed compatibility contract.
- One turn retains at most 128 durable tool records and 64 references. Tool IDs/names are capped at 128 UTF-8 bytes; reference fields at 1024 bytes; serialized tool/reference payloads are capped at 64 KiB/128 KiB. Tool output remains bounded by the existing loop contract.
- Reasoning summary, completed/failed tool lifecycle, final answer, and references are inserted in one SQLite transaction keyed by the assistant message. Migration adds bounded JSON columns with `[]` defaults for existing databases.
- `/api/sessions/{id}/activity` reloads the structured record after a new `Storage`/`SessionService` instance opens the same disposable database.
- Cross-round reasoning is retained and independently bounded at 8 KiB; it is no longer cleared merely because a tool caused a second provider round.
- Branch sessions preserve prior reasoning behavior. Structured tool/reference writes fail explicitly on the still-unified branch boundary rather than being silently discarded.

## Verification

All commands used `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1` and a Python subprocess timeout.

- Frozen WEB-009 target: 1 passed, 0 failed.
- Existing `session_turn_activity_api`: 2 passed, 0 failed (legacy fail-closed and reasoning reload).
- Provider `responses_stream_semantics`: 6 passed, 0 failed.
- Session `web_activity_branch`: 1 passed, 0 failed.
- Storage library: 122 passed, 0 failed.
- `git diff --check`: passed.
- Frozen test hash re-read from disk and unchanged.

Pre-existing warnings remain in unrelated crates. `cargo fmt --all` was not retained because it rewrote 148 unrelated files and frozen tests; all unrelated formatter output was restored byte-for-byte from HEAD before verification.

## Remaining blockers

This is a candidate patch, not acceptance. WEB-009 remains blocked pending independent verification of browser accessibility (collapsed Reasoning/Tools, meaningful labels, navigable References, restrained live announcements), the third exact disconnect/cancellation pass, and integrated production-web evidence. Branch-session structured activity remains explicitly unavailable until format-1/format-2 activity storage is unified.
