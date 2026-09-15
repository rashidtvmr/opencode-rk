# TOOL-017

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-042.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: TOOL-017-T01, TOOL-017-T02, TOOL-017-T03, TOOL-017-T04, TOOL-017-T05.
Ownership locks: crates/tools/src/mcp_bulk_actions.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/tools/src/mcp_bulk_actions.rs (new module in crate opencode-rk-tools; lib.rs wiring left to integrator).

## User-observable outcome

MCP selection and bulk actions: keyboard/mouse selection with select-all, clear, and invert; enable, disable, remove, refresh, and install actions over a bounded selection; confirmation required for destructive actions; partial failures reported per item.

## Source evidence

- crates/tools/src/mcp.rs for existing MCP/extension policy surfaces (McpConfig, McpPolicy, McpTool, McpClient).
- crates/tools/src/ext_perms.rs for existing MCP/extension policy surfaces (grant_perm, MAX_EXT_PERMS).
- crates/tools/src/ext_secure.rs for existing MCP/extension policy surfaces (grant_all, MAX_SECURE_PERMS).
- crates/tools/src/tool_allow.rs:8 for bounded tool allowlist precedent (MAX_TOOL_ALLOW).
- crates/server/src/event_bus.rs for bounded event delivery (ServerEvent, BusError Full/Closed).
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env.

## Observable contract

- `McpEntry { id: String, enabled: bool }`: identity plus flag only; caller owns registry state.
- `Selection { ids: BoundedSet<String> }`: select(id), deselect(id), select_all(ids), clear(), invert(known_ids); idempotent; duplicate selects never double-count.
- `BulkAction { Enable, Disable, Remove, Refresh, Install }`: Remove and Install are destructive/installing and require explicit confirmation.
- `apply(selection, action, confirm) -> BulkReport { per_item: Vec<ItemOutcome>, succeeded: u32, failed: u32 }`: Remove/Install without confirm yields ConsentRequired and changes nothing; per-item outcomes in deterministic id order.
- `ItemOutcome { id, outcome: Applied | Skipped | Failed { code } }`: unknown ids report Failed unknown, never abort the batch.
- Bounds: MAX_BULK_SELECTION 64; ids 1..=128 chars; per-item error codes only, no secret or destructive payload in report.
- Deterministic: same selection plus action yields byte-identical report order; no I/O, no network, no clock inside planner (execution owned elsewhere).
- Suggested module boundary: crates/tools/src/mcp_bulk_actions.rs owning McpEntry, Selection, BulkAction, BulkReport, BulkError, apply; shared lib.rs wiring left to integrator.

## Failure states

- Empty selection on apply: `Err(BulkError::EmptySelection)`; nothing applied.
- Over-cap selection (65th id): `Err(BulkError::TooManySelected)`; selection unchanged.
- Destructive action without confirm: `Err(BulkError::ConsentRequired)`; nothing applied.
- Unknown id in selection: per-item Failed unknown; rest of batch still applied.
- Secret safety: selections and reports hold ids and codes only, zero credentials. Tests use disposable in-memory entries only; no live MCP servers touched.

## Resource bounds

- Pure planner: no Command, no thread, no I/O, no network; caller owns entries and confirmation.
- Bounded selection and report; O(n) time, O(n) memory with n within 64.
- No unbounded queue, no unbounded retained output; owner and cancel path defined per crates/tools/src/tool_allow.rs:8 bounded-allowlist precedent.

## Acceptance criteria

- TOOL-017-T01: select-all and enable: 5 entries, select_all then apply Enable yields 5 Applied in id order with succeeded 5 and failed 0.
- TOOL-017-T02: invert and clear: full selection inverted over known ids yields empty; clear after partial yields empty; deselect of one id leaves remainder intact.
- TOOL-017-T03: destructive gating: apply Remove without confirm yields ConsentRequired with zero entries removed; with confirm yields Applied removals.
- TOOL-017-T04: partial failure: selection with one unknown id yields Failed unknown for that id plus Applied for the rest; empty selection yields EmptySelection; 65 ids yield TooManySelected.
- TOOL-017-T05: determinism and isolation: same batch byte-identical report; Debug plus serialize contain ids and codes only; no subprocess, network, or file writes.

## Test-first execution

- RED: author tests TOOL-017-T01..T05 against crates/tools/src/mcp_bulk_actions.rs; establish compiling RED (fail: no mcp_bulk_actions module).
- GREEN: implement minimum native Rust bounded selection plus bulk planner; GREEN, refactor, rerun; negative tests (empty, over-cap, no-consent, unknown-id).
- Evidence: `cargo test -p opencode-rk-tools mcp_bulk_actions`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
