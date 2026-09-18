# Worklog PAR-001-discovered (planner lane)

Claim: created mandatory discovery children file `tasks/completion/discovered.json` with 19 stories (DISC-101..119), each one owned file, 5 concrete tests, deps on AUD shards + COORD-001.

Source evidence (HEAD 5af7884):
- `docs/audits/2026-09-17-app-completion.md` gaps 1-8 (no default journey; line UI not OpenTUI; subsystems not one engine; accept-before-integrate; batch barrier; identity not ownership; TBD plan; presence not proof).
- `sources/completion/audits/AUD-*.json` repairChildren fields, all 20 read; findings classes missing/unwired/unverified mapped per child.
- `ralph.completion.json` requirements COMP-001..007; contract discovery clause.
- Existing includes inventoried: 70 stories; all paths indexed to guarantee no overlap; legacy ralph.json has DISC-001/008/010 only, so DISC-101..119 collide with nothing.
- ID regex from `tools/completion_plan.py:20` (`[A-Z]+-[0-9]{3}`) satisfied.

Decisions:
- Deps use real AUD-001..017 shard IDs (load() synthesizes AUD rows, so they resolve) plus COORD-001 for controller child.
- requirementIds: COMP-001 for daemon/command children, COMP-002 for native shell, COMP-004 for share/pairing, COMP-005 for mobile freeze, COMP-003 default; COMP-006 for controller child.
- Mobile framework freeze is a config-file lane (COMP-005 sole authority, per AUD-016 repair note).
- Batch-refill fix left to COORD-002 (owned there); DISC-119 covers accept-before-integrate + lock table + lane gate.

Verification:
- `timeout 30 python3 tools/completion_plan.py --check` still passes: SPEC OK additions=90 legacy=258 (discovered.json not in includes, so check unaffected by design).
- Standalone schema simulation: 19 stories, IDs unique vs includes+legacy, >=5 tests each, deps resolve, single non-overlapping paths, valid requirementIds. ERRORS: NONE.

Remaining unknowns: integrator must add discovered.json to includes (or wire children into PAR-001) for load() to enforce them; until then they are advisory, not gated.
