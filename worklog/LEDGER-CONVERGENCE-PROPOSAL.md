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
implementation/controller lane and an independent verifier. The controller must
use a new reviewed transaction implementation or an explicitly reviewed bespoke
implementation. It must not call `save_ledger()`: `tools/completion_claims.py:187-192`
uses direct `write_text()` and has no compare-and-swap, durability, or recovery
protocol.

### Accounting boundary and exact operation

Evaluate the operation against the exact source ledger bytes, not a stale count.
Remove exactly R78. Demote exactly `AUD-017`, `AUD-020`, and
`INSTALLED-DEFAULT-CONTRACT-INTEGRATION` from `completed` to `blocked`; copy
each original `completedNote` byte-for-byte to `blockedNote`, then remove the
old `completedNote`. Preserve every other row and accepted evidence. This is
bookkeeping and evidence ownership, not feature reversal, parent completion,
accepted-commit rollback, or frozen-test-hash change.

### Initial CAS and authorization preconditions

Before Phase A, the authorized controller records the exact integrated commit,
fresh raw ledger SHA-256, branch/ref, mapping-document commits and raw hashes.
It verifies schema version, row bound, every row shape, exact R78 membership,
`status == completed` for every R78 row, no R78 ID in plan stories, and the
three dead scratchpads' absence or already durable re-home. It verifies exact
notes for `AUD-017`, `AUD-020`, and
`INSTALLED-DEFAULT-CONTRACT-INTEGRATION`, records the dependency consequences
for `PAR-001`, `COORD-001`, `SHIP-004`, and `DISC-119`, and confirms no accepted
parent, frozen test hash, worklog, or product commit will be edited or deleted.
Any mismatch aborts before Phase A. The controller must not reuse stale
`6c8cf2d8...` from addendum commit `3ba4cf9`.

### Hash domains and canonical bytes

The controller must use these exact domains. `JCS` means RFC 8785 JSON
Canonicalization Scheme, UTF-8, with duplicate keys, NaN, and Infinity
rejected. Arrays retain stated order; operation ID arrays are sorted ascending.
Hashes are lowercase hexadecimal SHA-256 values.

| Name | Preimage and hash |
|---|---|
| `row-v1` | `sha256(UTF-8(JCS(row-object)))`; row object is the exact ledger row without its task ID. This preserves the cited three row hashes. |
| `file-v1` | `sha256(raw file bytes)`; no path, newline, or Git metadata is prepended. |
| `ledger-v1` | `sha256(raw ledger bytes)`; candidate bytes are UTF-8 `json.dumps(document, indent=2, sort_keys=True, ensure_ascii=True, allow_nan=False)` followed by exactly one LF. This is the existing serialization profile, emitted without `save_ledger()`. |
| `manifest-v1` | `sha256(UTF-8(JCS(manifest with the manifestHash member absent)))`. The member is absent, never blank or recursive. |
| `receipt-v1` | `sha256(UTF-8(JCS(receipt with the receiptHash member absent)))`. The member is absent, never blank or recursive. |

Commit IDs, refs, output ledger hashes, and later verifier receipts are forbidden
in a Phase A manifest preimage when they depend on the commit containing that
preimage. Phase B and C records may reference them only after they exist.

### Phase A: durable evidence publication

Generate one bounded `txid` (1-64 lowercase ASCII characters, normally 32
random hexadecimal characters). Acquire the exclusive controller lock before
reading preconditions. Freeze the source commit and raw pre-operation ledger
hash. Publish, without changing `tasks/completion/claims.json`, these two files
in one Phase A commit:

* `worklog/DISC-003-ALIAS-EVIDENCE-REHOME.md`: the three dead rows' exact
  canonical row JSON, `row-v1` hash, session, status, missing scratchpad path,
  completed note, parent, source commit, pre-operation ledger hash, mapping
  path/line references, txid, and bookkeeping-only statement. It contains no
  Phase A commit ID, Phase B output hash, Phase C receipt hash, or self-hash.
* `worklog/DISC-003-ALIAS-EVIDENCE-REHOME.MANIFEST.json`: the immutable
  `manifest-v1` object with this required shape:

```json
{
  "format": "ledger-convergence-phase-a/v1",
  "phase": "A",
  "transactionId": "<txid>",
  "source": {
    "branch": "refs/heads/plan/ledger-convergence",
    "baseCommit": "<40-hex-git-id>",
    "ledgerPath": "tasks/completion/claims.json",
    "ledgerSha256": "<sha256 raw bytes>",
    "planRefs": [{"path": "PLAN.md", "commit": "<40-hex>", "sha256": "<sha256>"}],
    "mappingRefs": [
      {"path": "worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md", "commit": "<40-hex>", "sha256": "<sha256>"},
      {"path": "worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md", "commit": "<40-hex>", "sha256": "<sha256>"}
    ]
  },
  "evidence": {
    "path": "worklog/DISC-003-ALIAS-EVIDENCE-REHOME.md",
    "sha256": "<sha256 raw bytes>",
    "rows": [{"id": "<id>", "rowSha256": "<row-v1>", "parent": "<id>"}]
  },
  "operation": {
    "removeIds": ["<all 78 R78 IDs, sorted>"],
    "demote": [
      {"id": "AUD-017", "from": "completed", "to": "blocked", "rowSha256": "<input row-v1>"},
      {"id": "AUD-020", "from": "completed", "to": "blocked", "rowSha256": "<input row-v1>"},
      {"id": "INSTALLED-DEFAULT-CONTRACT-INTEGRATION", "from": "completed", "to": "blocked", "rowSha256": "<input row-v1>"}
    ]
  },
  "canonicalization": {"json": "RFC8785-JCS-UTF8", "ledger": "ledger-v1", "hashes": ["row-v1", "file-v1", "ledger-v1", "manifest-v1", "receipt-v1"]}
}
```

`removeIds` is the exact sorted R78 set, never a count or inferred mapping.
For each content-addressed ref, verify both the named Git blob at the named
commit and the raw file hash. The manifest has no `manifestHash`; compute it
after the object is complete and record it only in the private journal and
later receipts.

Write same-filesystem temporary siblings with mode `0600`, write all bytes,
flush, `fsync(fd)`, close, atomically replace each final path, then `fsync()`
the parent directory. Any failed write, flush, fsync, rename, or directory
fsync aborts closed. Phase A is committed only after both final hashes match
the manifest. Commit only these files, verify the tree diff, push ordinary
non-force fast-forward, and verify the exact remote branch tip. Record
`phaseACommit` only in the private journal and later receipts. Phase A is
immutable once the remote ref equals that commit; changed evidence requires a
new txid and Phase A commit.

### Phase B: ledger candidate and publication

Begin only after the Phase A commit exists locally, its tree is verified, and
the remote ref equals that exact commit. Store `manifestHash`, `phaseACommit`,
and the remote ref in a private journal, then re-read every CAS input immediately
before publication:

1. The exclusive lock is held by this txid, the worktree is clean, and HEAD is
   not detached.
2. Local HEAD, `refs/remotes/origin/plan/ledger-convergence`, and fresh
   `git ls-remote` all equal `phaseACommit`.
3. Ledger bytes and `ledger-v1` equal the Phase A `ledgerSha256`.
4. PLAN, mapping documents, their named commits/blob hashes, evidence file, and
   manifest hashes equal the Phase A manifest.
5. Schema, row bounds, row shapes, R78 membership, three demotions,
   missing-scratchpad evidence, and no-plan-story conditions pass.

Build the candidate in memory from the exact Phase A input. Validate claim row
shapes and exact changed-ID diff. Serialize with `ledger-v1`, write to a
same-filesystem temporary sibling, flush and `fsync` it, atomically replace the
canonical ledger, and `fsync` its parent directory. Immediately before rename,
perform a final branch/ledger CAS read. Any changed value leaves the canonical
ledger untouched and reports the exact field. After replacement, recheck branch
and candidate hash before the Phase B commit. A remote advance is a hard
failure, even if the local worktree is unchanged. Push Phase B only as a
non-force fast-forward from Phase A; rejected push means pending recovery,
never force-push.

Record candidate and bounded content-addressed pre-operation backup in the
private journal before Phase B commit. Never use `save_ledger()`, raw `del`, or
an unreviewed status replacement. Phase B output fields are not Phase A
preimage fields.

### Phase C: independent verification and receipt

An independent verifier receives the exact pushed Phase B commit. It verifies
Phase A hashes, input/output ledger hashes, exact R78 removal plus three
demotions, schema and row preservation, accepted commits/worklogs, and residual
classification. It runs JSON/schema and changed-ID checks,
`python3 tools/convergence_gate.py`, `python3 tools/validate_repository.py`
(backlog separate), `git diff --check`, backup/rollback checks, and preservation
checks for frozen hashes and historical evidence.

The verifier creates a `receipt-v1` candidate naming txid, Phase A commit and
manifest hash, Phase B commit and branch, input/output ledger hashes, evidence
path/hash, verifier commit, exact commands/results, frozen hashes, residual
IDs/line count, and unresolved rows. While hashed it has no `receiptHash`
member. The controller computes the hash, writes it to a new append-only
receipt file, and publishes a Phase C commit non-force. The Phase C commit ID
is recorded in the private journal after commit, never in the receipt preimage.
No receipt is proof of parent completion.

### Lock, journal, backups, and recovery

Use an exclusive lock on the repository common Git directory discovered with
`git rev-parse --git-path`, not a worktree-relative guess. Acquire a mode
`0600`, non-blocking exclusive lock with bounded txid/pid/start record. Failure
to acquire, malformed owner record, unexpected lock type, or active owner fails
closed. Hold it from the initial Phase A CAS read through the Phase C push. A
stale lock is never silently unlinked: only an explicitly authorized controller
may prove the owner stopped, inspect the journal, and recover or quarantine it.

The private journal is a bounded sidecar at
`.git/ledger-convergence/transactions/<txid>/state.json`, outside product state.
Journal writes use file fsync, atomic replace, and parent-directory fsync. It
contains txid, phase/state, expected refs, input/output hashes, manifest hash,
known commits, temp/backup hashes, and exact error codes. Permit one active
txid, at most eight sidecar files, and 16 MiB total. Keep one immutable
pre-operation ledger backup and one candidate backup until verification or
rollback review finishes. Never overwrite a backup.

Recovery is state-driven and idempotent; never automatically replay an
ambiguous side effect:

| Crash point | Required recovery |
|---|---|
| Before temp fsync | Canonical file unchanged; discard only untrusted temp; restart phase after CAS. |
| After temp fsync, before rename | Canonical unchanged; verify temp hash, then resume or discard. |
| After rename, before directory fsync | Verify final hash; finish directory fsync if expected candidate present; otherwise stop on corruption. |
| After directory fsync, before Phase A commit | If exact evidence/manifest bytes remain and branch is base, resume commit; otherwise guarded pre-commit restore. |
| After Phase A commit, before push | Resume non-force push only if remote is recorded parent; otherwise stop, never rewrite. |
| After Phase A push, before Phase B temp fsync | Phase A immutable; re-read all CAS values; resume or abort without ledger change. |
| After Phase B temp fsync, before ledger rename | Canonical unchanged; verify candidate and resume or discard after CAS. |
| After ledger rename, before directory fsync | Verify candidate, fsync directory, recheck CAS; guarded restore if branch/ledger changed. |
| After ledger directory fsync, before Phase B commit | Resume only with expected candidate hash and Phase A branch; otherwise stop or guarded restore. |
| After Phase B commit, before push | Push exact commit non-force only while remote is Phase A; never reset or force-push. |
| After Phase B push, before receipt | Phase B canonical; verify exact commit; do not reapply Phase B. |
| After receipt temp fsync/rename or directory fsync, before Phase C commit | Verify receipt preimage/hash and Phase B ref; resume receipt commit or discard unreferenced temp. |
| After Phase C commit, before push | Push exact commit non-force only if remote is its recorded parent; otherwise stop. |
| After Phase C push | Verify remote tip and receipt hash, mark journal final, clean only txid-owned bounded temps. |

Direct rollback is allowed only before Phase B commit, under the same explicit
controller authority, expected current candidate ledger hash, and expected
Phase A branch tip. Restore atomically, fsync file and parent, and record a
rollback receipt. After Phase B commit, rollback is a new explicit revert
commit, never overwrite/reset: require expected current branch tip, candidate
ledger hash, unchanged remote tip, and non-force fast-forward publication.
Any mismatch means no rollback and a blocked transaction. Concurrent changes
are never overwritten.

### Disposable simulation

Copy repository and ledger to a disposable restricted fixture. In that copy only,
write the three evidence records and Phase A manifest, verify hashes, remove R78,
demote three rows, validate the candidate, and exercise branch/ledger CAS and
guarded-rollback mismatch cases. Run the gate against that copy. Never write
the canonical ledger during simulation.

### Post-operation boundary

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
| `b15e0b6`, corrected proposal row completed | `f0cf39f78d5dbc4ddeeb6d9725d45a53f22326a97b429f481f1a70b95ee280af` | 90 | 4 | 94 | 92 |
| `642d8b8`, final verifier row completed | `fe922997896274140dfcb2e78e03aad6b996dbe9bdbed130b113e8683a53cfab` | 91 | 4 | 95 | 93 |

At `b15e0b6`, the correction row is completed and contributes one off-plan
finding. The final verifier commit `642d8b8` contributes another completed
off-plan row, so the current observed gate is `total=95`. Neither row belongs
to authoritative R78. A controller must explicitly classify each completed
proposal/verifier row; it must not treat either as in-progress or silently fold
either into DISC-003.

The drift from 86 to 95 is not a change to DISC-003's 78 aliases. It is eight
additional planning/verifier rows, producing nine finding lines because one
row has a bad-note finding as well: `LEDGER-CONVERGENCE-PROPOSAL`,
`LEDGER-CONVERGENCE-PROPOSAL-CORRECTION`, `LEDGER-CONVERGENCE-VERIFY`,
`LEDGER-CONVERGENCE-VERIFY-RETRY`, `LEDGER-CONVERGENCE-PROPOSAL-FINAL-VERIFY`,
`APP-010-FROZEN-INTEGRITY`, `APP-010-REVISION-RECEIPT`,
`APP-010-REVISION-RECEIPT-INTEGRATION`. The four `APP-012-*` rows are already
included in the base findings rather than this drift. The current `total=95`
includes the eight added rows as completed off-plan rows plus the one duplicate
bad-note line; all require explicit scope and none is part of R78.

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
lines. At corrected-proposal base `b15e0b6`, the completed correction row adds
one line, so the operation leaves exactly 12 finding lines:

```text
APP-010-FROZEN-INTEGRITY       off-plan + bad-note ('out of scope') = 2 lines
APP-010-REVISION-RECEIPT       off-plan = 1
APP-010-REVISION-RECEIPT-INTEGRATION off-plan = 1
APP-012-FILE-OPS               off-plan = 1
APP-012-READ-EXECUTOR          off-plan = 1
APP-012-READ-INTEGRATION       off-plan = 1
APP-012-SERVER-READ-WIRING          off-plan = 1
LEDGER-CONVERGENCE-PROPOSAL        off-plan = 1
LEDGER-CONVERGENCE-PROPOSAL-CORRECTION off-plan = 1
LEDGER-CONVERGENCE-VERIFY          off-plan = 1
LEDGER-CONVERGENCE-VERIFY-RETRY    off-plan = 1
```

At current base `642d8b8`, `LEDGER-CONVERGENCE-PROPOSAL-FINAL-VERIFY` adds a
13th residual line. The `b15e0b6` 12-line result is the required corrected-base
accounting. Any controller disposition for proposal, correction, verifier,
retry, or final-verifier rows requires explicit scope and must not be silently
counted as DISC-003 reconciliation.

## Classification of every residual after R78 reconciliation

This classification is for the corrected-proposal-base 12-line projection. It
is not an instruction to mutate state. At current base `642d8b8`, add
`LEDGER-CONVERGENCE-PROPOSAL-FINAL-VERIFY` as another controller-disposition
row with its verifier worklog and rejection receipt preserved.

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
| `LEDGER-CONVERGENCE-PROPOSAL-CORRECTION` | Retire or canonicalize only under controller authority | Preserve correction worklog and rejection/correction receipt |
| `LEDGER-CONVERGENCE-VERIFY` | Retire or canonicalize only under controller authority | Preserve verifier worklog and rejection receipt |
| `LEDGER-CONVERGENCE-VERIFY-RETRY` | Retire or canonicalize only under controller authority | Preserve retry verifier worklog and rejection receipt |
| `LEDGER-CONVERGENCE-PROPOSAL-FINAL-VERIFY` | Retire or canonicalize only under controller authority | Preserve final verifier worklog and rejection receipt; current-base-only, not part of b15 residual |

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
