# BRIDGE-050 session_plugins

Claim: plugin list enable/disable UI state.
Source: `packages/tui/.../system/plugins.tsx:135-144` row, `:150-232` View/flip/cursor, `:9` manager id; `system_plugins.rs` covers other builtins, not this list.
Boundary: list state only. No commands/registry (plugin_host lane), no install flow.
Tests: 6 (bounds, dup, toggle, manager lock, clamp, empty-safe).
Decisions: toggle returns bool not Result (TS toasts on fail, no throw); cursor clamps no wrap (DialogSelect onMove linear).
Unknowns: none.
