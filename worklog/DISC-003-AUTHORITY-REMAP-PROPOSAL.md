# DISC-003 authority remap proposal

## Scope and authority boundary

- Audited revision: `9c4fb6b39c04feab239e4a730fc72d2b1d3be27b`.
- Audited `tasks/completion/claims.json` SHA-256:
  `2551dd5675dc0a3f839e439e493755b81e24db91b5c813a904b9ca50123d5d06`.
- Source audit: `worklog/DISC-003-CONVERGENCE-REVIEW.md` (53 unique rows).
- This worker did **not** modify canonical statuses, accepted flags, plan files,
  verifier logic, or policy. The patch below is a proposal for the designated
  controller/integration authority.

## Required disposition

The 51 off-plan `completed` rows are candidate-lane bookkeeping, not canonical
completion-plan tasks. Remove those rows from the active claim ledger after
retaining their immutable Git history and worklogs. This is retirement/remapping,
not deletion of evidence and not acceptance of the proposed parent. Change the two
in-plan audit rows whose own notes disclaim acceptance from `completed` to
`blocked`; move their existing notes to `blockedNote` unchanged.

Primary parent assignment is exact and single-valued:

```text
ACP-001=AUD-009
BASE-004=AUD-001
FIX-LOGROTATE=AUD-018
FIX-LOOP-RULES=AUD-017
FIX-NATIVE-DAEMON=AUD-001
FIX-PACKAGING=AUD-018
FIX-SANDBOX=DISC-106
FIX-SESSIONS-STUBS=AUD-003
FIX-SQLITE-GATE=AUD-007
FIX-TIMELINE=AUD-013
G6-CHAT-DATADIR=AUD-001
HEAD-001=AUD-001
HEAD-002=AUD-001
LANE-APPSTART-VIEW=APP-001
LANE-AUTH-401=AUD-001
LANE-AUTODRIVE-CLAMP=AUD-017
LANE-CHAT-ORIGIN=APP-011
LANE-CI-CAPS=AUD-018
LANE-DESC-STALE=AUD-001
LANE-FILE-AUTHZ=AUD-005
LANE-LOOP-CAP=AUD-004
LANE-MAIN-ONCE2=APP-001
LANE-ONBOARD-SETUP=APP-005
LANE-PROV-FALLBACK=AUD-002
LANE-RALPH-MAX2=AUD-017
LANE-SHELL-AUTHZ=AUD-005
LANE-SRV-ROUTER=AUD-001
LANE-TIMELINE-LAND=AUD-013
LANE-TOOL-PERM=DISC-105
LANE-TRANSCRIPT-LAND=AUD-013
LANE-TUI-HOST=AUD-011
LANE-TURN-SETTLE=APP-004
LANE-WEB-HONEST=AUD-014
OPS-009=AUD-018
PROV-018=AUD-002
PROV-019=AUD-002
PROV-020=AUD-002
PROV-021=AUD-002
PROV-022=AUD-002
REL-003=AUD-018
RUN-001=AUD-010
SDK-001=AUD-010
SDK-002=AUD-010
SYNC-001=AUD-010
SYNC-002=AUD-010
TOOL-012=AUD-005
TOOL-018=AUD-005
TOOL-019=AUD-005
WEB-004=AUD-014
WEB-005=AUD-014
WEB-006=AUD-014
```

Legacy assignments follow `completion_plan.legacy_report()` and the canonical
`auditShards[].legacyPrefixes`; aliases use the real repaired caller/surface.
The assignment records where candidate evidence must be reconsidered. It does
not mutate the parent's status.

## Exact authority patch algorithm

Run only after checking out the audited revision and confirming the hash above.
The script intentionally aborts on any changed ID set or status. It writes a
candidate file; the authority must review the diff before replacing the ledger.

```python
import hashlib, json, pathlib

path = pathlib.Path("tasks/completion/claims.json")
raw = path.read_bytes()
assert hashlib.sha256(raw).hexdigest() == (
    "2551dd5675dc0a3f839e439e493755b81e24db91b5c813a904b9ca50123d5d06"
)
doc = json.loads(raw)
claims = doc["claims"]

retire = set("""ACP-001 BASE-004 FIX-LOGROTATE FIX-LOOP-RULES
FIX-NATIVE-DAEMON FIX-PACKAGING FIX-SANDBOX FIX-SESSIONS-STUBS
FIX-SQLITE-GATE FIX-TIMELINE G6-CHAT-DATADIR HEAD-001 HEAD-002
LANE-APPSTART-VIEW LANE-AUTH-401 LANE-AUTODRIVE-CLAMP LANE-CHAT-ORIGIN
LANE-CI-CAPS LANE-DESC-STALE LANE-FILE-AUTHZ LANE-LOOP-CAP
LANE-MAIN-ONCE2 LANE-ONBOARD-SETUP LANE-PROV-FALLBACK LANE-RALPH-MAX2
LANE-SHELL-AUTHZ LANE-SRV-ROUTER LANE-TIMELINE-LAND LANE-TOOL-PERM
LANE-TRANSCRIPT-LAND LANE-TUI-HOST LANE-TURN-SETTLE LANE-WEB-HONEST
OPS-009 PROV-018 PROV-019 PROV-020 PROV-021 PROV-022 REL-003 RUN-001
SDK-001 SDK-002 SYNC-001 SYNC-002 TOOL-012 TOOL-018 TOOL-019 WEB-004
WEB-005 WEB-006""".split())
assert len(retire) == 51
assert all(claims[tid]["status"] == "completed" for tid in retire)
for tid in sorted(retire):
    del claims[tid]

for tid in ("AUD-017", "AUD-020"):
    row = claims[tid]
    assert row["status"] == "completed"
    note = row.pop("completedNote")
    assert "no acceptance" in note.lower()
    row["status"] = "blocked"
    row["blockedNote"] = note

out = path.with_suffix(".authority-candidate.json")
out.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n")
print(out)
```

## Expected verification

On the exact audited tree after authority replaces the ledger with the reviewed
candidate:

1. `python3 tools/convergence_gate.py` reports structural GREEN and zero ledger
   findings. This is necessary, never release acceptance.
2. `python3 tools/validate_repository.py` must pass independently; any unrelated
   backlog/protection failure remains a blocker and must not be bypassed.
3. `python3 -m json.tool tasks/completion/claims.json` passes.
4. Recompute the 51 retired IDs and two blocked audit IDs; no other row changes.
5. Do not mark any mapped parent completed from this proposal. Each parent still
   requires integrated frozen journey evidence on the exact release revision.

If the ledger hash or finding set differs after rebase, stop and regenerate the
proposal from the new integrated revision rather than applying it partially.

## Disposable simulation receipt

The exact algorithm above was applied only in a detached disposable worktree at
`9c4fb6b`; canonical files were not changed.

- `python3 tools/convergence_gate.py` returned:
  `CONVERGENCE STRUCTURE GREEN`.
- `python3 tools/validate_repository.py` still failed at the independent backlog
  exhaustion check with 51 errors. This confirms the 53-row ledger remap is
  sufficient for the convergence gate but is not sufficient for the canonical
  repository guard or release acceptance.
- The backlog/exhaustion reconciliation remains a separate controller-authority
  action. This proposal must not be used to bypass or weaken it.
