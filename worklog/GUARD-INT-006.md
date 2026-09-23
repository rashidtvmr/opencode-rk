# GUARD-INT-006 reconciliation audit

Claim: INT-006, session ses_f320c8902ffeeq4dUaNnSQmtdp, scratchpad worklog/GUARD-INT-006.md.
Worktree: guard-INT-006 (branch lane/GUARD-INT-006-20260923, HEAD b3e0d01). Audit-only: no canonical/product/test edits.

## Source evidence (guard worktree paths)
- ralph.json:1101-1111 INT-006: status accepted, requirementIds [], userStory generic DISC-002 placeholder, testObligations T01-T05. PROV-016/017 list INT-006 as dependency (:3561,:3580).
- tasks/INT-006.md: EXISTS. New native remote-MCP transport/auth boundary card (McpTransport Sse/WebSocket/Stdio, McpAuth handles-only, validate/connect/close, HANDSHAKE_TIMEOUT_MS 10000, 64KiB, URL 2KiB, argv 32x1KiB, handle 128B, int_mcp.rs additive, T01-T05 frozen).
- worklog/INT-006.md: EXISTS. Verification-only: crates/providers/src/mcp_transport.rs + crates/providers/tests/mcp_transport.rs T01-T05, GREEN 5/5, mutation probe 4/1 (T04), full-crate 312 passed, hashes recorded.
- sources/integrations-ownership-gap.json: status source-reviewed-no-exact-task-owner; storyIds [INT-001,003,005,006,007,009]; ownershipDecision.INT-006 null; taskBindingState.INT-006 generic-disc-002-no-card-or-worklog controllerStatus in-progress; unresolvedPartitions include provider-specific-executable-auth-refresh-network-and-secret-authority + external-protocol-reconnect-timeout-and-resource-lifetime; closureCriteria requires explicit credential/auth/reconnect/timeout/lifetime ownership, no re-own of INT-004/INT-008.
- sources/backlog-exhaustion.json stories INT-006: category unresolved-decomposition, controllerStatus in-progress, reasonKey integration-family-not-decomposed, taskCard null, worklog null, implementationCommits [], surfaceIds [opencode.integrations].
- sources/behavior-surface-rules.json: opencode.integrations featureIds 10 INT rows incl INT-006, candidate-only.
- sources/completion/audits/AUD-009.json: assignedLegacy incl INT-006; finding MCP live spawn/auth unverified (types only).
- FEATURES.md:18 + :636 + :888: INT-006 accepted placeholder rows. ralph.completion.json:26 legacyAcceptedIsReleaseEvidence false, so accepted is not release evidence.
- Local impl present but unattributed: crates/providers/src/mcp_transport.rs, crates/providers/src/int_mcp_lane.rs, crates/providers/tests/mcp_transport.rs + int_mcp_lane.rs, lib.rs:38 pub mod mcp_transport.
- tools/validate_backlog_exhaustion.py:2010-2130 integrations_ownership_gap_errors expects ralph status in-progress (not-started only INT-009), generic story, no reqs, binding match, no tasks/<ID>.md or worklog/<ID>.md, backlog projection unresolved-decomposition/taskCard null/worklog null/commits [].

## Observed validator output (read-only runs, guard worktree)
- python3 tools/validate_backlog_exhaustion.py exit=1, 51 errors.
- Exactly one error line attributable to INT-006: `INT-006: integrations ownership-gap is stale after Ralph semantics changed` (ralph accepted vs expected in-progress short-circuits via continue; task/worklog-appeared check not reached).
- Repo-wide lines (accepted-classification, controller-accepted, summary drift, 30 other stale lines incl other INT rows) are shared pre-existing conditions, not INT-006-owned.

## Wiring / lifecycle / protocol / resource-bound gaps (local GREEN is not acceptance)
- Wiring: local mcp_transport/int_mcp_lane modules exist and lib.rs exports, but gap ownership null + backlog implementationCommits [] + no caller wiring evidence to real socket/process/secret resolution; card assigns handle-to-secret, socket/process, session semantics, persistence, events to caller - unwired.
- Lifecycle: bounded handshake/timeout/cancel/close/Drop claimed locally; upstream attempt lifecycle (10min/1min/30s/5min, attempt map no cap, background callbacks, credential persistence, events) explicitly excluded per gap integrationAuthConstraints/overlap; no genuine spec for lifecycle constants.
- Protocol: SSE/WebSocket/Stdio fake-transport only, no live network, no MCP session/tool/prompt semantics (card exclusion); AUD-009 MCP live spawn/auth unverified.
- Resource bounds: local caps (64KiB/10s/url/argv/handle, no reconnect/registry/outbox) deliberate; upstream unbounded outbox + graceful-shutdown tracking + attempt-capacity gaps unresolved; caller concurrency bound not established.

## Disposition: BLOCKED (source-grounded)
- Gap ownership null, backlog taskCard/worklog null with zero implementation commits, validator stale-semantics line attributable, accepted status explicitly not release evidence. Nothing independently proves integration acceptance.
- No authority patch inside guard bounds: flipping ralph.json status or inventing task semantics is controller authority and would mutate the frozen accepted-task surface; product/test edits forbidden.

## Authority patch fields
- None. Lawful change requires controller-authored source-grounded decomposition reconciling ralph.json + sources/integrations-ownership-gap.json (ownershipDecision, taskBindingState, equivalenceGroup, closureCriteria for network/transport credential/auth/timeout/lifetime) + sources/backlog-exhaustion.json (taskCard/worklog/implementationCommits/category) under integration authority.

## Verification (read-only, no Cargo/network/heavy)
- python3 tools/validate_backlog_exhaustion.py (exit 1, 51 errors; 1 line attributable to INT-006 as above)
- Scoped diff check: only worklog/GUARD-INT-006.md + tasks/completion/claims.json touched
