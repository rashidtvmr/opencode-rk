# AGENT-003 - Agent message types and in-memory store

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: none.
Dependencies: none.
Ownership locks: crates/agents/src/message.rs, this card.
Suggested module: `crates/agents/src/message.rs`.

## User-observable outcome

Core message data types (Message, MessageRole) and an in-memory ordered
MessageStore used by the agent runtime to track conversation turns before
persistence.

## Source evidence

- `crates/contracts/src/lib.rs` line 161-168: `MessageRole` enum (System, User,
  Assistant, Tool).
- `crates/contracts/src/lib.rs` line 59-62: `MessageId` UUID type.
- `crates/contracts/src/lib.rs` line 110-128: `Timestamp` wrapper with `now()`.
- `crates/sessions/src/lib.rs` line 226-234: role decode shows System=0,
  User=1, Assistant=2, Tool=3 mapping.

## Observable contract

- `MessageRole` enum: System, User, Assistant, Tool. Clone, Copy, Debug, Eq,
  PartialEq, Hash, serde Serialize/Deserialize.
- `Message` struct: id (MessageId), role (MessageRole), content (String),
  timestamp (Timestamp), metadata (HashMap<String, String>). All public.
  Clone, Debug, Eq, PartialEq, serde Serialize/Deserialize.
- `Message::new(role, content) -> Message`: auto-generates id via
  `MessageId::new()`, sets timestamp to `Timestamp::now()`, empty metadata.
- `Message::with_metadata(role, content, metadata) -> Message`: same but
  accepts caller-provided metadata.
- `MessageStore` struct: messages Vec<Message> backing store with an index map
  from id-string to position for O(1) lookup.
- `MessageStore::new() -> MessageStore`: empty store.
- `MessageStore::append(msg)`: insert message; if id already exists, replace
  in place (idempotent upsert by id).
- `MessageStore::get(id: MessageId) -> Option<&Message>`: lookup by id.
- `MessageStore::find_by_role(role: MessageRole) -> Vec<&Message>`: all
  messages matching role, in insertion order.
- `MessageStore::recent(n: usize) -> Vec<&Message>`: last n messages in
  insertion order; if n exceeds length returns all; if n is 0 returns empty.
- `MessageStore::len() -> usize` and `MessageStore::is_empty() -> bool`.

## Acceptance criteria

- AGENT-003-T01 append_and_get: append a message, get it back by id, content
  and role match.
- AGENT-003-T02 find_by_role: append mixed-role messages, find_by_role returns
  only matching role in insertion order; empty for absent role.
- AGENT-003-T03 recent_returns_last_n: append 5 messages, recent(3) returns
  last 3 in order; recent(10) returns all 5; recent(0) returns none.
- AGENT-003-T04 empty_store: new store is_empty, len 0, recent(5) empty,
  get returns None.
- AGENT-003-T05 message_timestamp: timestamp is set to approximately "now" at
  construction.
- `cargo test -p opencode-rk-agents` green.
- `cargo check --workspace` clean.

## Test-first execution

1. RED: tests written inline in message.rs with stub returning panic.
2. GREEN: full implementation; 5 tests pass.

## Evidence

- `cargo test -p opencode-rk-agents`:
  `test result: ok. 5 passed; 0 failed; 0 ignored.`
- `cargo check --workspace`: Finished dev profile, EXIT 0.
