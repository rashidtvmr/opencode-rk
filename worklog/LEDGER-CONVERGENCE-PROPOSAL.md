# LEDGER-CONVERGENCE-PROPOSAL

## Status and authority boundary

This document supersedes the proposal committed as `d93927e`
(`worklog/LEDGER-CONVERGENCE-PROPOSAL.md` at that revision). The old 54-row
retirement and 28-residual proposal is rejected and unsafe. It omitted 24
authoritative aliases, duplicated 27 rows, listed 15 wrong fold targets, risked
destroying the only evidence for three rows, and used operations unavailable to
workers.

This is a research and controller-reconciliation proposal only. It does not
modify `tasks/completion/claims.json`, parent status, plan files, verifier
configuration, tests, product files, or acceptance state. A controller must
recompute all preconditions on the exact integrated revision before any
operation. A worker must not apply the operation described here.

## Authoritative evidence

| Evidence | Exact location | Meaning |
|---|---|---|
| Original alias set and mappings | `worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md:22-81,100-127` | 51 aliases, exact canonical parents |
| Integrated alias addendum | `worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md:11-69` | 27 new aliases, exact canonical parents, prior hash warning |
| Authoritative verifier | `worklog/LEDGER-CONVERGENCE-VERIFY-RETRY.md:12-28,60-233` | Rejection, API limits, evidence hazard, transaction requirements |
| Gate implementation | `tools/convergence_gate.py:126-148` | Finding definition and line-count behavior |
| Claim API | `tools/completion_claims.py:32-38,89-92,144-192,243-247` | Legal transitions and absence of retire/demote API |
| Worker boundary | `AGENTS.md:10-15,174-194` and `.agents/WORKER.md:144-155` | No controller-state mutation by this lane |

The authoritative set is reproducibly derived, not inferred from naming:

```text
R51 = the exact `retire` set in DISC-003-AUTHORITY-REMAP-PROPOSAL.md:100-110
R27 = the exact alias IDs in DISC-003-INTEGRATED-REMAP-ADDENDUM.md:18-45
R78 = R51 union R27
|R51| = 51
|R27| = 27
R51 intersect R27 = empty
|R78| = 78
```

The two source documents are the canonical mapping evidence. The following
complete transcription preserves every alias-to-parent assignment. No target
outside those documents is proposed.

### R51: original authority mapping

```text
ACP-001 -> AUD-009
BASE-004 -> AUD-001
FIX-LOGROTATE -> AUD-018
FIX-LOOP-RULES -> AUD-017
FIX-NATIVE-DAEMON -> AUD-001
FIX-PACKAGING -> AUD-018
FIX-SANDBOX -> DISC-106
FIX-SESSIONS-STUBS -> AUD-003
FIX-SQLITE-GATE -> AUD-007
FIX-TIMELINE -> AUD-013
G6-CHAT-DATADIR -> AUD-001
HEAD-001 -> AUD-001
HEAD-002 -> AUD-001
LANE-APPSTART-VIEW -> APP-001
LANE-AUTH-401 -> AUD-001
LANE-AUTODRIVE-CLAMP -> AUD-017
LANE-CHAT-ORIGIN -> APP-011
LANE-CI-CAPS -> AUD-018
LANE-DESC-STALE -> AUD-001
LANE-FILE-AUTHZ -> AUD-005
LANE-LOOP-CAP -> AUD-004
LANE-MAIN-ONCE2 -> APP-001
LANE-ONBOARD-SETUP -> APP-005
LANE-PROV-FALLBACK -> AUD-002
LANE-RALPH-MAX2 -> AUD-017
LANE-SHELL-AUTHZ -> AUD-005
LANE-SRV-ROUTER -> AUD-001
LANE-TIMELINE-LAND -> AUD-013
LANE-TOOL-PERM -> DISC-105
LANE-TRANSCRIPT-LAND -> AUD-013
LANE-TUI-HOST -> AUD-011
LANE-TURN-SETTLE -> APP-004
LANE-WEB-HONEST -> AUD-014
OPS-009 -> AUD-018
PROV-018 -> AUD-002
PROV-019 -> AUD-002
PROV-020 -> AUD-002
PROV-021 -> AUD-002
PROV-022 -> AUD-002
REL-003 -> AUD-018
RUN-001 -> AUD-010
SDK-001 -> AUD-010
SDK-002 -> AUD-010
SYNC-001 -> AUD-010
SYNC-002 -> AUD-010
TOOL-012 -> AUD-005
TOOL-018 -> AUD-005
TOOL-019 -> AUD-005
WEB-004 -> AUD-014
WEB-005 -> AUD-014
WEB-006 -> AUD-014
```

### R27: integrated addendum mapping

```text
LANE-AGENT-FILES -> AUD-004
LANE-CI -> AUD-018
LANE-CI-EXT -> AUD-018
LANE-CI-FLAG -> AUD-018
LANE-COMMANDS-LIVE -> AUD-012
LANE-CONTEXT-ACCOUNT -> AUD-003
LANE-CONTEXT-CMD -> AUD-003
LANE-DISPATCH-DENY -> AUD-005
LANE-GLOBS -> AUD-012
LANE-GLOBS-LIVE -> AUD-012
LANE-LOOP -> AUD-004
LANE-LOOP-LIVE -> AUD-004
LANE-MCP-LIVE -> AUD-009
LANE-RULES -> AUD-012
LANE-SANDBOX -> AUD-006
LANE-SUBAGENT-LIVE -> AUD-004
LANE-THEMES -> AUD-013
LANE-TUI-GRAPH -> AUD-013
LANE-ULTRA-CODEGEN -> AUD-004
LANE-WEB-CANVAS -> AUD-014
LANE-WF-CREATE -> AUD-004
LANE-WF-TIMELINE -> AUD-013
RC-01 -> AUD-001
RC-02 -> AUD-001
RC-03 -> AUD-001
WEB-EVENT-STREAM -> AUD-014
WEB-HINT -> AUD-014
```

All 78 rows are evidence aliases, not plan stories. Retiring an alias must
not mark its parent completed, alter a parent worklog, reopen a feature, or
reverse an accepted commit. The fold target records evidence ownership only.

## Resolution of the 15 contradictory targets

The old proposal's A2 table is not source authority and must be discarded.
The authoritative table in `DISC-003-AUTHORITY-REMAP-PROPOSAL.md:24-76`
resolves all 15 conflicts:

| Alias | Rejected target | Authoritative target |
|---|---|---|
| BASE-004 | AUD-010 | AUD-001 |
| FIX-LOGROTATE | AUD-001 | AUD-018 |
| FIX-LOOP-RULES | AUD-005 | AUD-017 |
| FIX-PACKAGING | DISC-102 | AUD-018 |
| G6-CHAT-DATADIR | AUD-011 | AUD-001 |
| HEAD-001 | AUD-011 | AUD-001 |
| HEAD-002 | AUD-003 | AUD-001 |
| LANE-AUTODRIVE-CLAMP | COORD-001 | AUD-017 |
| LANE-CHAT-ORIGIN | AUD-015 | APP-011 |
| LANE-CI-CAPS | COORD-005 | AUD-018 |
| LANE-FILE-AUTHZ | DISC-105 | AUD-005 |
| LANE-PROV-FALLBACK | PROV-014 | AUD-002 |
| LANE-RALPH-MAX2 | COORD-002 | AUD-017 |
| LANE-SHELL-AUTHZ | DISC-105 | AUD-005 |
| LANE-SRV-ROUTER | AUD-018 | AUD-001 |

No target is invented for these rows. The other 63 mappings are the exact
source mappings transcribed above.

## Evidence re-homing before removal

Three rows point to scratchpads absent from the working tree and all refs:

| Alias | Missing scratchpad | Current note evidence | Current canonical row hash |
|---|---|---|---|
| LANE-CI-EXT | `worklog/LANE-CI-EXT.md` | `VERIFY 11/12 (t03 gated pre-existing STREAM regression, owner LANE-STREAM-FIX/ci_ext.rs fixture) @767a86a, zero test edits` | `70913b2873eb0595a3875c28c0ec9a69682073b524823d8e4b4cf9425c03c771` |
| WEB-EVENT-STREAM | `worklog/WEB-EVENT-STREAM.md` | `VERIFY 5/5 event_stream @767a86a, zero test edits` | `4ebddbb46e15ed8bb479fbcdfb7582221af5a1ba1729a103c1d56bec1250e197` |
| WEB-HINT | `worklog/WEB-HINT.md` | `VERIFY 8/8 composer-effort vitest + tsc clean @767a86a, zero test edits (blobs restored, untracked)` | `b75907e6767a87381bf9885f247b622f8b7ef08f88e6130bb623fa0dccb3adeb` |

Before removal, the authorized controller must create and commit a durable,
append-only evidence file, proposed destination:
`worklog/DISC-003-ALIAS-EVIDENCE-REHOME.md`. This proposal does not create it.
The file must preserve, verbatim and without secret material:

1. alias ID, exact canonical row JSON, row hash, session, status, missing
   scratchpad path, and completed note;
2. source commit, pre-operation full-ledger SHA-256, mapping-document commit
   and path/line references;
3. authoritative parent mapping and explicit statement that the row is
   retired bookkeeping, not feature reversal or acceptance;
4. controller transaction ID, destination-file hash, commit hash, and
   independent-verifier receipt hash after re-homing.

The controller must refuse the 78-row removal if any of these three notes is
not durably re-homed and hash-linked first. A row deletion that leaves only an
uncommitted or chat copy is evidence loss.

## Legal operations and authority

| Operation | Worker API/status | Proposal boundary |
|---|---|---|
| Claim a task | `not-started -> in-progress` via `claim()` | Legal only for the worker's own leased task |
| Finish a task | `in-progress -> completed` or `blocked` via `update()` | Legal only with required evidence note |
| Release/reclaim live work | `release()` for own in-progress; controller `reclaim()` for live foreign claim | Does not touch completed rows |
| Retire/delete completed alias row | No `delete`, `remove`, or `retire` API | Explicit controller authority required |
| Demote completed row | `TRANSITIONS["completed"] == set()`; `completed -> blocked` is rejected | Explicit controller authority required |

`save_ledger()` validates row shape but does not authorize a transition.
Raw `del claims[id]`, raw status replacement, or a direct `save_ledger()` call
is therefore not a routine worker command. The old proposal's Stage 1 and
Stage 2 snippets are withdrawn, not authorized instructions. No worker may
extend `completion_claims.py` or bypass it in this lane.

## Proposed controller transaction

This is a design, not an execution command. It requires a separate
implementation/controller lane and an independent verifier.

### Preconditions

1. Freeze the exact integrated commit and calculate the full SHA-256 of
   `tasks/completion/claims.json`; do not reuse stale `6c8cf2d8...`, which
   belongs to the addendum's earlier commit `3ba4cf9`.
2. Verify schema version, bounded row count, every row shape, all 78 IDs
   present with `status == completed`, no 78 ID in plan stories, and the exact
   `R51 union R27` set. Verify the three dead scratchpads are still absent or
   already re-homed.
3. Verify `AUD-017`, `AUD-020`, and
   `INSTALLED-DEFAULT-CONTRACT-INTEGRATION` are completed and preserve their
   exact notes. Record dependency consequences: demoting AUD-017/AUD-020
   affects `PAR-001`, `COORD-001`, `SHIP-004`, and `DISC-119`.
4. Confirm no accepted parent, frozen test hash, worklog, or product commit is
   being edited or deleted. Confirm the transaction is not a feature rollback.

### Disposable simulation

Copy the repository and ledger to a disposable restricted fixture. In that
copy only, re-home the three notes, remove R78, move each of the three
completed notes verbatim to `blockedNote`, and validate the candidate. Run the
gate against that copy. Do not write the canonical ledger during simulation.

### Backup, candidate, atomic write

Create a content-addressed backup of the pre-operation ledger, re-homing file,
and transaction manifest. Write a candidate ledger plus manifest to temporary
files in the same filesystem, fsync as required by the controller, validate
JSON/schema/hash preconditions, then atomically replace the canonical ledger.
Retain the backup and rollback metadata. On any mismatch, leave the canonical
ledger untouched and report the exact failed precondition. Rollback restores
the backup only under the same explicit controller authority and receipt
process.

### Post-operation verification

The independent verifier, on the exact resulting commit, must run:

1. JSON/schema validation and exact changed-ID diff: only R78 removals plus the
   three status/note demotions, and the authored evidence re-home;
2. `python3 tools/convergence_gate.py`, with expected ledger result described
   below;
3. `python3 tools/validate_repository.py`, independently, without treating
   backlog findings as ledger convergence;
4. repository validation, `git diff --check`, backup/hash/rollback receipt
   checks, and preservation checks for accepted commits, worklogs, frozen test
   hashes, and historical evidence;
5. a signed or hash-linked post-operation receipt naming input SHA, output SHA,
   transaction ID, verifier commit, commands, results, and unresolved rows.

No convergence gate result is an acceptance claim. The parent remains open
until its integrated journey and independent release verification satisfy the
repository contract.

## Count model and expected residuals

Counts are finding lines emitted by `tools/convergence_gate.py`, not unique
task IDs. A task can emit two lines when it is both off-plan and has a bad
completed note.

### Immutable bases

| Base | Ledger SHA-256 | Off-plan lines | Bad-note lines | Total lines | Distinct finding IDs |
|---|---|---:|---:|---:|---:|
| `1f4a9e6`, before rejected proposal | `25bb55306e0f3f4650a2b4430d68a277627c15a3fc1d3c9a833109ab6985fca3` | 83 | 3 | 86 | 85 |
| `d93927e`, rejected proposal row added | `950ac488642cd667700ee4582afddef192ecd986ea683ab88fe38b0fd935a2f4` | 84 | 3 | 87 | 86 |
| `b16e71b`, prior verifier row added | `62d5af88aaa9dbef360afbd4b9dd760fbe5c85532f90c09419ddab706e4b3959` | 88 | 4 | 92 | 90 |
| `4e5175e`, retry verifier row added | `10137d6df687357159f46209eef1e780c8c7092e01ce81496012b40422970d52` | 89 | 4 | 93 | 91 |

Current observed gate is `total=93` at `4e5175e`/`056a210` base before this
proposal row is counted. This correction's in-progress row is itself a new
off-plan row, so a later gate count can become `94` unless the controller
includes or retires it. The correction row must not be silently folded into
the authoritative R78 set.

The drift from 86 to 93 is not a change to DISC-003's 78 aliases. It is seven
additional planning/verifier rows: `LEDGER-CONVERGENCE-PROPOSAL`,
`LEDGER-CONVERGENCE-VERIFY`, `LEDGER-CONVERGENCE-VERIFY-RETRY`,
`APP-010-FROZEN-INTEGRITY`, `APP-010-REVISION-RECEIPT`,
`APP-010-REVISION-RECEIPT-INTEGRATION`, and this correction row. The current
observed 93 includes the first six and excludes this in-progress row from
`ledger_errors()` because only completed rows are findings.

### Residual after the authoritative operation

At immutable base `1f4a9e6`, disposable simulation of R78 removal plus the
three demotions leaves exactly 4 finding lines:

```text
APP-012-FILE-OPS
APP-012-READ-EXECUTOR
APP-012-READ-INTEGRATION
APP-012-SERVER-READ-WIRING
```

At prior-verifier base `b16e71b`, the same operation leaves exactly 10 finding
lines. The retry verifier commit `4e5175e` and current `056a210` add one
completed off-plan retry row, so the current base operation leaves exactly 11
finding lines:

```text
APP-010-FROZEN-INTEGRITY       off-plan + bad-note ('out of scope') = 2 lines
APP-010-REVISION-RECEIPT       off-plan = 1
APP-010-REVISION-RECEIPT-INTEGRATION off-plan = 1
APP-012-FILE-OPS               off-plan = 1
APP-012-READ-EXECUTOR          off-plan = 1
APP-012-READ-INTEGRATION       off-plan = 1
APP-012-SERVER-READ-WIRING     off-plan = 1
LEDGER-CONVERGENCE-PROPOSAL   off-plan = 1
LEDGER-CONVERGENCE-VERIFY     off-plan = 1
LEDGER-CONVERGENCE-VERIFY-RETRY off-plan = 1
```

The retry row is already present at current base. This correction row is
in-progress and therefore emits no finding. Any controller disposition for
proposal, verifier, retry, or correction rows requires explicit scope and must
not be silently counted as DISC-003 reconciliation.

## Classification of every residual after R78 reconciliation

This classification is for the current-base 11-line projection. It is not an
instruction to mutate state.

| Residual finding | Classification | Required authority/evidence |
|---|---|---|
| `APP-010-FROZEN-INTEGRITY` off-plan line | Investigate, then canonicalize or keep open | Review its `out of scope` note and parent receipt; no automatic fold |
| `APP-010-FROZEN-INTEGRITY` bad-note line | Keep open pending investigation | Its note explicitly limits scope; no completion claim |
| `APP-010-REVISION-RECEIPT` | Canonicalize only if controller maps it to `APP-010`; otherwise keep open | Parent ownership and receipt scope must be verified |
| `APP-010-REVISION-RECEIPT-INTEGRATION` | Canonicalize only if controller maps it to `APP-010`; otherwise keep open | Integrated revision receipt must remain durable |
| `APP-012-FILE-OPS` | Keep open under in-progress `APP-012` | Parent remains open; do not retire child as accepted |
| `APP-012-READ-EXECUTOR` | Keep open under in-progress `APP-012` | Parent remains open; preserve frozen hash and note |
| `APP-012-READ-INTEGRATION` | Keep open under in-progress `APP-012` | Parent remains open; preserve integrated receipt |
| `APP-012-SERVER-READ-WIRING` | Keep open under in-progress `APP-012` | Parent remains open; preserve caller/wiring evidence |
| `LEDGER-CONVERGENCE-PROPOSAL` | Retire or canonicalize only under controller authority | Historical proposal must remain in Git/worklog evidence |
| `LEDGER-CONVERGENCE-VERIFY` | Retire or canonicalize only under controller authority | Preserve verifier worklog and rejection receipt |
| `LEDGER-CONVERGENCE-VERIFY-RETRY` | Retire or canonicalize only under controller authority | Preserve retry verifier worklog and rejection receipt |

`canonicalize` means an explicit controller mapping to a real plan parent with
evidence retained; it does not mean marking that parent completed. `demote`
means an explicit controller status decision preserving the note and recording
dependency ripple. No current residual is authorized for automatic demotion.
The retry row and this correction row require the same explicit disposition if
they become completed/off-plan findings later.

## Separate repository backlog gate

`validate_repository.py` backlog-exhaustion findings are separate from ledger
convergence. The verifier reports a pre-existing 51-error backlog-exhaustion
failure at current base. A green convergence simulation cannot suppress,
reinterpret, or satisfy that guard. The controller must run repository
validation independently and retain its failure or success receipt. No
backlog, plan, `ralph`, DISC-003 manifest, protection, or acceptance state is
changed by this proposal.

## Explicit authorization and handoff

Before application, the designated controller/integrator must explicitly
authorize all of the following in a recorded transaction:

1. the exact integrated commit and fresh pre-operation ledger SHA;
2. R78, exactly as `R51 union R27`, and the authoritative mapping table;
3. evidence re-homing for all three dead-scratchpad rows;
4. removal of exactly the authorized alias rows, if approved;
5. demotion of `AUD-017`, `AUD-020`, and
   `INSTALLED-DEFAULT-CONTRACT-INTEGRATION`, with notes preserved and
   dependency impact recorded;
6. any separate disposition for off-plan proposal/verifier/retry/correction
   rows and current APP-010/APP-012 residuals;
7. backup, atomic-write, rollback, schema, repository-validation, gate, and
   independent-verifier procedures;
8. post-operation receipt location and hashes.

Application belongs to a separate implementation/controller lane. Verification
belongs to an independent verifier lane on the exact resulting commit. This
proposal author cannot authorize, apply, or accept reconciliation. Until those
lanes complete, DISC-003 and the parent convergence task remain open.
