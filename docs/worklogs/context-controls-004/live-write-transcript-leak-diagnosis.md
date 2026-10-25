# `live_write_tool_inroot_is_allowed_and_persisted` diagnosis

## Finding

This is an actual public-projection regression, not an observer error. The
write content is admitted and durably retained as the typed provider call
argument, but `GET /api/sessions/{id}/messages` reconstructs a public `parts`
entry in `crates/server/src/lib.rs:list_messages` (around lines 1075-1079)
with `arguments: serde_json::from_str(&tool.payload)`. For `write`,
`tool.payload` is the original JSON containing `content`, so the GET response
contains `ALLOWED_CONTENT`. The live NDJSON path is already safe: the
`FunctionCall` projection at lines 2552-2557 emits `{}` for `write`, and the
fixture's public-event assertion passes. `typed_tool_records` must remain raw:
`crates/storage/src/lib.rs:admit_tool_round` stores exact arguments for
provider replay, while `history_item_to_responses_item` correctly restores
them to the provider.

## Minimal repair

In `list_messages`, preserve call metadata but project write arguments as an
empty object (the same `{}` policy used by the stream), e.g. choose
`json!({})` when `tool.name == "write"`; leave all other typed arguments and
durable storage unchanged. Add/retain a GET-message assertion that write
payload is absent while success metadata remains present.

Pinned-upstream authority cannot supersede this explicit current contract:
public transcript metadata is secret-safe, and the existing semantic test must
not be weakened.
