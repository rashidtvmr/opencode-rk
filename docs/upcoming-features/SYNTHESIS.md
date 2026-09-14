# Synthesis: harness mining vs owned backlog

Source pins: opencode-rk `a754746`; claude-code `eec3692`; codex `35d9e4b`; reverse `c0d99ea`. Detail in sibling scratchpads (claude-code-harness.md, codex-harness.md, claude-code-reverse-harness.md).

## Overlap map (both harnesses agree, owned backlog already covers)
- Plan mode / read-only planning: claude C11/#7 + codex #8. Owned: REQ-013 AGENT-007/008 UI-005. Gap: no explicit plan-mode story. Proposal: add story, not new REQ.
- Granular per-tool approvals + session cache: claude C4/#1 + codex #1. Owned: REQ-021/030 SEC-001/003/017 UI-008. Adopt enum shape only.
- Execpolicy / denylist shell gate: claude C5/#2 + codex #2. Owned: REQ-025 SEC-002/012/016 + REQ-026 SEC-013. Adopt static list + prefix-rule check.
- Compact + summarizer: claude C7/#4 + reverse #3 + codex queue/compress claims. Owned: REQ-006 SESS-001/017 + REQ-001 OPS-001/007. Adopt thresholds + 3-strike breaker.
- Skill load/invoke split + safe extract: claude C9/#5 + codex #6. Owned: REQ-017 EXT-001/002 + REQ-027 SEC-004/005. Adopt O_EXCL/0600/traversal-reject + policy gate.
- Subagent isolation (parent keeps final only): reverse #4 + codex SubAgentSource + claude C10/#6. Owned: REQ-009/010/012/014 AGENT-003/004/005/011. Adopt label + GC-able child trace.
- Cheap-model chores + cost counters: reverse #5 + claude C8/#8. Owned: REQ-018 CAT-004/AGENT-006 + REQ-016 UI-006/ROUTE-008. Adopt counter schema + chore routing.

## Unique to claude-code (adopt)
- Bounded hook bus 100 + SSRF guard + always-emit allowlist (C6/#3). Owned REQ-020 SEC-010/011 EXT-008. Low-med cost. Highest safety-per-line after denylist.
- Typed permission modes/sources/reasons enum (C4/#1). Zero runtime cost, enables audit log.
- /doctor diagnostics command (part of #8). Extends REL diagnostics.

## Unique to codex (adopt)
- Turn submission state machine: StartOrSteer/StartIfIdle + NotSubmittedReason (codex #3). Owned REQ-002/012/013. Pure core logic, kills busy-error ad-hocery.
- Fork/revert/rollback split with typed ForkBoundary (codex #4). Owned REQ-008 SESS-011 UI-004. Revert free pointer move, only fork copies.
- Dual rollout JSONL truth + sqlite snapshot (codex #5). Owned REQ-006/033. Matches ADR-004.
- request_permissions mid-turn tool with scope (codex #7). Owned REQ-021/026. Structured escalation over free-text asks.
- Structured review child emitting findings schema (codex #8). Owned REQ-009/013. Machine-checkable review.

## Unique to reverse (adopt)
- Provider-boundary tap with redaction, uid-keyed records (reverse #1). No owned story (ralph grep: no transcript/observability match). Propose NEW-REQ PROV-OBS. Single seam, off-by-default in perf path.
- Content-addressed transcript dedupe by canonical hash (reverse #2). Owned DB-008/009. Mechanical storage win.
- Offline debug-export bundle + CDN-free viewer (reverse #6). Owned OPS/SESS-017/UI-006 adjacency. Makes bug reports reproducible.

## Unified ranked Top 10 for lean clone
1. Dangerous-pattern denylist + execpolicy prefix check. REQ-025/026. Low cost. Do first.
2. Turn submission state machine. REQ-002/012/013. Low-med. Lease one slice.
3. Granular approvals + session cache (clear on fork). REQ-021/030. Low.
4. Bounded hook bus 100 + SSRF guard. REQ-020. Low-med.
5. Provider tap + redaction (NEW-REQ PROV-OBS) + debug-export bundle. NEW + OPS. Low-med. Enables all future debugging.
6. Dual rollout JSONL + sqlite + content-hash dedupe. REQ-006/033. Med.
7. Fork/revert/rollback split. REQ-008. Low.
8. Auto-compact thresholds + breaker + cheap-model chores + cost counters. REQ-006/018/016/001. Med.
9. Plan mode + structured review child + request_permissions tool. REQ-013/009/021. Low-med combined slice.
10. Skill safe-extract + skill/MCP policy gate + /doctor. REQ-017/027/005 + REL. Low.

## Explicitly reject (all three sources)
- Ink/React TUI, OTel/GrowthBook weight, 90 slash commands (keep 8), 40 tools 1:1 (keep factory + ToolSearch), full MCP client depth, CDN viewer, v1 minified-JS technique, copying prompt text verbatim, 9router-style router in codex (absent; ours already planned).

## Proposed NEW-REQs (only 3)
- PROV-OBS provider tap + redaction + debug export (reverse #1/#6).
- PLAN-MODE explicit story under REQ-013 (or standalone micro-REQ if ledger demands).
- STRUCTURED-REVIEW output schema (codex #8 tail).
- Everything else maps to existing REQ/story; no new subsystem.
