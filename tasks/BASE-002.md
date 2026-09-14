# BASE-002 - Wire domain contracts and event schema

Status: COMPLETE. Kind: product.
Requirements: opencode.contracts-schema surface.
Ownership locks: crates/contracts/src/events.rs, this card.
Suggested module: `crates/contracts/src/events.rs`.

## User-observable outcome

Typed event envelope for all domain events. Every event has ID, timestamp, source, typed payload. Used by event bus, storage, audit trail.

## Source evidence

- `crates/contracts/src/lib.rs` (SessionId, AgentId, MessageRole, uuid_id macro, compressed style).
- `crates/contracts/Cargo.toml` (chrono, serde, serde_json, thiserror, uuid workspace deps).
- `MEMORY.md` TASK BASE-002 spec.

## Observable contract

- `EventId(String)`: `new()` uuid v4, `as_str()`, Display, Default, transparent serde.
- `EventSource`: System, Session(SessionId), Agent(AgentId), User; Display as `system`, `user`, `session:<id>`, `agent:<id>`.
- `EventEnvelope<T>`: `{ id, timestamp: DateTime<Utc>, source, payload }`; `new(source, payload)` stamps now + fresh id; `with_timestamp` const ctor; Ord by (timestamp, id); manual PartialEq/Eq impls (generic Eq bound).
- `DomainEvent`: Session/Message/Tool/Permission wrappers, snake_case serde.
- `SessionEvent`: Created/ Renamed `{ id, title }`, Archived(SessionId).
- `MessageEvent`: Appended `{ session, seq, role: MessageRole }`, Compacted `{ session, before, after }`.
- `ToolEvent`: Invoked `{ name, session }`, Completed `{ name, duration_ms, success }`.
- `PermissionEvent`: Requested/Granted/Denied `{ intent }`.

## Failure states

- None beyond serde errors on malformed JSON. No validation on title/intent length (deferred to contracts validate_bounded_text at boundary).

## Acceptance criteria

- envelope_serde_roundtrip green.
- domain_event_variants green (10 variants construct, match, serde roundtrip).
- event_id_uniqueness green (1000 distinct).
- source_display green.
- event_ordering green (sort by timestamp).
- `cargo test -p opencode-rk-contracts` green (9 tests); `cargo check --workspace` clean.

## Test-first execution

1. RED: stub file compiled; tests written against missing types (failed to compile = missing behavior).
2. GREEN: full implementation; 9 passed (5 new + 4 pre-existing).
3. Evidence: /home/rashid/.cache/bun-tmp/opencode/base002.log shows `9 passed; 0 failed`.
