# TOOL-018

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-042.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: TOOL-018-T01, TOOL-018-T02, TOOL-018-T03, TOOL-018-T04, TOOL-018-T05.
Ownership locks: crates/tools/src/mcp_lifecycle.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/tools/src/mcp_lifecycle.rs (new module in crate opencode-rk-tools; lib.rs wiring left to integrator).

## User-observable outcome

Per-MCP power toggle and lifecycle state: each server is enabled, disabled, starting, ready, or error; state persists across restarts; reconnect and refresh are explicit user actions; off means no tool registration and no invocation.

## Source evidence

- crates/tools/src/mcp.rs for existing MCP/extension policy surfaces (McpConfig, McpPolicy, McpTool, McpClient).
- crates/tools/src/ext_perms.rs for existing MCP/extension policy surfaces (grant_perm, MAX_EXT_PERMS).
- crates/tools/src/ext_secure.rs for existing MCP/extension policy surfaces (grant_all, MAX_SECURE_PERMS).
- crates/tools/src/tool_allow.rs:8 for bounded tool allowlist precedent (MAX_TOOL_ALLOW).
- crates/server/src/event_bus.rs for bounded event delivery (ServerEvent, BusError Full/Closed).
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env.

## Observable contract

- `LifecycleState { Disabled, Enabled, Starting, Ready, Error { code } }`: explicit per-server state; Error carries code only, never secrets or stack contents.
- `set_enabled(registry, id, on) -> LifecycleState`: true moves Disabled to Enabled; false moves any state to Disabled and unregisters tools; unknown id yields error.
- `mark_starting / mark_ready / mark_error(registry, id, code)`: forward-only transitions from Enabled/Starting; terminal Error requires explicit reconnect.
- `reconnect(id) -> Enabled`: explicit user action only; never automatic, never on a timer.
- `is_runnable(id) -> bool`: true only in Ready; Disabled/Enabled/Starting/Error all return false.
- `persist(registry) -> PersistedShape`: serializable id plus state-code pairs only, bounded length; reload restores Disabled/Enabled plus Error codes, never auto-starts.
- Bounds: servers max 64, id 1..=128 chars; persisted bytes max 16 KiB.
- Deterministic: same transition sequence yields byte-identical registry order (sorted ids); no I/O, no network, no clock inside transitions.
- Suggested module boundary: crates/tools/src/mcp_lifecycle.rs owning LifecycleState, LifecycleRegistry, LifecycleError, transitions, is_runnable, persist/restore; shared lib.rs wiring left to integrator.

## Failure states

- Unknown id on any transition: `Err(LifecycleError::Unknown)`; registry unchanged.
- mark_ready from Disabled: `Err(LifecycleError::NotEnabled)`; registry unchanged.
- Empty id: `Err(LifecycleError::EmptyId)`; registry unchanged.
- Registry full at 64 plus new id: `Err(LifecycleError::Overflow)`; registry unchanged.
- Off-means-off invariant: Disabled servers contribute zero tools to registration and fail invocation lookup with NotEnabled.
- Secret safety: states, errors, and persisted shapes contain ids and codes only. Tests use disposable in-memory registries; no live servers started.

## Resource bounds

- Pure state machine: no Command, no thread, no I/O, no network; caller owns registry lifetime.
- Bounded servers and persisted bytes; O(n) listing, O(1) transitions.
- No unbounded queue, no unbounded retained output; owner and cancel path defined per crates/tools/src/tool_allow.rs:8 bounded-allowlist precedent.

## Acceptance criteria

- TOOL-018-T01: toggle happy path: Disabled enables to Enabled, Starting then Ready yields is_runnable true; disabling a Ready server yields is_runnable false with zero registered tools.
- TOOL-018-T02: error and reconnect: mark_error from Starting yields Error with code and is_runnable false; reconnect yields Enabled (not Ready); auto-transition never occurs.
- TOOL-018-T03: transition guards: mark_ready from Disabled yields NotEnabled; unknown id yields Unknown; empty id yields EmptyId; 65th server yields Overflow.
- TOOL-018-T04: persistence: persist plus restore round-trips ids and states without auto-starting; persisted bytes within 16 KiB with no secret substrings.
- TOOL-018-T05: determinism and isolation: same sequence byte-identical listing; Debug plus serialize contain ids and codes only; no subprocess, network, or file writes.

## Test-first execution

- RED: author tests TOOL-018-T01..T05 against crates/tools/src/mcp_lifecycle.rs; establish compiling RED (fail: no mcp_lifecycle module).
- GREEN: implement minimum native Rust toggle plus lifecycle machine with off-means-off; GREEN, refactor, rerun; negative tests (unknown, not-enabled, overflow, auto-start never).
- Evidence: `cargo test -p opencode-rk-tools mcp_lifecycle`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
