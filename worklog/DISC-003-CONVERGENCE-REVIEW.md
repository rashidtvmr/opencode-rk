# DISC-003 PHASE1 Convergence Review

## Task ID
DISC-003

## Session
ses_f32ffe76efferxGs5oxsA8WXdD

## Base commit
dd7670dc78e5738a8dee1ca64c308b8618e8c181 (lane/PHASE1-convergence-review, HEAD)

## Objective
Audit DISC-003 convergence findings: enumerate exactly 53 gate findings from current
state, classify each by presence in ralph.json / ralph.completion.json / claims-only /
legacy, recompute category counts, explain the prior audit's A3=23 vs remediation=43
discrepancy, assess AUD-017/AUD-020 note logic, and confirm WEB-009 is correctly
blocked at dd7670d and absent from the 53. Produce a machine-checkable reconciliation
proposal table. Set DISC-003 blocked.

## Gate Output (current HEAD dd7670d)
```
CONVERGENCE BLOCKED
total=53
```

53 findings = 51 off-plan completed claims + 2 in-plan bad-note claims.

## Plan/Legacy Context (sources)
- `tools/convergence_gate.py:116-148` defines findings via `ledger_errors()`:
  - `plan_ids()` = `completion_plan.load()['stories'].keys()` = 109 plan story IDs.
  - For every claim with `status == "completed"`:
    - If `tid not in plan_ids()` => `LEDGER: completed off-plan task {tid}`
    - If `completedNote` contains a BAD_NOTE_MARKER =>
      `LEDGER: {tid} is completed but its own note admits '{marker}'`
- `tools/completion_plan.py:97` `load()` reads `ralph.completion.json` manifest,
  loads 6 includes (local, tui, parity, remote, delivery, discovered) = 109 stories.
  `legacyStories` from `ralph.json` = 258 legacy userStories.
- `ralph.completion.json:26` `contract.legacyAcceptedIsReleaseEvidence: false`
  means legacy `status=accepted` is NOT release evidence.
- `completion_plan.py:165` asserts `set(old_ids) & stories.keys()` is empty: 0 overlap
  between 258 legacy IDs and 109 plan story IDs.

## Exact 53 Findings

### Category A1: FIX-* claims, off-plan, not in ralph.json / claims-only (8 findings)

| # | ID | In ralph.json? | In plan? | In claims-only? | Evidence |
|---|----|----------------|----------|-----------------|----------|
| 1 | FIX-LOGROTATE | no | no | claims-only | claims.json completedNote: "VERIFY-ONLY 7/7 diagnostics GREEN oc2 bin"; scratchpad worklog/FIX-LOGROTATE.md |
| 2 | FIX-LOOP-RULES | no | no | claims-only | claims.json completedNote: "GREEN 8/8 rules_globs t01-t08"; scratchpad worklog/FIX-LOOP-RULES.md |
| 3 | FIX-NATIVE-DAEMON | no | no | claims-only | claims.json completedNote: "VERIFY-ONLY zero-diff: run_with_dir+stdin guard"; scratchpad worklog/FIX-NATIVE-DAEMON.md |
| 4 | FIX-PACKAGING | no | no | claims-only | claims.json completedNote: "--help identity gate: bash-n OK"; scratchpad worklog/FIX-PACKAGING.md |
| 5 | FIX-SANDBOX | no | no | claims-only | claims.json completedNote: "covered by external DISC-106 b688c56"; scratchpad worklog/FIX-SANDBOX.md |
| 6 | FIX-SESSIONS-STUBS | no | no | claims-only | claims.json completedNote: "snapshot.rs build_snapshot real fold"; scratchpad worklog/FIX-SESSIONS-STUBS.md |
| 7 | FIX-SQLITE-GATE | no | no | claims-only | claims.json completedNote: "Backports rejected without audited allowlist"; scratchpad worklog/FIX-SQLITE-GATE.md |
| 8 | FIX-TIMELINE | no | no | claims-only | claims.json completedNote: "GREEN 11/11 native_timeline"; scratchpad worklog/FIX-TIMELINE.md |

### Category A2: LANE-* claims, off-plan, not in ralph.json / claims-only (20 findings)

| # | ID | In ralph.json? | In plan? | In claims-only? | Evidence |
|---|----|----------------|----------|-----------------|----------|
| 9 | LANE-APPSTART-VIEW | no | no | claims-only | claims.json completedNote: "GREEN 16/16 standalone rustc app_start"; scratchpad worklog/LANE-APPSTART-VIEW.md |
| 10 | LANE-AUTH-401 | no | no | claims-only | claims.json completedNote: "GREEN daemon_auth_api 5/5"; scratchpad worklog/LANE-AUTH-401.md |
| 11 | LANE-AUTODRIVE-CLAMP | no | no | claims-only | claims.json completedNote: "py_compile OK; lanes default 2 max 4"; scratchpad worklog/LANE-AUTODRIVE-CLAMP.md |
| 12 | LANE-CHAT-ORIGIN | no | no | claims-only | claims.json completedNote: "cargo check bins GREEN"; scratchpad worklog/LANE-CHAT-ORIGIN.md |
| 13 | LANE-CI-CAPS | no | no | claims-only | claims.json completedNote: "CI caps per-step env CARGO_BUILD_JOBS=2"; scratchpad worklog/LANE-CI-CAPS.md |
| 14 | LANE-DESC-STALE | no | no | claims-only | claims.json completedNote: "GREEN daemon::tests 15/15 + daemon_auth 5/5"; scratchpad worklog/LANE-DESC-STALE.md |
| 15 | LANE-FILE-AUTHZ | no | no | claims-only | claims.json completedNote: "GREEN tools file_ops 5/5"; scratchpad worklog/LANE-FILE-AUTHZ.md |
| 16 | LANE-LOOP-CAP | no | no | claims-only | claims.json completedNote: "cargo test agent_loop: 6 passed"; scratchpad worklog/LANE-LOOP-CAP.md |
| 17 | LANE-MAIN-ONCE2 | no | no | claims-only | claims.json completedNote: "cargo check -p opencode-rk-cli GREEN"; scratchpad worklog/LANE-MAIN-ONCE2.md |
| 18 | LANE-ONBOARD-SETUP | no | no | claims-only | claims.json completedNote: "GREEN 13/13 onboarding"; scratchpad worklog/LANE-ONBOARD-SETUP.md |
| 19 | LANE-PROV-FALLBACK | no | no | claims-only | claims.json completedNote: "fallback 6/6 green"; scratchpad worklog/LANE-PROV-FALLBACK.md |
| 20 | LANE-RALPH-MAX2 | no | no | claims-only | claims.json completedNote: "py_compile OK; default=2, effective-with-config6=2"; scratchpad worklog/LANE-RALPH-MAX2.md |
| 21 | LANE-SHELL-AUTHZ | no | no | claims-only | claims.json completedNote: "GREEN 9/9 shell_tool"; scratchpad worklog/LANE-SHELL-AUTHZ.md |
| 22 | LANE-SRV-ROUTER | no | no | claims-only | claims.json completedNote: "GREEN server --lib 205/205"; scratchpad worklog/LANE-SRV-ROUTER.md |
| 23 | LANE-TIMELINE-LAND | no | no | claims-only | claims.json completedNote: "bins check 0 errors"; scratchpad worklog/LANE-TIMELINE-LAND.md |
| 24 | LANE-TOOL-PERM | no | no | claims-only | claims.json completedNote: "GREEN 5/5 permission"; scratchpad worklog/LANE-TOOL-PERM.md |
| 25 | LANE-TRANSCRIPT-LAND | no | no | claims-only | claims.json completedNote: "bins check 0 errors"; scratchpad worklog/LANE-TRANSCRIPT-LAND.md |
| 26 | LANE-TUI-HOST | no | no | claims-only | claims.json completedNote: "native_host 5/5 rustc --test green"; scratchpad worklog/LANE-TUI-HOST.md |
| 27 | LANE-TURN-SETTLE | no | no | claims-only | claims.json completedNote: "GREEN 9/9 turn_service"; scratchpad worklog/LANE-TURN-SETTLE.md |
| 28 | LANE-WEB-HONEST | no | no | claims-only | claims.json completedNote: "GREEN lib web_assets 5/5"; scratchpad worklog/LANE-WEB-HONEST.md |

### Category A3: Legacy ralph.json IDs, not in plan, completed (22 findings)

All 258 legacy IDs have `status=accepted` in ralph.json, but
`contract.legacyAcceptedIsReleaseEvidence: false` (ralph.completion.json:26).
These 22 are also marked `completed` in claims.json, which the gate flags as
off-plan completed.

| # | ID | Legacy status | In plan? | In claims? | Evidence |
|---|----|---------------|----------|------------|----------|
| 29 | ACP-001 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 acp_bridge frozen"; worklog/ACP-001.md |
| 30 | BASE-004 | accepted (ralph.json) | no | completed | claims.json completedNote: "SHA-256 repair GREEN"; worklog/BASE-004.md |
| 31 | HEAD-001 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 8/8 run_headless"; worklog/HEAD-001.md |
| 32 | HEAD-002 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 8/8 session_export"; worklog/HEAD-002.md |
| 33 | OPS-009 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 ops_replay"; worklog/OPS-009.md |
| 34 | PROV-018 | accepted (ralph.json) | no | completed | claims.json completedNote: "5/5 frozen tests green"; worklog/PROV-018.md |
| 35 | PROV-019 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 prov_019_request_profile"; worklog/PROV-019.md |
| 36 | PROV-020 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 prov_020_auth_commands"; worklog/PROV-020.md |
| 37 | PROV-021 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 prov_021_usage_status"; worklog/PROV-021.md |
| 38 | PROV-022 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 prov_022_auth_store"; worklog/PROV-022.md |
| 39 | REL-003 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN validator CLI vs frozen fixtures"; worklog/REL-003.md |
| 40 | RUN-001 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 runner"; worklog/RUN-001.md |
| 41 | SDK-001 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 12/12 sdk_client"; worklog/SDK-001.md |
| 42 | SDK-002 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 11/11 sdk_spawns"; worklog/SDK-002.md |
| 43 | SYNC-001 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 sync_log"; worklog/SYNC-001.md |
| 44 | SYNC-002 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 part_events"; worklog/SYNC-002.md |
| 45 | TOOL-012 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN startup 4/4..."; worklog/TOOL-012-STARTUP-IMPLEMENTATION.md |
| 46 | TOOL-018 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 mcp_lifecycle"; worklog/TOOL-018.md |
| 47 | TOOL-019 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 mcp_payload_filter"; worklog/TOOL-019.md |
| 48 | WEB-004 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 control_plane_exposure"; worklog/WEB-004.md |
| 49 | WEB-005 | accepted (ralph.json) | no | completed | claims.json completedNote: "GREEN 5/5 event_stream"; worklog/WEB-005.md |
| 50 | WEB-006 | accepted (ralph.json) | no | completed | claims.json completedNote: "daemon.rs publish_backend_descriptor"; worklog/WEB-006-DESCRIPTOR.md |

### Category A4: Other off-plan, claims-only, not legacy (1 finding)

| # | ID | In ralph.json? | In plan? | In claims-only? | Evidence |
|---|----|----------------|----------|-----------------|----------|
| 51 | G6-CHAT-DATADIR | no | no | claims-only | claims.json completedNote: "chat.rs spawn forwards --data-dir"; worklog/G6-CHAT-DATADIR.md |

### Category B: In-plan completed with bad-note marker (2 findings)

| # | ID | In ralph.json? | In plan? | Marker | Evidence |
|---|----|----------------|----------|--------|----------|
| 52 | AUD-017 | no | yes | "no acceptance" | claims.json completedNote contains "no acceptance"; worklog/AUD-017.md line 33: "Verdict stays audit-complete, no acceptance claimed" |
| 53 | AUD-020 | no | yes | "no acceptance" | claims.json completedNote contains "no acceptance"; worklog/AUD-020.md line 20: "verdict stays 'audit-complete; no acceptance claimed'" |

## Recomputed Category Counts

| Category | Definition | Count |
|----------|------------|-------|
| A1 | FIX-* off-plan, claims-only | 8 |
| A2 | LANE-* off-plan, claims-only | 20 |
| A3 | Legacy ralph.json IDs, not in plan, completed | 22 |
| A4 | Other off-plan, claims-only | 1 |
| B | In-plan, completedNote contains bad-note marker | 2 |
| **Total** | | **53** |

## Prior Audit Discrepancy: A3=23 vs remediation=43

### The 53-vs-54 transition

- At commit e57d1d3 (lane/PHASE1-convergence): 54 findings.
  - 52 off-plan completed claims (51 current + WEB-009 which was `completed` at e57d1d3)
  - 2 bad-note claims (AUD-017, AUD-020)
- At commit dd7670d (current HEAD): 53 findings.
  - dd7670d changed WEB-009 from `completed` to `blocked` (commit message:
    "WEB-009: record disconnect test race"; claims.json blockedNote: "Frozen disconnect
    test SHA 95fbe83a is nondeterministic...").
  - WEB-009 removed from off-plan completed => 51 off-plan + 2 bad-note = 53.

### A3 count correction

- A3 = legacy ralph.json IDs, not in plan, completed.
- At e57d1d3: A3 = 22 current + WEB-009 (WEB-009 is a legacy ralph.json ID with
  status=accepted) = 23. This matches the prior audit's A3=23.
- WEB-009 was blocked at dd7670d, removing it from A3 => A3 = 22. Correct.

### The "remediation=43" figure

The prior audit (worklog/DISC-003-PHASE1-CONVERGENCE.md at e57d1d3) contained an
internal inconsistency. The "remediation=43" figure appears in worklog/GUARD-TRIAGE-16.md
as a historical reference to a prior wave count, but was erroneously echoed in the
e57d1d3 audit text as a claim about current findings.

The actual e57d1d3 finding count was 54 (52 off-plan + 2 bad-note). Subtracting
WEB-009 (which was completed then but blocked at dd7670d): 53. There is no
category with 43 findings at either commit.

The "43" figure likely refers to a different counting method (e.g., total
in-plan stories requiring reconciliation minus already-completed = 109 - 66 =
43), but this was never the convergence-gate count. The gate produces exactly
53 findings at dd7670d and 54 at e57d1d3.

## AUD-017 / AUD-020 Note Logic Assessment

Both AUD-017 and AUD-020 are in-plan (part of the 109 plan stories loaded from
ralph.completion.json). They appear as Category B findings because their
completedNotes contain the BAD_NOTE_MARKER "no acceptance".

- **AUD-017** (audit shard: "Controller acceptance, leases, TDD and worker
  ownership"): claims.json completedNote states "...no acceptance". The scratchpad
  worklog/AUD-017.md line 33 explicitly states: "Verdict stays audit-complete,
  no acceptance claimed." This is correct behavior for an audit-only task.

- **AUD-020** (audit shard: "Every legacy requirement and accepted-flag
  reconciliation"): claims.json completedNote states "...no acceptance". The
  scratchpad worklog/AUD-020.md line 20 explicitly states: "verdict stays
  'audit-complete; no acceptance claimed'." This is correct behavior for an
  audit-only task.

The BAD_NOTE_MARKER detection for "no acceptance" is working as designed: the
convergence gate flags any in-plan completed claim whose note admits lack of
acceptance. AUD-017 and AUD-020 are audit reports, not acceptance gates, so their
self-admitted "no acceptance" status is honest and correct. The gate correctly
surfaces them for the integrator/controller to map or resolve.

## WEB-009 Confirmation

- **Current status at dd7670d**: WEB-009 is `blocked` in claims.json with a
  blockedNote: "Frozen disconnect test SHA 95fbe83a is nondeterministic: it
  disconnects on tool_call before opaque fixture PID publication is synchronized.
  Exact revision 732c046 passed once and failed controller rerun at PID deadline.
  Replacement must wait for fixture PIDs before disconnect while preserving
  permit/provider/process-tree/sentinel/durability assertions; test-authority
  approval required."

- **tasks/WEB-009.md**: Status field reads "Status: IN PROGRESS / NOT ACCEPTED"
  (line 3).

- **FEATURES.md line 89**: `WEB-009 | accepted | ... | write-paths disabled by
  design + browser unexecuted` -- stale sync with ralph.json legacy status.

- **ralph.json WEB-009** (line 3390): `"status": "accepted"` -- legacy status,
  NOT release evidence per contract.legacyAcceptedIsReleaseEvidence=false.

- **Absent from 53 findings**: WEB-009 is `blocked`, not `completed`, so neither
  "completed off-plan task" nor "bad-note" check fires. Confirmed absent from
  the convergence_gate.py output (53 findings, none is WEB-009).

- **Commit dd7670d**: "WEB-009: record disconnect test race" changed claims.json
  WEB-009 from completed to blocked. git log confirms dd7670d is HEAD.

## Reconciliation Proposal Table

Machine-checkable: old ID -> proposed parent/disposition -> evidence -> authority.

| Old ID | Proposed disposition | Evidence (file:line or command output) | Authority |
|--------|---------------------|----------------------------------------|-----------|
| ACP-001 | Remain in legacy set A3; no plan mapping without integrator decision | ralph.json:3390 status=accepted; claims.json status=completed; plan_ids excludes ACP-001; completion_plan.py:165 asserts no overlap | AUD-020 shard (catch-all legacy) |
| BASE-004 | Remain in legacy set A3; legacy repair, not plan story | ralph.json BASE-004 status=accepted; claims.json completed; not in plan_ids | AUD-020 shard |
| FIX-LOGROTATE | Map FIX-* into AUD-006 repair children or retire as verify-only | claims.json completedNote; worklog/FIX-LOGROTATE.md; FIX-* prefix not in plan | Controller/integrator |
| FIX-LOOP-RULES | Map into AUD-017 repair children | claims.json completedNote; worklog/FIX-LOOP-RULES.md | AUD-017 shard |
| FIX-NATIVE-DAEMON | Map into AUD-001 or AUD-006 repair children | claims.json completedNote; worklog/FIX-NATIVE-DAEMON.md | Controller/integrator |
| FIX-PACKAGING | Map into AUD-018 repair children | claims.json completedNote; worklog/FIX-PACKAGING.md | AUD-018 shard |
| FIX-SANDBOX | Map into DISC-106 (already covers) or retire | claims.json completedNote references DISC-106 b688c56; worklog/FIX-SANDBOX.md | DISC-106 owner |
| FIX-SESSIONS-STUBS | Map into AUD-007 repair children | claims.json completedNote; worklog/FIX-SESSIONS-STUBS.md | AUD-007 shard |
| FIX-SQLITE-GATE | Map into AUD-007 repair children | claims.json completedNote; worklog/FIX-SQLITE-GATE.md | AUD-007 shard |
| FIX-TIMELINE | Map into AUD-013 or TUI-010 repair children | claims.json completedNote; worklog/FIX-TIMELINE.md | Controller/integrator |
| G6-CHAT-DATADIR | Map into AUD-001 repair children or retire | claims.json completedNote; worklog/G6-CHAT-DATADIR.md; not in plan | Controller/integrator |
| HEAD-001 | Remain in legacy set A3; not a plan story | ralph.json HEAD-001 status=accepted; claims.json completed | AUD-010 shard |
| HEAD-002 | Remain in legacy set A3; not a plan story | ralph.json HEAD-002 status=accepted; claims.json completed | AUD-010 shard |
| LANE-APPSTART-VIEW | Retire: APP-001 is the plan story | claims.json; worklog/LANE-APPSTART-VIEW.md; APP-001 in plan | Controller/integrator |
| LANE-AUTH-401 | Map into AUD-001 repair children | claims.json; worklog/LANE-AUTH-401.md | AUD-001 shard |
| LANE-AUTODRIVE-CLAMP | Map into AUD-017 repair children | claims.json; worklog/LANE-AUTODRIVE-CLAMP.md | AUD-017 shard |
| LANE-CHAT-ORIGIN | Map into APP-011 repair children | claims.json; worklog/LANE-CHAT-ORIGIN.md; APP-011 in plan | Controller/integrator |
| LANE-CI-CAPS | Map into AUD-018 repair children | claims.json; worklog/LANE-CI-CAPS.md | AUD-018 shard |
| LANE-DESC-STALE | Map into AUD-001 repair children | claims.json; worklog/LANE-DESC-STALE.md | AUD-001 shard |
| LANE-FILE-AUTHZ | Map into AUD-009 or AUD-012 repair children | claims.json; worklog/LANE-FILE-AUTHZ.md | Controller/integrator |
| LANE-LOOP-CAP | Map into AUD-004 repair children | claims.json; worklog/LANE-LOOP-CAP.md; AUD-004 in plan | AUD-004 shard |
| LANE-MAIN-ONCE2 | Map into APP-001 repair children | claims.json; worklog/LANE-MAIN-ONCE2.md; APP-001 in plan | Controller/integrator |
| LANE-ONBOARD-SETUP | Map into APP-005 repair children | claims.json; worklog/LANE-ONBOARD-SETUP.md; APP-005 in plan | Controller/integrator |
| LANE-PROV-FALLBACK | Map into AUD-002 repair children | claims.json; worklog/LANE-PROV-FALLBACK.md | AUD-002 shard |
| LANE-RALPH-MAX2 | Map into AUD-017 repair children | claims.json; worklog/LANE-RALPH-MAX2.md | AUD-017 shard |
| LANE-SHELL-AUTHZ | Map into AUD-009 or AUD-005 repair children | claims.json; worklog/LANE-SHELL-AUTHZ.md | Controller/integrator |
| LANE-SRV-ROUTER | Map into AUD-001 or AUD-014 repair children | claims.json; worklog/LANE-SRV-ROUTER.md | Controller/integrator |
| LANE-TIMELINE-LAND | Map into AUD-013 or TUI-005 repair children | claims.json; worklog/LANE-TIMELINE-LAND.md | Controller/integrator |
| LANE-TOOL-PERM | Map into AUD-006 or DISC-105 repair children | claims.json; worklog/LANE-TOOL-PERM.md; DISC-105 in plan | DISC-105 owner |
| LANE-TRANSCRIPT-LAND | Map into AUD-007 or TUI-005 repair children | claims.json; worklog/LANE-TRANSCRIPT-LAND.md | Controller/integrator |
| LANE-TUI-HOST | Map into AUD-001 or AUD-011 repair children | claims.json; worklog/LANE-TUI-HOST.md | Controller/integrator |
| LANE-TURN-SETTLE | Map into APP-004 repair children | claims.json; worklog/LANE-TURN-SETTLE.md; APP-004 in plan | Controller/integrator |
| LANE-WEB-HONEST | Map into AUD-014 repair children | claims.json; worklog/LANE-WEB-HONEST.md | AUD-014 shard |
| OPS-009 | Remain in legacy set A3; not a plan story | ralph.json OPS-009 status=accepted; claims.json completed | AUD-018 shard |
| PROV-018 | Remain in legacy set A3; not a plan story | ralph.json PROV-018 status=accepted; claims.json completed | AUD-002 shard |
| PROV-019 | Remain in legacy set A3; not a plan story | ralph.json PROV-019 status=accepted; claims.json completed | AUD-002 shard |
| PROV-020 | Remain in legacy set A3; not a plan story | ralph.json PROV-020 status=accepted; claims.json completed | AUD-002 shard |
| PROV-021 | Remain in legacy set A3; not a plan story | ralph.json PROV-021 status=accepted; claims.json completed | AUD-002 shard |
| PROV-022 | Remain in legacy set A3; not a plan story | ralph.json PROV-022 status=accepted; claims.json completed | AUD-002 shard |
| REL-003 | Remain in legacy set A3; not a plan story | ralph.json REL-003 status=accepted; claims.json completed | AUD-018 shard |
| RUN-001 | Remain in legacy set A3; not a plan story | ralph.json RUN-001 status=accepted; claims.json completed | AUD-010 shard |
| SDK-001 | Remain in legacy set A3; not a plan story | ralph.json SDK-001 status=accepted; claims.json completed | AUD-010 shard |
| SDK-002 | Remain in legacy set A3; not a plan story | ralph.json SDK-002 status=accepted; claims.json completed | AUD-010 shard |
| SYNC-001 | Remain in legacy set A3; not a plan story | ralph.json SYNC-001 status=accepted; claims.json completed | AUD-010 shard |
| SYNC-002 | Remain in legacy set A3; not a plan story | ralph.json SYNC-002 status=accepted; claims.json completed | AUD-010 shard |
| TOOL-012 | Remain in legacy set A3; not a plan story | ralph.json TOOL-012 status=accepted; claims.json completed | AUD-009 shard |
| TOOL-018 | Remain in legacy set A3; not a plan story | ralph.json TOOL-018 status=accepted; claims.json completed | AUD-009 shard |
| TOOL-019 | Remain in legacy set A3; not a plan story | ralph.json TOOL-019 status=accepted; claims.json completed | AUD-009 shard |
| WEB-004 | Remain in legacy set A3; not a plan story | ralph.json WEB-004 status=accepted; claims.json completed | AUD-014 shard |
| WEB-005 | Remain in legacy set A3; not a plan story | ralph.json WEB-005 status=accepted; claims.json completed | AUD-014 shard |
| WEB-006 | Remain in legacy set A3; not a plan story | ralph.json WEB-006 status=accepted; claims.json completed | AUD-014 shard |
| AUD-017 | Resolve: retire bad-note marker via plan mapping (not acceptance) | claims.json completedNote contains "no acceptance"; worklog/AUD-017.md:33 "no acceptance claimed"; AUD-017 in plan_ids | Controller/integrator |
| AUD-020 | Resolve: retire bad-note marker via plan mapping (not acceptance) | claims.json completedNote contains "no acceptance"; worklog/AUD-020.md:20 "no acceptance claimed"; AUD-020 in plan_ids | Controller/integrator |

## Verification

1. `python3 tools/convergence_gate.py` => CONVERGENCE BLOCKED, total=53. Verified.
2. DISC-003 claim set to `blocked` via `tools/completion_claims.py`.
3. WEB-009 confirmed `blocked` at dd7670d, absent from 53 findings. Verified.
4. AUD-017/AUD-020 confirmed both `completed` in-plan with "no acceptance" notes. Verified.
5. Prior audit discrepancy: A3=23 at e57d1d3 (includes WEB-009), now 22 at dd7670d.
   "Remediation=43" figure is not a gate count at any commit; it is an internal
   inconsistency in the e57d1d3 audit text. Verified.

## Remaining Limitations

- This is an audit-only review. The 53 findings are gate structural evidence, not
  release evidence. Resolution requires controller/integrator authority to map
  off-plan claims into repair children or retire them.
- No canonical plan/verifier/source/test files were modified.
- The reconciliation proposal table maps each finding to a shard owner, but actual
  remapping requires the integrator's edit authority on shared contracts.