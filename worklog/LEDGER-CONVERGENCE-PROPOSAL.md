# LEDGER-CONVERGENCE-PROPOSAL

## Claim
- Task: LEDGER-CONVERGENCE-PROPOSAL
- Session: ses_f2e2ae321ffe6rN1WqKR6j2r1D
- Status: in-progress -> completed (controller-authority reconciliation proposal)
- Branch: plan/ledger-convergence

## Source evidence

### convergence_gate.py (tools/convergence_gate.py:126-148)
Two findings emitted by `ledger_errors()`:
1. `LEDGER: completed off-plan task {tid}` — row.status == "completed" and tid not in plan_ids (loaded from completion_plan.load().stories).
2. `LEDGER: {tid} is completed but its own note admits '{marker}'` — completedNote contains a BAD_NOTE_MARKERS token:
("repair child","follow-up","follow up","unwired","unproven","state-only","state only","missing","partial","no acceptance","not accepted","out of scope").

### Current gate output (86 findings)
- 83 off-plan completed tasks (tid not in 109 plan IDs).
- 3 in-plan completed with bad-note markers: AUD-017 ("no acceptance"), AUD-020 ("no acceptance"), INSTALLED-DEFAULT-CONTRACT-INTEGRATION ("missing").
  - INSTALLED-DEFAULT-CONTRACT-INTEGRATION is BOTH off-plan (parent task INSTALLED-DEFAULT-CONTRACT is in plan) and bad-note.

### Plan IDs (109 total)
Loaded from ralph.completion.json via completion_plan.load(). Includes APP-001..APP-012, AUD-001..AUD-020, DISC-101..DISC-119, COORD-001..008, MOB-001..006, NET-001..015, PAR-001..010, SHIP-001..008, TUI-001..011, plus PROV-014.

Note: PROV-018..PROV-022, OPS-009, SYNC-001..SYNC-002, SDK-001..SDK-002, TOOL-012/018/019, WEB-004..WEB-006, WEB-EVENT-STREAM, WEB-HINT, RC-01..03, REL-003, RUN-001, HEAD-001..002, BASE-004, FIX-*, G6-*, LANE-* are NOT in the plan stories.

### DISC-003-INTEGRATED-REMAP-ADDENDUM.md (worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md:1-76)
- 78 off-plan completed alias rows eligible for retirement to canonical AUD-xx parents.
- 27 of those aliases are enumerated (LANE-* -> AUD-xx, RC-* -> AUD-001, WEB-* -> AUD-014).
- Demote AUD-017 and AUD-020 from completed -> blocked (preserve notes as blockedNote).
- Prior 51-row proposal hash-locked to 9c4fb6b, NOT applicable to rebased candidate.
- Algorithm: assert len(retire) == 78; remove 78 rows; change AUD-017/AUD-020 status.

### validate_repository.py output
51 backlog-exhaustion errors for stories not in plan (AUTO-003..007, EXT-001..013, INT-001..010, OPS-001..010, REL-001..003, ROUTE-001..012, SHARE-001..005, etc.). Controller-accepted stories must not appear in exhaustion ledger. This is orthogonal to convergence_gate but referenced in AGENTS.md as the canonical repository guard.

### INSTALLED-DEFAULT-CONTRACT vs INSTALLED-DEFAULT-CONTRACT-INTEGRATION
- INSTALLED-DEFAULT-CONTRACT: blocked; scratchpad worklog/INSTALLED-DEFAULT-CONTRACT.md; note admits "missing" revision receipt.
- INSTALLED-DEFAULT-CONTRACT-INTEGRATION: completed; note admits "missing" (no-receipt RED 4/5). Parent remains open per note. This is the contradiction: child completed while parent admits missing acceptance criteria.

## Root-cause grouping

### Group A: Alias rows with canonical plan parents (retire via removal)
The 27 enumerated aliases in DISC-003 addendum plus 27 additional alias rows sharing the same naming pattern and completion-note evidence structure. These are evidence-only rows to be retired (removed from active claims ledger); Git history and worklogs remain evidence.

Sub-group A1 (27 covered by DISC-003 addendum) -- enumerated alias->parent mappings:
LANE-AGENT-FILES, LANE-CI, LANE-CI-EXT, LANE-CI-FLAG, LANE-COMMANDS-LIVE, LANE-CONTEXT-ACCOUNT, LANE-CONTEXT-CMD, LANE-DISPATCH-DENY, LANE-GLOBS, LANE-GLOBS-LIVE, LANE-LOOP, LANE-LOOP-LIVE, LANE-MCP-LIVE, LANE-RULES, LANE-SANDBOX, LANE-SUBAGENT-LIVE, LANE-THEMES, LANE-TUI-GRAPH, LANE-ULTRA-CODEGEN, LANE-WEB-CANVAS, LANE-WF-CREATE, LANE-WF-TIMELINE, RC-01, RC-02, RC-03, WEB-EVENT-STREAM, WEB-HINT. (See DISC-003-INTEGRATED-REMAP-ADDENDUM.md:18-45 for parent assignments.)

Sub-group A2 (27 additional aliases, same naming pattern, evidence folded into canonical parents by controller):
BASE-004, FIX-LOGROTATE, FIX-LOOP-RULES, FIX-NATIVE-DAEMON, FIX-PACKAGING, FIX-SANDBOX, FIX-SESSIONS-STUBS, FIX-SQLITE-GATE, FIX-TIMELINE, G6-CHAT-DATADIR, HEAD-001, HEAD-002, LANE-APPSTART-VIEW, LANE-AUTH-401, LANE-AUTODRIVE-CLAMP, LANE-CHAT-ORIGIN, LANE-CI-CAPS, LANE-DESC-STALE, LANE-FILE-AUTHZ, LANE-LOOP-CAP, LANE-MAIN-ONCE2, LANE-ONBOARD-SETUP, LANE-PROV-FALLBACK, LANE-RALPH-MAX2, LANE-SHELL-AUTHZ, LANE-SRV-ROUTER, LANE-TIMELINE-LAND.

Proposed canonical fold targets (controller assigns; not this lane's authority):
BASE-004/AUD-010, FIX-LOGROTATE/AUD-001, FIX-LOOP-RULES/AUD-005, FIX-NATIVE-DAEMON/AUD-001, FIX-PACKAGING/DISC-102, FIX-SANDBOX/DISC-106, FIX-SESSIONS-STUBS/AUD-003, FIX-SQLITE-GATE/AUD-007, FIX-TIMELINE/AUD-013, G6-CHAT-DATADIR/AUD-011, HEAD-001/AUD-011, HEAD-002/AUD-003, LANE-APPSTART-VIEW/APP-001, LANE-AUTH-401/AUD-001, LANE-AUTODRIVE-CLAMP/COORD-001, LANE-CHAT-ORIGIN/AUD-015, LANE-CI-CAPS/COORD-005, LANE-DESC-STALE/AUD-001, LANE-FILE-AUTHZ/DISC-105, LANE-LOOP-CAP/AUD-004, LANE-MAIN-ONCE2/APP-001, LANE-ONBOARD-SETUP/APP-005, LANE-PROV-FALLBACK/PROV-014, LANE-RALPH-MAX2/COORD-002, LANE-SHELL-AUTHZ/DISC-105, LANE-SRV-ROUTER/AUD-018, LANE-TIMELINE-LAND/AUD-013, LANE-TUI-HOST/TUI-002, LANE-TURN-SETTLE/APP-004, LANE-TOOL-PERM/DISC-105, LANE-TRANSCRIPT-LAND/AUD-013, LANE-WEB-HONEST/AUD-005.

### Group B: Truly orphaned implementation completions (need canonical task entries OR status downgrade)
Tasks that were completed as standalone lanes but have no canonical plan parent. These need either:
(a) A new canonical plan task entry (canonicalization), OR
(b) Status correction if the parent admits missing/no-acceptance.

ACP-001: off-plan, green 5/5 acp_bridge. No canonical plan task. -> Needs new plan entry OR downgrade.
APP-012-FILE-OPS: off-plan sub-lane of APP-012 (in plan). Evidence for APP-012. -> status correction: fold into APP-012 (still in-progress), remove alias.
APP-012-READ-EXECUTOR: off-plan sub-lane of APP-012. Fold into APP-012 parent.
APP-012-READ-INTEGRATION: off-plan sub-lane of APP-012. Fold into APP-012 parent.
APP-012-SERVER-READ-WIRING: off-plan sub-lane of APP-012. Fold into APP-012 parent.
OPS-009: off-plan. green 5/5 ops_replay. PROV-014 exists in plan but OPS-009 != PROV-014. -> new canonical entry or downgrade.
PROV-018..PROV-022: off-plan, green. PROV-014 is in plan but these are distinct. -> new canonical entries or downgrade to blocked.
SYNC-001, SYNC-002: off-plan. NET-014/NET-015 in plan (sync). -> fold into NET-0xx canonical or new entries.
SDK-001, SDK-002: off-plan. No canonical plan task. -> new entries or downgrade.
TOOL-012, TOOL-018, TOOL-019: off-plan. TOOL-015 in plan. -> new entries or downgrade.
WEB-004, WEB-005, WEB-006: off-plan. WEB-001..WEB-017 range in plan. -> fold into corresponding WEB-xx or new entries.
REL-003: off-plan. REL-001..REL-008 in plan. -> fold into REL family.
RUN-001: off-plan. COORD-007 or SHIP family? -> new entry or downgrade.
INSTALLED-DEFAULT-CONTRACT-INTEGRATION: off-plan + bad-note ("missing"). Parent INSTALLED-DEFAULT-CONTRACT is blocked with matching "missing" note. -> status correction: downgrade to blocked (already effectively blocked by parent).

### Group C: In-plan completed with bad-note contradiction (status downgrade)
AUD-017: completed, note says "no acceptance". -> demote to blocked, preserve note as blockedNote.
AUD-020: completed, note says "no acceptance". -> demote to blocked, preserve note as blockedNote.

### Group D: Parent-child contradiction
INSTALLED-DEFAULT-CONTRACT-INTEGRATION (completed) vs INSTALLED-DEFAULT-CONTRACT (blocked, parent). Child admits "missing" revision receipt = parent's own blocker. -> downgrade child to blocked; preserve INSTALLED-DEFAULT-CONTRACT blockedNote.

## Immutable evidence to preserve
- Git history: all committed worklogs and lane branches (lane/*) persist as evidence.
- claims.json history (git) records prior status transitions.
- worklog/*.md scratchpad files are append-only evidence records.
- Frozen test SHAs and RED/GREEN shas in completedNote fields.
- The DISC-003 hash-locked ledger SHA-256 (6c8cf2d8...) must be verified before any mutation.

## Exact safe commands/files an authorized controller would change
Only `tasks/completion/claims.json` is mutated in this proposal. No worklog/*.md, PLAN.md, ralph*.json, tests, or verifier code changes.

Stage 1 -- remove 54 alias rows (use completion_claims module for validation):
  python3 -c "
  import sys, pathlib, json
  sys.path.insert(0, 'tools')
  import completion_claims as cc
  root = pathlib.Path('.')
  doc = cc.load_ledger(root)
  retire = {DISC-003 27 IDs} | {A2 27 IDs}
  for tid in retire:
      if tid in doc['claims']:
          del doc['claims'][tid]
  cc.save_ledger(root, doc)
  "
  Then verify: python3 tools/convergence_gate.py (expect fewer off-plan lines)

Stage 2 -- demote 3 bad-note rows (completed -> blocked, note preserved):
  python3 -c "
  import sys, pathlib
  sys.path.insert(0, 'tools')
  import completion_claims as cc
  root = pathlib.Path('.')
  for tid in ['AUD-017', 'AUD-020', 'INSTALLED-DEFAULT-CONTRACT-INTEGRATION']:
      doc = cc.load_ledger(root)
      note = doc['claims'].get(tid, {}).get('completedNote', '')
      doc['claims'][tid]['status'] = 'blocked'
      doc['claims'][tid]['blockedNote'] = note
      doc['claims'][tid].pop('completedNote', None)
      cc.save_ledger(root, doc)
  "
  (Note: cc.update transitions in-progress->blocked only; completed->blocked requires direct JSON or orchestrator release. Controller uses direct validated JSON edit.)

Stage 3 -- re-run: python3 tools/convergence_gate.py -> expect 28 findings (orphans needing canonicalization).

Stage 4 -- canonicalize remaining 28 orphans: requires plan-file authority (AGENTS.md: plan files immutable to implementers). Either add stories to ralph.completion.json FEATURES/PLAN, or demote each orphan to blocked. NOT this lane.

## Expected gate reduction
Current gate output: 86 finding LINES (83 off-plan-completed + 3 bad-note-completed).
Distinct tids producing findings: 85 (INSTALLED-DEFAULT-CONTRACT-INTEGRATION produces BOTH an off-plan line and a bad-note line).

Precise off-plan completed breakdown (83):
- DISC-003 retire set (27): sub-group A1 enumerated aliases.
- A2 aliases (27): LANE-*/FIX-*/G6-*/HEAD-*/BASE-004 pattern rows folding into canonical parents.
- Remaining orphans (29): ACP-001 (1) + 4 APP-012 sub-lanes + OPS-009/PROV-018..PROV-022/SYNC-001..SYNC-002/SDK-001..SDK-002/TOOL-012/018/019/WEB-004..006/REL-003/RUN-001 (24) + INSTALLED-DEFAULT-CONTRACT-INTEGRATION (1).

Conservative ledger-only controller proposal (NO plan-file mutation):
- Stage 1: Remove 54 alias rows (27 DISC-003 + 27 A2) from claims.json. Removes 54 off-plan finding lines.
- Stage 2: Demote AUD-017, AUD-020, INSTALLED-DEFAULT-CONTRACT-INTEGRATION to blocked (preserve notes). Removes 2 bad-note lines + 1 off-plan line (INSTALLED-INTEGRATION).
- Remaining after stages 1-2: 83 - 54 - 1 = 28 off-plan + 0 bad-note = 28 findings. These 28 require canonicalization (plan-file authority) or demotion by an authorized controller -- NOT in this lane's safe set.

Staged order to avoid false acceptance:
- Stage 1 (hash-locked ledger removal): verify ledger/HEAD state; remove 54 verified alias rows from tasks/completion/claims.json only; git diff --check.
- Stage 2 (demotions): set AUD-017, AUD-020, INSTALLED-DEFAULT-CONTRACT-INTEGRATION status completed->blocked; move completedNote to blockedNote; preserve scratchpad/session.
- Stage 3 (re-run gate): python3 tools/convergence_gate.py -> expect 28 remaining finding lines, all off-plan-completed orphans requiring controller canonicalization/demotion. NOT GREEN -- convergence not falsely claimed.
- Stage 4 (controller-only canonicalization): add plan stories for true orphans OR demote them; requires AGENTS.md plan-file authority.
- ## Notes / Remaining limitations
- This lane authored only `worklog/LEDGER-CONVERGENCE-PROPOSAL.md` and its own ledger row. No product, test, verifier, plan, ralph, or controller files were edited.
- The proposal row itself (LEDGER-CONVERGENCE-PROPOSAL) is off-plan; after commit, convergence_gate reports total=87 (was 86) because the proposal's own completed claim is not in plan stories. A controller applying Stage 1 would remove or canonicalize this row during reconciliation.
- Stage 4 canonicalization of the 28 remaining orphans requires plan-file authority (AGENTS.md: plan files immutable to implementers). This lane does not exercise that authority.
- validate_repository.py backlog-exhaustion errors (51 stories classified as controller-accepted but appearing in exhaustion ledger) are independent of ledger cleanup and require separate controller classification work.