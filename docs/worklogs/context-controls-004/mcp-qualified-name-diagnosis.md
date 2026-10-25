# MCP Eight-Hex Qualified-Name Collision Diagnosis

## Context

Base SHA: `e3069eee` (committedbasee3069eee)
Scope: read-only diagnosis of the `stable_name` 8-hex truncated-digest collision
concern in `crates/server/src/mcp_context.rs`. No source/test edits.

## Observed Constructible Behavior (not hypothetical)

File: `crates/server/src/mcp_context.rs:23-28`

```rust
fn stable_name(server: &str, tool: &str) -> String {
    let digest = blake3::hash(format!("{server}\0{tool}").as_bytes()).to_hex().to_string();
    let sanitized: String = tool.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    let keep = MAX_TOOL_NAME_BYTES.saturating_sub(13 + 1 + digest.len().min(8));
    format!("mcp_{}_{}", &digest[..8], &sanitized.chars().take(keep).collect::<String>())
}
```

- `MAX_TOOL_NAME_BYTES = 64` (crates/server/src/mcp_context.rs:9)
- Digest truncated to exactly 8 hex chars = **32 bits** of entropy.
- Name shape: `mcp_` (4) + `_` (1) + 8 hex (8) + `_` (1) + sanitized_tool (up to 50) = 64 max.

## Concrete Collision Path

Blake3 full digest is 256 bits, truncated to 8 hex (32 bits). Birthday bound:
2^16 distinct (server, tool) pairs give ~50% collision probability. With many MCP
servers each advertising dozens of tools, the practical surface is reachable.

### Permission-binding consequence

`permission_rules::deny(tool=&def.name, ...)` (mcp_context.rs:48,68) and
`permission_rules::Rule.tool` validation (permission_rules.rs:29-31) accept
`mcp_*` names of length 5..=64. A saved `allow`/`deny` rule keyed on a
colliding 8-hex prefix applies to BOTH `(serverA, toolA)` and `(serverB, toolB)`
that happen to share the truncated digest, silently binding one tool's trust
to another. The `denied` fingerprint path keys intent by `def.name` only, so the
two distinct tools resolve to identical permission keys.

### Execution consequence

`execute` (mcp_context.rs:42-76) re-resolves `defs` and finds `|d| d.name == call_name`.
If two distinct tools collide to the same `stable_name`, the `.find()` is
first-match over iteration order of `connected_names` (insertion order,
non-deterministic across registry restarts). A caller invoking the colliding
name may execute the OTHER tool's backend with different arguments, violating
the exact-tool binding asserted by
`crates/security/tests/mcp_human_gate.rs` and
`crates/server/tests/mcp_permission_rules.rs:17`
(`saved_mcp_grants_bind_exact_tool_arguments_context_and_session`).

## Existing Exact-Match Tests (baseline, pre-fix shape)

- `crates/server/tests/mcp_permission_rules.rs` — uses `mcp_01234567_exact_probe`
  and `mcp_89abcdef_other_probe` (8-hex prefixes), asserts rules bind to exact
  tool/arguments/session/context. These fixtures would need longer prefixes
  after a fix.
- `crates/security/tests/mcp_human_gate.rs` — uses `mcp_01234567_probe`.

## Recommended Minimal Regression Test (concrete, constructible)

Add to a NEW test file `crates/server/tests/mcp_name_uniqueness.rs` (outside
this no-edit diagnosis):

1. Register two local MCP fixtures whose `(server, tool)` pairs are crafted to
   collide under the current 8-hex truncation (e.g. server A = `"alpha"`,
   tool = `"probe"`; server B with server/tool chosen so
   `blake3("{A}\0{probe}").to_hex()[..8] == blake3("{B}\0{x}").to_hex()[..8]`).
2. Assert `stable_name` (exposed for test via `pub(crate)` or a thin shim)
   returns DISTINCT names for the two pairs, OR that `effective_tools`
   produces no duplicate `name` fields.
3. Assert that a persisted permission rule for pair 1 does NOT authorize
   execution of pair 2 (i.e. `permission_rules::decide` returns `None` for
   the colliding short name after the rule is written against pair 1's full
   name).

Because `stable_name` is private, the test must either:
- make `stable_name` `pub(crate)` and `pub` the module path, or
- drive via `effective_tools` against real fixtures and assert
  `!defs.iter().any(|d| defs.iter().filter(|e| e.name == d.name).count() > 1)`.

## Recommended Fix Direction (approval scope only, no edits here)

Per the constraint: "longer digest within 64 ASCII + explicit duplicate
rejection; avoid breaking old stored permission behavior silently".

1. Extend `stable_name` to emit a digest long enough to push collision
   probability negligible (e.g. 16 hex = 64-bit digest), while keeping total
   name within `MAX_TOOL_NAME_BYTES=64`. With `mcp_` (4) + `_` (1) + 16 hex
   + `_` (1) = 22 char overhead, leaving 42 chars for sanitized tool name.
2. Add explicit duplicate detection in `effective_tools`: if two resolved
   `(server, tool)` pairs yield the SAME `stable_name`, reject the duplicate
   rather than silently aliasing two distinct backends under one name. This
   preserves the `permission_rules` binding (which keys on exact tool string)
   without silently remapping an old stored rule.
3. Stored permission rules remain bound to `mcp_*` strings. After extending
   the digest, old 8-hex rules would not match new longer names -- but no
   silent remap is required because the fix rejects duplicates rather than
   reusing truncated aliases. Existing tests using fixed 8-hex fixtures must
   update their fixture prefixes to the new length (test-only mechanical
   change, not product behavior change).

## Approval Scope

- Two independent reviewers (P0/P1 collision/permission-binding concern).
- One security evaluator (permission broker path change,
  `permission_rules.rs` interaction).
- Test owner for mechanical fixture-prefix updates in
  `mcp_permission_rules.rs`, `mcp_human_gate.rs`, `mcp_provider_execution.rs`.
- No change to stored-rules format or persisted permission files.
