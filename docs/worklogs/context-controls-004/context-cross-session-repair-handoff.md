# Context cross-session typed-history repair

Implemented the storage repair from parent candidate `validate_typed_ownership`.

- `crates/storage/src/session_context.rs`: split ownership validation into four
  independent, session-seeded `EXISTS ... LIMIT 1` checks (owner rounds,
  incoming turn links, owner record links, and incoming display links). Missing
  turn/display rows and non-tool display roles fail closed; no global `OR`
  predicate or payload read is used.
- `crates/storage/src/lib.rs`: `display_tool_history` now validates before
  selecting payloads; typed-history migration adds the additive
  `typed_tool_message_idx(message_id,round_id)` index without changing table
  identity SQL.

No Cargo/build/test/Git/DB/UI command was run per parent instruction. Parent
test owner must add the EXPLAIN proof and public display regression.

Final candidate diagnosis differs materially: validation and hydration now
share one deferred read snapshot; the new index is checked for table,
non-unique status, and exact column order before and after migration, rejecting
malformed same-name indexes. The incoming display query also leaves the inner
round lookup unhinted so SQLite can use the round-id primary key directly,
avoiding the prior session-index per-record scan.
