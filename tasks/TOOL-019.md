# TOOL-019

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-042.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: TOOL-019-T01, TOOL-019-T02, TOOL-019-T03, TOOL-019-T04, TOOL-019-T05.
Ownership locks: crates/tools/src/mcp_payload_filter.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/tools/src/mcp_payload_filter.rs (new module in crate opencode-rk-tools; lib.rs wiring left to integrator).

## User-observable outcome

MCP payload filtering: disabled MCP servers and their tools are excluded from message/tool payloads and execution lookup, with a deterministic snapshot proving the exclusion and no stale enabled state leaking through.

## Source evidence

- crates/tools/src/mcp.rs for existing MCP/extension policy surfaces (McpConfig, McpPolicy, McpTool, McpClient).
- crates/tools/src/ext_perms.rs for existing MCP/extension policy surfaces (grant_perm, MAX_EXT_PERMS).
- crates/tools/src/ext_secure.rs for existing MCP/extension policy surfaces (grant_all, MAX_SECURE_PERMS).
- crates/tools/src/tool_allow.rs:8 for bounded tool allowlist precedent (MAX_TOOL_ALLOW).
- crates/server/src/event_bus.rs for bounded event delivery (ServerEvent, BusError Full/Closed).
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env.

## Observable contract

- `PayloadInput { servers: Vec<ServerFlag { id, enabled }>, tools: Vec<ToolEntry { server_id, name }> }`: caller-supplied view; ids and names only.
- `filter_payload(input) -> FilteredPayload { servers: Vec<String>, tools: Vec<ToolRef>, snapshot_hash: u64 }`: keeps enabled servers plus their tools only; deterministic id/name order; snapshot_hash (FNV-1a over kept pairs) proves exclusion.
- `lookup_tool(filtered, server_id, name) -> Result<ToolRef, PayloadError>`: disabled or unknown server yields NotEnabled/Unknown; never falls back to stale state.
- `verify_snapshot(filtered, expected_hash) -> bool`: recomputes hash; false on any tampering or stale-enabled leak.
- Bounds: servers max 64, tools max 512, id/name 1..=128 chars; over-cap rejected before filtering.
- Deterministic: same input yields byte-identical filtered output plus hash; no I/O, no network, no clock.
- Suggested module boundary: crates/tools/src/mcp_payload_filter.rs owning ServerFlag, ToolEntry, FilteredPayload, PayloadError, filter_payload, lookup_tool, verify_snapshot; shared lib.rs wiring left to integrator.

## Failure states

- Empty server id or tool name: `Err(PayloadError::EmptyField)`; nothing filtered.
- Over-cap servers/tools: `Err(PayloadError::OverCap)`; nothing filtered.
- Lookup against disabled server: `Err(PayloadError::NotEnabled)`; no invocation allowed.
- Lookup of unknown tool: `Err(PayloadError::Unknown)`; no fallback to stale state.
- Stale-enabled leak (disabled tool present in output): snapshot verification fails; treated as filter bypass failure.
- Secret safety: payloads hold ids and names only, zero credentials. Tests use disposable in-memory inputs only.

## Resource bounds

- Pure filter: no Command, no thread, no I/O, no network; O(servers plus tools) time, O(kept) memory within caps.
- No unbounded queue, no unbounded retained output; owner and cancel path defined per crates/tools/src/tool_allow.rs:8 bounded-allowlist precedent.

## Acceptance criteria

- TOOL-019-T01: exclusion happy path: 2 enabled plus 1 disabled server with tools yields filtered output with only enabled tools; snapshot_hash verifies true.
- TOOL-019-T02: lookup gating: lookup of enabled tool succeeds; lookup of disabled-server tool yields NotEnabled; lookup of unknown tool yields Unknown.
- TOOL-019-T03: stale-state proof: flipping a server to disabled then re-filtering drops its tools and changes the hash; verify_snapshot against the old hash returns false.
- TOOL-019-T04: validation: empty id yields EmptyField; 513 tools yields OverCap; both leave no partial output.
- TOOL-019-T05: determinism and isolation: repeated filter byte-identical including hash; Debug plus serialize contain ids and names only; no I/O or network touched.

## Test-first execution

- RED: author tests TOOL-019-T01..T05 against crates/tools/src/mcp_payload_filter.rs; establish compiling RED (fail: no mcp_payload_filter module).
- GREEN: implement minimum native Rust enabled-only filter with hashed snapshot; GREEN, refactor, rerun; negative tests (empty, over-cap, not-enabled, stale-leak).
- Evidence: `cargo test -p opencode-rk-tools mcp_payload_filter`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
