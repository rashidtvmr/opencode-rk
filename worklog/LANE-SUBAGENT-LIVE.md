# LANE-SUBAGENT-LIVE — Parent→Child Delegation Contract Composition Layer

## Claim

RAW_FEATURE 1.3: parent→child DELEGATION CONTRACT as a pure composition layer.
Given parent session context + delegation request, produce typed child spawn
plan and completion handback, then verify end-to-end round trip.

## Session

ses_worker_subagent_live

## Source Evidence

- `crates/agents/src/app_delegation.rs` — existing delegation primitives (ChildId, Delegations, Ownership, etc.)
- `crates/agents/src/session.rs` — existing session types (AgentSession, SessionId)
- `crates/agents/src/context.rs` — existing ExecutionContext, ContextLimits

## Design Decision

Self-contained module: `delegation_live.rs` mirrors the public API surface of
`app_delegation` and `session` rather than importing via `crate::` (which doesn't
work in standalone `#[path]` test harness). This avoids touching existing files
and keeps the composition layer independent.

Types:
- `DelegationRequest` — parent's delegation intent (agent_id, task, budget)
- `SpawnPlan` — typed child spawn plan (fresh SessionId, ChildLimits, parent link)
- `Handback` — completion return (bounded output, status, transcript entry)
- `CompositionError` — typed errors for the composition layer

Functions:
- `build_spawn_plan(parent, req) -> SpawnPlan` — produces child plan from parent context
- `complete_handback(delegations, child, owner, hb) -> Result<()>` — reclaims live slot via existing API

## Tests (11 total, 10 owned + 1 module test)

| # | Test | What it verifies |
|---|------|-----------------|
| T01 | `t01_spawn_plan_fresh_session_id` | Child SessionId differs from parent |
| T02 | `t02_spawn_plan_forks_parent_context` | Agent id, task, limits inherited |
| T03 | `t03_spawn_plan_budget_inheritance` | Budget values pass through correctly |
| T04 | `t04_spawn_plan_cancellation_link` | link_to_delegation attaches ChildId |
| T05 | `t05_handback_output_bounds` | Output truncated to 8KiB max |
| T06 | `t06_handback_status_values` | Status string stored correctly |
| T07 | `t07_full_round_trip` | End-to-end: request → plan → spawn → handback → transcript |
| T08 | `t08_handback_cancelled_child` | Handback on terminal child returns error |
| T09 | `t09_distinct_session_ids` | Each plan gets unique SessionId |
| T10 | `t10_spawn_plan_parent_link` | Parent session/agent metadata preserved |
| mod | `ownership_and_effort` | Basic type assertions |

## RED/Green Evidence

- RED sha256 (tests + stubs): f2abcd19 (impl), 0fa8db9e (tests)
- GREEN: `cargo test -p opencode-rk-agents --test delegation_live` = 11 passed
- GREEN: `cargo test -p opencode-rk-agents` = 79 passed (68 existing + 11 new)
- Zero test edits post-freeze
- Zero regressions

## Decisions

1. Self-contained types: avoid `crate::` imports that break standalone test harness
2. Mirror delegation API: keep ChildId/Delegations/Ownership identical to app_delegation
3. Timestamp-based SessionId: `child-{parent_id}-{nanos}` ensures uniqueness
4. Output bounded to 8KiB with truncation marker (same pattern as delegation_lane)
5. `complete_handback` reclaims live slot via steer+cancel (composing existing API)

## Gaps / Follow-up

1. `delegation_live` not wired into `lib.rs` (left to integrator per lane rules)
2. `ExecutionContext` variable forking not implemented (would need HashMap clone)
3. Real cancellation token (tokio CancellationToken) not wired — this is pure composition
4. `ChildContext` type omitted — the `SpawnPlan` subsumes its role
