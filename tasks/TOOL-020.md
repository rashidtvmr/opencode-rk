# TOOL-020

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-042.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: TOOL-020-T01, TOOL-020-T02, TOOL-020-T03, TOOL-020-T04, TOOL-020-T05.
Ownership locks: crates/sessions/src/mcp_status_panel.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/sessions/src/mcp_status_panel.rs (new module in crate opencode-rk-sessions; lib.rs wiring left to integrator).

## User-observable outcome

MCP state integrated into the right-side info panel and status events: bounded EventBus updates carry counts, tools, last error, and latency; panel refreshes without blocking agent turns; diagnostics redacted.

## Source evidence

- crates/server/src/event_bus.rs for bounded event delivery (ServerEvent, EventKind, BusError Full/Closed, bounded fan-out).
- crates/sessions/src/ui_006.rs:13-41 existing status panel shape and bounds (StatusCounts, StatusPanel, StatusPanelError, build_panel, MAX_PANEL_SESSIONS).
- crates/tools/src/mcp.rs for existing MCP/extension policy surfaces (McpConfig, McpPolicy, McpTool, McpClient).
- crates/tools/src/ext_perms.rs for existing MCP/extension policy surfaces (grant_perm, MAX_EXT_PERMS).
- crates/tools/src/ext_secure.rs for existing MCP/extension policy surfaces (grant_all, MAX_SECURE_PERMS).
- crates/tools/src/tool_allow.rs:8 for bounded tool allowlist precedent (MAX_TOOL_ALLOW).
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env.

## Observable contract

- `McpStatus { connected, enabled_count, disabled_count, tools: BoundedVec<String>, last_error: Option<String>, latency_ms: Option<u64>, updated_ms: u64 }`: caller-supplied values; last_error is a code/label only, never secrets.
- `summarize(status) -> McpCounts { connected, enabled, disabled, tools_shown, truncated: bool }`: bounded counts with honest truncation flag.
- `render(status, width) -> Vec<String>`: pure panel fragment; every line within width; compact form below 60 columns.
- `publish(bus, status) -> Result<(), PublishOutcome>`: bounded non-blocking EventBus send; Full bus yields Skipped rather than blocking the agent turn.
- Bounds: tools max 16 shown, error label max 256 chars, latency max 3_600_000 ms; over-cap truncates with marker.
- Deterministic: same status plus width yields byte-identical lines; no I/O, no clock reads inside render/publish (caller supplies updated_ms).
- Suggested module boundary: crates/sessions/src/mcp_status_panel.rs owning McpStatus, McpCounts, McpPanelError, summarize, render, publish; shared lib.rs wiring left to integrator.

## Failure states

- Full event bus: publish returns Skipped, agent turn continues; never blocks, never retries unboundedly.
- Empty tool entries or over-long error label: truncated or skipped with flag; never exceeds caps.
- Width below 20 columns: `Err(McpPanelError::TooNarrow)`; caller falls back to counts line only.
- Stale status (updated_ms older than caller horizon): rendered with "stale" marker, never presented as live.
- Secret safety: statuses, events, errors, and logs contain counts, names, and codes only. Tests use disposable snapshots and a bounded fake bus only.

## Resource bounds

- Pure summarize/render plus single bounded send; O(tools) time, O(lines) memory within caps; no background threads.
- Refresh never blocks agent turns: publish is try-send only; no unbounded queue beyond the bus cap, no unbounded retained output; owner and cancel path defined per crates/server/src/event_bus.rs bounded fan-out precedent.

## Acceptance criteria

- TOOL-020-T01: panel happy path: representative status at width 120 renders connected/enabled/disabled counts, tools, last error code, latency, and timestamp within width.
- TOOL-020-T02: narrow and stale: width 50 renders compact form within width; stale updated_ms renders with stale marker and unchanged counts.
- TOOL-020-T03: non-blocking publish: full fake bus yields Skipped with the agent-turn fixture unblocked; ready bus yields Sent with counts intact.
- TOOL-020-T04: truncation: 30 tools truncate to 16 with marker and truncated true; over-long error label truncates within 256 chars.
- TOOL-020-T05: redaction and determinism: output plus published events contain no fixture secrets; repeated render byte-identical; no network or file writes.

## Test-first execution

- RED: author tests TOOL-020-T01..T05 against crates/sessions/src/mcp_status_panel.rs; establish compiling RED (fail: no mcp_status_panel module).
- GREEN: implement minimum native Rust MCP status fragment with non-blocking publish; GREEN, refactor, rerun; negative tests (full-bus, too-narrow, stale, truncation, leak scan).
- Evidence: `cargo test -p opencode-rk-sessions mcp_status_panel`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
