# DISC-003 PHASE1 Convergence Audit

## Task ID
DISC-003

## Session
ses_f33319188ffe45W7jd4C8he6JS

## Base commit
732c04652054438fd823fb264f2f18c4cb083541 (lane/PHASE1-convergence)

## Objective
Audit DISC-003 convergence findings from `convergence_gate.py`, classify them by cause
and authority path, and mark DISC-003 blocked.

## Gate Output
```
CONVERGENCE BLOCKED
total=54
```
52 off-plan completed claims + 2 bad-note completed claims = 54 findings.

## Authority Note
DISC-003 is NOT in the plan story set (ralph.completion.json `stories`). It is NOT in
ralph.json legacy userStories either. It is a convergence audit task, not an acceptance task.
DISC-003 only audits and reports; it cannot resolve other claims.

## Plan Story Context
- `ralph.completion.json` schema v1 defines 109 plan stories via `completion_plan.load()`.
- `plan_ids()` returns the 109 story IDs from `plan['stories']`.
- `legacyStories` is a separate 258-row set derived from `ralph.json` (legacy status is
  NOT release evidence per `contract.legacyAcceptedIsReleaseEvidence = false`).
- DISC-003 appears in claims.json but is NOT in plan_ids or legacyStories.

## WEB-009 Branch Discrepancy

### Base branch (732c046 / lane/PHASE1-convergence)
- `claims.json` WEB-009: **status=completed**, `completedNote` states GREEN 1/1
  `runtime_wiring_disconnect_http`.
- This is the FALSE COMPLETION: the disconnect test is standalone (runtime_wiring only),
  not wired into the parent WEB-009 acceptance path which requires durable turn events,
  references, accessibility gaps, and provider-turn-stream integration.
- `tasks/WEB-009.md` Status field: "IN PROGRESS / NOT ACCEPTED"
- `ralph.json` userStories WEB-009: status="accepted" (STALE - legacy status is not release evidence)
- `FEATURES.md` line 89: shows WEB-009 as "accepted" with "write-paths disabled by design +
  browser unexecuted" (stale sync with ralph.json, not independent evidence)

### Review branch dd7670d (WEB-009: record disconnect test race)
- `claims.json` WEB-009: **status=blocked** with `blockedNote`:
  "Frozen disconnect test SHA 95fbe83a is nondeterministic: it disconnects on tool_call
  before opaque fixture PID publication is synchronized. Exact revision 732c046 passed once
  and failed controller rerun at PID deadline. Replacement must wait for fixture PIDs before
  disconnect while preserving permit/provider/process-tree/sentinel/durability assertions;
  test-authority approval required."
- This is the corrected authority state. The base branch's completed status is overridden.

The base branch 732c046 carries the FALSE WEB-009 completed claim. The review branch dd7670d
corrects it to blocked. The convergence gate sees the base branch state.

## Finding Classification

### Category A: Off-plan completed claims (52 findings)

These are claims with `status=completed` whose task ID is NOT in `plan_ids()` (109 plan stories).
They are completed against legacy/repair/lane scopes that the new completion plan does not
enumerate as mandatory, so any completed status outside the plan is flagged.

#### A1: Legacy/repair fixes not in plan (8 findings)
FIX- prefixed claims that are repair work, not plan stories:

| ID | Session | Scratchpad | CompletedNote (truncated) |
|---|---|---|---|
| FIX-LOGROTATE | ses_f40e83c32ffeYf1LA0bmr0DLkp | worklog/FIX-LOGROTATE.md | VERIFY-ONLY 7/7 diagnostics GREEN oc2 bin |
| FIX-LOOP-RULES | ses_fix_looprules | worklog/FIX-LOOP-RULES.md | GREEN 8/8 rules_globs t01-t08 |
| FIX-NATIVE-DAEMON | ses_fix_natdaemon | worklog/FIX-NATIVE-DAEMON.md | VERIFY-ONLY zero-diff: run_with_dir+stdin guard |
| FIX-PACKAGING | ses_fix_pkg | worklog/FIX-PACKAGING.md | --help identity gate: bash-n OK |
| FIX-SANDBOX | ses_orch_fix1 | worklog/FIX-SANDBOX.md | covered by external DISC-106 b688c56 |
| FIX-SESSIONS-STUBS | ses_f40b69bc1ffeFIMSlnp8tT1VwE | worklog/FIX-SESSIONS-STUBS.md | snapshot.rs build_snapshot real fold |
| FIX-SQLITE-GATE | ses_fix_sqlite | worklog/FIX-SQLITE-GATE.md | Backports rejected without audited allowlist |
| FIX-TIMELINE | ses_f40321fd7ffeHN2p2HkWBucRbO | worklog/FIX-TIMELINE.md | GREEN 11/11 native_timeline |

**Legitimacy**: All are off-plan by definition. FIX-* tags denote repair/safety fixes
outside the 109-story completion gate. They pass frozen tests but cannot be accepted
as release evidence without controller/integrator reconciliation.

#### A2: LANE-* prefixed claims (20 findings)
Completed lane work not promoted to plan stories:

| ID | Session | Scratchpad | CompletedNote (truncated) |
|---|---|---|---|
| LANE-APPSTART-VIEW | ses_f423ccf75ffe72BSE5xLbs3Rf5 | worklog/LANE-APPSTART-VIEW.md | GREEN 16/16 standalone rustc app_start |
| LANE-AUTH-401 | ses_f423cd0afffeIbW7EsUV3TUvIA | worklog/LANE-AUTH-401.md | GREEN daemon_auth_api 5/5 |
| LANE-AUTODRIVE-CLAMP | ses_f423ccebbffe34G50hL9WgxgWo | worklog/LANE-AUTODRIVE-CLAMP.md | py_compile OK; lanes default 2 max 4 |
| LANE-CHAT-ORIGIN | ses_f423cd0e6ffeUlSuxlJpbTBk5Y | worklog/LANE-CHAT-ORIGIN.md | cargo check bins GREEN |
| LANE-CI-CAPS | ses_f423ccebdffemjzOag67siYSxl | worklog/LANE-CI-CAPS.md | CI caps per-step env CARGO_BUILD_JOBS=2 |
| LANE-DESC-STALE | ses_f423cd0adffe1mKflQIPCPcvT5 | worklog/LANE-DESC-STALE.md | GREEN daemon::tests 15/15 |
| LANE-FILE-AUTHZ | ses_f423cd0dcffeKV0Mtlfh4TMwp2 | worklog/LANE-FILE-AUTHZ.md | GREEN tools file_ops 5/5 |
| LANE-LOOP-CAP | ses_f423cceb3ffe8DP2G3ZFzAOT5p | worklog/LANE-LOOP-CAP.md | cargo test agent_loop: 6 passed |
| LANE-MAIN-ONCE2 | ses_f423cd0e9ffeB95oRHX3HpaS0Y | worklog/LANE-MAIN-ONCE2.md | cargo check -p opencode-rk-cli GREEN |
| LANE-ONBOARD-SETUP | ses_f423cd0e5ffeeHMozSNUJfQ3zq | worklog/LANE-ONBOARD-SETUP.md | GREEN 13/13 onboarding |
| LANE-PROV-FALLBACK | ses_f423cd0daffezwT8o66LUJl0eU | worklog/LANE-PROV-FALLBACK.md | fallback 6/6 green |
| LANE-RALPH-MAX2 | ses_f423cceb8ffeWknMXoafTgKEZd | worklog/LANE-RALPH-MAX2.md | py_compile OK; default=2 max 4 |
| LANE-SHELL-AUTHZ | ses_f423cd0dfffenzXnM7HZSns8eh | worklog/LANE-SHELL-AUTHZ.md | GREEN 9/9 shell_tool |
| LANE-SRV-ROUTER | ses_f423cd0e2ffejOqhPZZSuaZbMD | worklog/LANE-SRV-ROUTER.md | GREEN server --lib 205/205 |
| LANE-TIMELINE-LAND | ses_f423ccec1ffeAs18vWOX0n8DYQ | worklog/LANE-TIMELINE-LAND.md | bins check 0 errors |
| LANE-TOOL-PERM | ses_f423cd0b1ffevAoq08DEYzAc6W | worklog/LANE-TOOL-PERM.md | GREEN 5/5 permission |
| LANE-TRANSCRIPT-LAND | ses_f423ccf72ffeIVRuzGDXMAMDAX | worklog/LANE-TRANSCRIPT-LAND.md | bins check 0 errors |
| LANE-TUI-HOST | ses_f425545efffeG8Seym5RbOamQf | worklog/LANE-TUI-HOST.md | native_host 5/5 rustc green |
| LANE-TURN-SETTLE | ses_f423cceb6ffeLkm86YxBA2Z5YW | worklog/LANE-TURN-SETTLE.md | GREEN 9/9 turn_service |
| LANE-WEB-HONEST | ses_f423cceb0ffeKndZg63lApY3Fy | worklog/LANE-WEB-HONEST.md | GREEN lib web_assets 5/5 |

**Legitimacy**: Lane work is isolated leaf scope. None are plan stories. Completion is
wiring-isolated and requires integrator reconciliation to promote to plan acceptance.

#### A3: In ralph.json but NOT in completion plan (23 findings)
These task IDs existed as accepted in the legacy ralph.json but are not migrated into
the 109-story `ralph.completion.json` plan (`story` entries):

| ID | Session | Scratchpad | CompletedNote (truncated) |
|---|---|---|---|
| ACP-001 | ses_f384fedeaffeiMPH6Dl2h3q5Ox | worklog/ACP-001.md | GREEN 5/5 acp_bridge frozen |
| BASE-004 | ses_f380c4bc9ffei2XDG2DStCM92e | worklog/BASE-004.md | SHA-256 repair GREEN: daemon_long_path |
| HEAD-001 | ses_f384fee05ffe9ZbohjUUo6uif | worklog/HEAD-001.md | GREEN 8/8 run_headless |
| HEAD-002 | ses_f384fedf9ffeu54QwccVGqCbzv | worklog/HEAD-002.md | GREEN 8/8 session_export |
| OPS-009 | ses_f387af8a9ffe7GarMQ3qCTI8wr | worklog/OPS-009.md | GREEN 5/5 ops_replay |
| PROV-018 | ses_f389670a7ffeuc9N40EFR1AM4H | worklog/PROV-018.md | 5/5 frozen tests green |
| PROV-019 | ses_f386f5b1fffeYqbqZ1uC13paZz | worklog/PROV-019.md | GREEN 5/5 prov_019_request_profile |
| PROV-020 | ses_f384fee27ffe1QKjki0oZQsHrQ | worklog/PROV-020.md | GREEN 5/5 prov_020_auth_commands |
| PROV-021 | ses_f384fee20ffertocB89srK0rm7 | worklog/PROV-021.md | GREEN 5/5 prov_021_usage_status |
| PROV-022 | ses_f387af8bdffeEySGQi8lJSC21p | worklog/PROV-022.md | GREEN 5/5 prov_022_auth_store |
| REL-003 | ses_f387af899ffejohREiG4Rewoar | worklog/REL-003.md | GREEN validator CLI vs frozen fixtures |
| RUN-001 | ses_f387af8bcffeojhREiG4Rewoar | worklog/RUN-001.md | GREEN 5/5 runner |
| SDK-001 | ses_f384fede9ffer5DDd9SzW9hr44 | worklog/SDK-001.md | GREEN 12/12 sdk_client |
| SDK-002 | ses_f384fede7ffenpg6X2t2lZtIyS | worklog/SDK-002.md | GREEN 11/11 sdk_spawns |
| SYNC-001 | ses_f387af8b7ffeg2GsF4ErYq07AX | worklog/SYNC-001.md | GREEN 5/5 sync_log |
| SYNC-002 | ses_f384fedd2ffeFXapK9O5i12eT7 | worklog/SYNC-002.md | GREEN 5/5 part_events |
| TOOL-012 | ses_f338c638dffeNOgL4RCjogVLIi | worklog/TOOL-012-STARTUP-IMPLEMENTATION.md | GREEN startup 4/4 |
| TOOL-018 | ses_f384fedaeffenNesgxTRphfrXU | worklog/TOOL-018.md | GREEN 5/5 mcp_lifecycle |
| TOOL-019 | ses_f384fec49ffeN1ZefbdMWO6lwv | worklog/TOOL-019.md | GREEN 5/5 mcp_payload_filter |
| WEB-004 | ses_f387af8aaffeFWlAQiQn1gYKIU | worklog/WEB-004.md | GREEN 5/5 control_plane_exposure |
| WEB-005 | ses_f387af8abffeg2GsF4ErYq07AX | worklog/WEB-005.md | GREEN 5/5 event_stream |
| WEB-006 | ses_f37d1ed3affeMszns7VKYpWhOw | worklog/WEB-006-DESCRIPTOR.md | daemon.rs publish_backend_descriptor |
| WEB-009 | ses_f3c4de578ffelQv59xDXmOs03B | worklog/WEB-009-DISCONNECT-GREEN.md | GREEN 1/1 runtime_wiring_disconnect_http |

**Legitimacy**: All exist in ralph.json legacy (258 stories). All are "accepted" in
legacy ralph.json status. But legacy status is NOT release evidence (`contract.legacyAcceptedIsReleaseEvidence=false`).
None are migrated into the 109-story ralph.completion.json plan. The completion gate
only validates against plan_ids, so these are all off-plan.

#### A4: Other off-plan (1 finding)

| ID | Session | Scratchpad | CompletedNote (truncated) |
|---|---|---|---|
| G6-CHAT-DATADIR | ses_f425545f6ffe4N8Iq6BNvdwYSC | worklog/G6-CHAT-DATADIR.md | chat.rs spawn forwards --data-dir; cargo check |

**Legitimacy**: Not a plan story ID (does not match `[A-Z]+-[0-9]{3}` pattern). Off-plan.

### Category B: Bad-note completed claims (2 findings)

These ARE in plan_ids but their own `completedNote` admits marker phrases that the
gate treats as false-completion confessions:

| ID | Session | Scratchpad | Marker | Full completedNote |
|---|---|---|---|---|
| AUD-017 | ses_f420f8732ffe5Rv2Nxle3iSLgV | worklog/AUD-017.md | "no acceptance" | audit-complete refresh at 1614754: SHAPE OK 8 findings/5 repairs; test_auto* 16run/6fail + leases_validate 18run/2fail (frozen drift, pre-existing); plan-check SPEC OK 109/258/545; zero test edits, no acceptance |
| AUD-020 | ses_f41f2c26cffeFFRai2y6T8LbZo | worklog/AUD-020.md | "no acceptance" | AUD-020 refresh at 10154: export 258 legacy/258 accepted/237 empty-deps/82 TBD +109 completion; audit-legacy requiresRevalidation all 258; check SPEC OK 109/258/545; JSON shape keys unchanged, verdict no acceptance; zero test edits |

**Legitimacy**: Both are in the 109-story plan. AUD-017 is an audit controller shard;
AUD-020 is the legacy reconciliation audit shard. Their own notes explicitly state
"no acceptance" - they audit and report, they do not accept. This is self-admitted.
The plan allows completion (audit done), but the notes correctly deny acceptance authority.

### Category C: WEB-009 False Completion (1 finding, counted within A3)

WEB-009 is the critical finding in A3. It shows `status=completed` on the base branch
732c046 but:

1. **`tasks/WEB-009.md`**: Status field says "IN PROGRESS / NOT ACCEPTED"
2. **`ralph.json`**: Shows status="accepted" but legacy status is NOT release evidence
3. **`FEATURES.md`** line 89: Shows "accepted" with caveat "write-paths disabled by design +
   browser unexecuted" - this is a stale sync with ralph.json
4. **`worklog/WEB-009.md`** (task-level worklog): Status: BLOCKED, with evidence that the
   full WEB-009 user-observable outcome is NOT implemented. Parent remains NOT ACCEPTED.
5. **`worklog/WEB-009-DISCONNECT-GREEN.md`**: The completedNote scratchpad only covers the
   disconnect sub-contract. Explicitly states "Parent remains NOT ACCEPTED for durable
   tool/reference/accessibility gaps."
6. **`crates/server/src/runtime_wiring.rs:301-357`**: `RuntimeWiring` has no caller in
   product code; only its own tests construct it. The disconnect test exercises
   `runtime_wiring_disconnect_http` in isolation, not wired into the daemon HTTP path.

The base branch 732c046 commit message is:
"WEB-009: disconnect cancellation via pinned ShellTool future"

This is a **sub-contract** landing, not the full WEB-009 acceptance. The review branch
dd7670d (commit dd7670d "WEB-009: record disconnect test race") correctly moved WEB-009
to `blocked` with a detailed `blockedNote` about test nondeterminism.

**Authority needed**: Controller/verifier must re-evaluate WEB-009's completed status.
The disconnect sub-contract is verified GREEN, but the parent WEB-009 user-observable
outcome (structured reasoning summaries, references section, accessibility gaps) remains
NOT ACCEPTED. Either:
- Accept WEB-009 based on full evidence (requires parent wiring + independent verifier), OR
- Revert WEB-009 to blocked (matching dd7670d branch), OR
- Promote WEB-009 disconnect sub-contract to its own plan story ID

### Category D: False Positives (0 findings)

All 54 findings are legitimate. No false positives detected.

## Summary Table

| Category | Count | Description | Legitimacy | Authority Needed |
|---|---|---|---|---|
| A1 | 8 | FIX-* repair claims off-plan | Legitimate - off-plan by gate definition | Integrator to reconcile or de-scope |
| A2 | 20 | LANE-* lane claims off-plan | Legitimate - isolated lane GREENs | Integrator to promote/wire into plan |
| A3 | 23 | ralph.json accepted but not in plan | Legitimate - legacy status not evidence | Controller to migrate or de-scope |
| A4 | 1 | G6-CHAT-DATADIR off-plan | Legitimate - non-standard ID | Integrator to classify |
| B | 2 | AUD-017, AUD-020 bad-note | Legitimate - self-admitted "no acceptance" | These are audit reports, not acceptance | 
| C | 1 | WEB-009 false completion | Legitimate false completion | Controller/verifier to resolve; dd7670d has blocked state |
| **Total** | **54** | | All legitimate | Controller/integrator authority |

## Required Authority to Resolve

DISC-003 is a **convergence audit only**. It cannot resolve any findings. To unblock:

1. **Controller/integrator**: Migrate the 43 legacy ralph.json stories (A3 minus WEB-009)
   into ralph.completion.json plan stories, or de-scope them as out-of-release-scope.
2. **Controller/integrator**: Reconcile 20 LANE-* claims (A2) - either promote to plan stories
   or accept as completed leaf scope.
3. **Controller/integrator**: Reconcile 8 FIX-* claims (A1) - repair fixes outside plan.
4. **Controller/integrator**: Validate AUD-017 and AUD-020 (B) - their self-admitted "no
   acceptance" status is correct; they are audit reports, not acceptance gates.
5. **Controller/verifier**: Resolve WEB-009 (C) - either:
   - Accept WEB-009 based on full parent wiring evidence, OR
   - Revert to blocked (matching dd7670d), OR
   - Split into separate plan story IDs for disconnect sub-contract vs full acceptance

## Branch State

- Current branch: lane/PHASE1-convergence @ 732c046
- WEB-009 on 732c046: completed (FALSE - disconnect-only)
- WEB-009 on dd7670d: blocked (CORRECT - nondeterministic test + parent NOT ACCEPTED)
- DISC-003 claim: marked blocked with this audit as evidence

## Disk Status
- Scratchpad: `worklog/DISC-003-PHASE1-CONVERGENCE.md` (written)
- Claims ledger: DISC-003 updated to `blocked` with `blockedNote` (via completion_claims.py)

## No Acceptance Claim
DISC-003 does not claim acceptance of any task. This is an audit report only.
