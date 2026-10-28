# AGENTS.md documentation audit
Updated: 2026-10-26; role: auditor; session: ses_e88d69cd4ffeXQ7i0yUzX5nXew
Base/candidate SHA: 7b94da36; upstream SHA: 450e7b92412c780a7b5a75226bb8d49aaca24e22

## Goal and success criteria
Verify AGENTS.md faithfully captures latest user allowlist/budgets, 20+ role-first, scratchpads, upstream, immediate integration. Output discrepancies or APPROVE.

## Evidence read (file, symbol/line, revision)
- AGENTS.md:1-303 (full)
- docs/AGENT_WORKER_PRACTICES.md:1-72 (full)
- docs/worklogs/ORCHESTRATOR_SCRATCHPAD.md:7,29,30,178

## Findings (confirmed / hypothesis / refuted)
Confirmed:
- Allowlist + budgets present (lines 18-26): vyce-deepseek-v41 (8 concurrent, unlimited cumulative), vyce-glm-5-3-flash (50M tokens), 9router-vyce-deepseek-v4.1-free-combined (no limit), xkiro-gpt-6-luna (resolve identifier), 9router-xk-qwen38-max-free (8M), 9router-luna-free (if working), 9router-th-dsv41-flash-free (if free). All matches user override.
- Track usage/exhaustion in orchestrator scratchpad + record UNKNOWN if absent present (lines 32-37).
- 20+ concurrent requirement present (lines 41-43, 247-249).
- Roles first, goals, success criteria present in AGENTS.md (lines 61-72) and reinforced in AGENT_WORKER_PRACTICES.md template.
- Scratchpad mandates: every research subagent unique scratchpad (lines 91-95); orchestrator maintains ORCHESTRATOR_SCRATCHPAD.md (lines 96-99); private dir absolute outside canonical (lines 100-105). Matches.
- Upstream reference: user-selected 450e7b92 (lines 79-87); supersedes sources/upstream.lock.json (line 82). Authority order cites upstream.lock.json at line 213 — potential inconsistency.
- Single integration writer + main-v2 canonical present (lines 53-56, 155, 205, 249-253).
- Temporary override recorded (lines 109-133) with exact attribution.

Hypothesis:
- ORCHESTRATOR_SCRATCHPAD.md exists but only partial grep match; may not fully track all allowlist budgets.

Refuted: none.

## Decisions and rationale
No authoritative contradiction between AGENTS.md and AGENT_WORKER_PRACTICES.md.

## Alternatives rejected and why
N/A.

## Verification actually executed (commands, results, raw log paths)
None (auditor role, read-only).

## Artifacts and hashes
Scratchpad at /Users/mymac/Projects/opencode-rk-recovery-20261005/.private-tmp/opencode/docs-auditor/scratchpad.md

## Remaining uncertainties, blockers and next steps
- Alias ambiguity: `xkiro-gpt-6-luna` user spelling has no in-repo mapping; AGENTS.md defers resolve-to-harness (lines 28-31, 30). Acceptable risk: AGENTS.md correctly says do not use without resolution. Not a doc bug.
- Potential contradiction: authority order (line 213) cites sources/upstream.lock.json as authority #3, while Section "OpenCode V2 is architectural reference" (lines 74-87) says user-selected 450e7b92 supersedes that lock. AGENTS.md itself resolves this (line 81-82) but phrasing in authority order could be read as conflicting. Minor; not a functional discrepancy.
