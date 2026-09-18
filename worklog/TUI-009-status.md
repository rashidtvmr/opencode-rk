# TUI-009-status — lane scratchpad

Claim: `crates/cli/src/native_status.rs` pure status-view state only, std only, `forbid(unsafe_code)`.
Base commit: 5af7884. Card: TUI-009 (T01..T05). No other files touched.

Source evidence:
- Card via `python3 tools/completion_plan.py --card TUI-009`: journey = usage/cache/context/memory, quota, MCP/tool state via daemon; tests T01 counters from normalized provider events, T02 memory/context sources + visibility, T03 MCP enable/disable/search lifecycle, T04 quota/fallback/connectivity states, T05 refresh/idle budgets.
- Prior art: `crates/cli/src/native_app.rs` freshness model; `crates/sessions/src/tui_state.rs` bounded status (cited in module docs).

Observed scenario: file already present on disk as untracked lane artifact (13.4K, full types + 6 tests) but `git status` shows `??` (never committed). Only defect found: `usage_observed` used `any(|e| event_value(e) > 0 || true)` — always-true RHS, clippy `logic_bug` class. Fixed to `!self.events.is_empty()`.

Target boundary: owned file only. Types: UsageEvent, Counter (Unobserved vs Measured, `value()->Option`), Visibility/ContextEntry.renderable, McpState/McpAction/McpServer, Connectivity, StatusBoard with bounded events/servers/context, quota/fallback, `request_refresh` gated by MIN_REFRESH_INTERVAL_TICKS const. No render/IO/threads.

Tests (in-file, frozen): unavailable-vs-zero, visibility policy, MCP lifecycle action→report flip + search, quota/connectivity, refresh budget, retention bounds. 6 tests.

Decisions: one-line fix only; no API change; no new deps; saturating_add for counters.

Remaining unknowns: none in-lane. Integration/wiring to daemon caller out of scope for this file.
