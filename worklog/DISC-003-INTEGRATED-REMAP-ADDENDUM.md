# DISC-003 integrated remap addendum

## Boundary

- Integrated candidate before this addendum: `3ba4cf967c66925def3f6b53f496460a7c863db6`.
- Candidate ledger SHA-256: `6c8cf2d8691bbd339bc54639ca82cf022f6cad9d10dfba878f70c3bdef572f99`.
- The prior 53-row proposal is intentionally hash-locked to `9c4fb6b` and MUST
  NOT be applied to this rebased candidate.
- No canonical status, accepted flag, plan, verifier, or policy is changed here.

## Integrated finding delta

`python3 tools/convergence_gate.py` reports 80 findings: 78 off-plan completed
claims plus the existing two in-plan bad-note rows (`AUD-017`, `AUD-020`). The
original 51 off-plan rows remain, and current `origin/main` contributes these 27:

```text
LANE-AGENT-FILES=AUD-004
LANE-CI=AUD-018
LANE-CI-EXT=AUD-018
LANE-CI-FLAG=AUD-018
LANE-COMMANDS-LIVE=AUD-012
LANE-CONTEXT-ACCOUNT=AUD-003
LANE-CONTEXT-CMD=AUD-003
LANE-DISPATCH-DENY=AUD-005
LANE-GLOBS=AUD-012
LANE-GLOBS-LIVE=AUD-012
LANE-LOOP=AUD-004
LANE-LOOP-LIVE=AUD-004
LANE-MCP-LIVE=AUD-009
LANE-RULES=AUD-012
LANE-SANDBOX=AUD-006
LANE-SUBAGENT-LIVE=AUD-004
LANE-THEMES=AUD-013
LANE-TUI-GRAPH=AUD-013
LANE-ULTRA-CODEGEN=AUD-004
LANE-WEB-CANVAS=AUD-014
LANE-WF-CREATE=AUD-004
LANE-WF-TIMELINE=AUD-013
RC-01=AUD-001
RC-02=AUD-001
RC-03=AUD-001
WEB-EVENT-STREAM=AUD-014
WEB-HINT=AUD-014
```

These are evidence assignments, not parent acceptance. In particular,
`LANE-CI-EXT` admits one gated pre-existing regression in its completion note;
retiring the alias cannot make that parent or journey complete.

## Authority algorithm delta

On the exact hash above, the designated controller/integrator may use the prior
proposal algorithm after adding the 27 IDs above to its `retire` set and changing
`assert len(retire) == 51` to `assert len(retire) == 78`. The same safeguards
remain mandatory:

1. Verify the ledger SHA before any mutation.
2. Assert all 78 rows exist and are `completed`.
3. Remove only those 78 alias rows from the active claims ledger; Git history and
   worklogs remain evidence.
4. Change only `AUD-017` and `AUD-020` from `completed` to `blocked`, preserving
   their notes as `blockedNote`.
5. Write a candidate file first and review the exact diff.
6. Run both `tools/convergence_gate.py` and `tools/validate_repository.py`.

Passing the convergence gate is structural only. Backlog exhaustion, protected
repository settings, frozen integrated journeys, and external release evidence
remain independent blockers.

## Disposable simulation

The 78-row retirement plus the two audit demotions was applied only in a detached
disposable worktree at `3ba4cf9`. `python3 tools/convergence_gate.py` returned
`CONVERGENCE STRUCTURE GREEN`. The candidate branch and canonical ledger were not
mutated by that simulation.
