# GUARD-OPS-003 reconciliation audit

## Scope

Guard reconciliation audit for OPS-003. Audit-only: no Cargo, canonical,
product, test, or validator edits. Owned surface: scratchpad + ledger row only.

Gate total budget: 80. No heavy commands run.

## Claim

- Task ID: OPS-003
- Session: ses_guard_ops_003_audit
- Scratchpad: worklog/GUARD-OPS-003.md
- Status set: in-progress -> blocked (audit outcome)

## Source evidence (exact path/line/symbol)

### Worker file / task card
- `tasks/OPS-003.md:1-164` — "Local-only repository cache-store lifecycle
  for one cache root: branch-isolated checkout slots are inspected, locked,
  marked fresh/stale, and swept when stale, entirely on the local filesystem
  with zero git transport, zero network, zero credential use."
- `tasks/OPS-003.md:76-79` — suggested module boundary
  `crates/ops/src/repo_cache_store.rs` (new crate `opencode-rk-ops`); fallback
  `crates/foundation/src/repo_cache_store.rs`. "Worker ships additive fragment
  only, never edits shared `lib.rs`, `Cargo.toml`, schemas, migrations."
- `tasks/OPS-003.md:80-84` — Explicit non-ownership (stays blocked): git
  transport (`git.ts`), clone/fetch/checkout/reset execution, observability
  sinks, installation beyond version metadata, `containers/**`, frozen
  INT-002/OPS-007/OPS-009, accepted BASE owners; OPS-002 parser owns
  normalization, this card only consumes its output shapes.
- `tasks/OPS-003.md:115-139` — Test obligations T01..T05 (frozen).

### ralph.json
- `ralph.json:1203-1214` — OPS-003 entry: status `accepted`,
  testObligations OPS-003-T01..T05, requirementIds [], dependencyIds [].

### ralph.completion.json
- `ralph.completion.json:83` — auditShards AUD-018 covers OPS-001..OPS-009.
- AUD-018 entry for OPS-003 (via sources/completion/audits/AUD-018.json:95-103):
  status `accepted`, tbd `false`, obligations T01/T02/T03.

### FEATURES.md
- `FEATURES.md:66` — row 24: `OPS-003` accepted, gate `b`.
- `FEATURES.md:894` — discovery note: "OPS-003 (accepted): Discovered
  during DISC-002 surface extraction; scope described by behavior-surface-rules.json"

### sources/behavior-surface-rules.json
- Line 599: `"OPS-003"` nominated by `opencode.repository-operations`
  surface rule (patterns: repository.ts, repository-cache.ts, git.ts,
  observability.ts, observability/**, installation/**, containers/**).

### sources/operations-ownership-gap.json
- Line 11: OPS-003 in `storyIds`.
- Line 24: `"OPS-003": ["opencode.repository-operations"]` in storySurfaceSignatures.
- Line 57: `"OPS-003"` in repository-only equivalence group.
- Line 79: `"OPS-003": null` in `ownershipDecision`.
- Line 103: `"OPS-003": { "ralphStory": "Discovered during DISC-002 ...",
  "requirementIds": [], "taskCard": null, "worklog": null }`.
- Line 416-458: `reviewedPartitions` entry `repository-cache-materialization`:
  source `repository-cache.ts:100-230` blob `cab2f631`, test blob `4433583`,
  caller `reference.ts:75-103` blob `1e1ab9d`.
  behavior: "RepositoryCache serializes checkout materialization behind a
  file lock and performs stale removal, clone/refresh/checkout/reset plus
  Git-derived result projection."
  sideEffects: filesystem-mutation, git-network-transport, cross-process-file-lock,
  background-scope-lifetime.
  ownershipBlocker: "The contract violates the authorized no-network boundary
  and remains shared across multiple residual OPS ids plus frozen
  INT-002/OPS-007/OPS-009 overlap."
- Line 467-472: `unresolvedRulePartitions` `opencode.repository-operations`:
  `git.ts`, `observability.ts` and observability/**, `installation/**` beyond
  reviewed version metadata, `containers/**` remain unowned remainders.
- Line 518: closureCriteria — "An OPS-001/002/003/004/005/006/008 ownership
  change requires new pinned task-specific source/path evidence, not surface
  subtraction, equivalence-group membership, requirement arithmetic or
  task-number inference."

### sources/disc-003-reconciliation.json
- Reviewed record `opencode.repository-operations` (the repository-operations
  finding). reviewState: `partial`.
  Key unresolved notes:
  - "The DISC-002 family also nominates container installation and
    observability paths not exhausted by this repository-cache slice."
  - "Do not treat parser-only behavior as full INT-002 or infer
    filesystem/network/Git/environment ownership for OPS tasks."
  - "OPS-007/OPS-009 and INT-002 still require task-level decomposition
    before implementation."

### sources/backlog-exhaustion.json
- Line 789: `"id": "OPS-003"` entry.
  `controllerStatus: "not-started"`, `reasonKey: "operations-family-not-decomposed"`,
  `taskCard: null`, `worklog: null`, `implementationCommits: []`,
  `localEvidencePaths: []`, `surfaceEvidenceGaps: []`.

### Existing implementation (pre-existing, not authored this session)
- `crates/foundation/src/repo_cache_store.rs:1-443` — full impl exists.
- `crates/foundation/src/lib.rs:26` — `pub mod repo_cache_store;` wired.
- `crates/foundation/tests/repo_cache_store.rs:1-219` — frozen T01..T05 tests.
- Tests GREEN (5 passed / 0 failed) — ran via cargo test prior session;
  this audit session did NOT re-run Cargo (no heavy commands per task).

### Prior worklog
- `worklog/OPS-003.md:1-34` — prior session notes: impl+tests GREEN,
  5/5 passing, no code/test edits, "Verifier acceptance separate."

## Reconciliation: guard errors across ralph/task/worklog

1. **ralph.json says `accepted`; task card says `NOT STARTED`.**
   - ralph.json:1205 status = `accepted`.
   - tasks/OPS-003.md:3 Status = `NOT STARTED`.
   - ralph.completion.json AUD-018:95 status = `accepted`, tbd = false.
   - Resolution: `accepted` in ralph.json/ralph.completion.json means the
     story is accepted into the backlog, NOT that it is implemented or
     owned. The task card Status field (`NOT STARTED`) and backlog-exhaustion.json
     `controllerStatus: not-started` are the controller truth. No conflict.

2. **ralph.json testObligations lists T01..T05; AUD-018 lists T01/T02/T03 only.**
   - ralph.json:1209-1213: OPS-003-T01, T02, T03, T04, T05.
   - AUD-018.json:99-103: only T01, T02, T03.
   - Resolution: AUD-018 audit shard excerpting T01-T03 is a partial listing
     in the audit evidence, not a truncation of the story contract. The full
     set per tasks/OPS-003.md:8 is T01..T05, all 5 implemented and passing.

3. **worklog/OPS-003.md claims GREEN; backlog-exhaustion.json says not-started.**
   - worklog/OPS-003.md:24: "GREEN this session (bounded cmd): 5 passed / 0 failed."
   - backlog-exhaustion.json:789: `controllerStatus: not-started`,
     `implementationCommits: []`, `worklog: null`.
   - Resolution: This is the central guard discrepancy. The prior worklog
     documents a local GREEN (impl+tests pass) but the exhaustion ledger —
     the canonical controller status record — remains `not-started` with no
     implementation commit and no linked worklog. The partial slice passed
     tests but was NOT landed/integrated as a completion. The ledger is the
     authoritative controller truth; the worklog note is verification-only
     evidence that the slice compiles and passes, which does not constitute
     acceptance (per ralph.completion.json:31 "testCasesAreSpecificationsNotPassingTests").

## Reconciliation: operations gap

- **operations-ownership-gap.json** `ownershipDecision.OPS-003 = null`.
  The `ownershipBlocker` for `repository-cache-materialization` (line 457):
  "The contract violates the authorized no-network boundary and remains
  shared across multiple residual OPS ids plus frozen INT-002/OPS-007/OPS-009
  overlap."
- tasks/OPS-003.md:80-84 explicitly lists what OPS-003 does NOT own: git
  transport, clone/fetch/checkout/reset, observability sinks, installation
  beyond version metadata, containers/**, frozen INT-002/OPS-007/OPS-009.
- The implemented `repo_cache_store.rs` is the transport-free local lifecycle
  only (inspect/lock/mark_fresh/sweep + NoNetworkTransport refusal). Real git
  materialization stays behind the `Transport` trait by design.
- **Gap status:** The full repository-cache-materialization contract (clone/
  fetch/checkout/reset + Git projection + observability) is NOT decomposed
  into a task-specific owner. OPS-003 consumes OPS-002 normalization outputs
  as input types only (tasks/OPS-003.md:40). The equivalence group
  repository-only = [OPS-002, OPS-003, OPS-006] (operations-ownership-gap.json:55-62)
  is mutually indistinguishable by surface signature alone.

## Reconciliation: exhaustion ledger

- `backlog-exhaustion.json:789`: OPS-003 `controllerStatus: not-started`,
  `reasonKey: operations-family-not-decomposed`, `taskCard: null`,
  `worklog: null`, `implementationCommits: []`.
- closureCriteria (operations-ownership-gap.json:517-521): an ownership
  change requires "new pinned task-specific source/path evidence" and
  "every unresolved partition and unresolved rule remainder must receive a
  source-grounded task disposition or remain explicitly blocked."
- The repository-only equivalence group (OPS-002/003/006) and config-and-
  repository group (OPS-004/008) still lack task-specific decomposition.
- `unresolvedRulePartitions` lists git.ts, observability.ts, observability/**,
  installation/**, containers/** as unowned remainders.

## Reconciliation: FEATURES.md

- FEATURES.md:66 row 24: OPS-003 accepted, gate `b`.
- FEATURES.md:894: OPS-003 accepted, discovery note.
- FEATURES-COMPLETION.md was not checked (out of claimed scope); no
  discrepancy found in the two FEATURES.md references. Both are consistent
  with "accepted into backlog, not yet implemented/owned."

## Authority changes and checks

Per tasks/OPS-003.md:78-79, the worker ships an additive fragment and
"never edits shared lib.rs, Cargo.toml, schemas, migrations." However,
`crates/foundation/src/lib.rs:26` already declares `pub mod repo_cache_store;`
and `crates/foundation/Cargo.toml` already exists. This audit does NOT edit
any of those — it confirms the slice is implemented and passing but remains
blocked at the controller/exhaustion level.

### Checks performed (no heavy commands)
- File existence + content reads verified for all evidence above.
- Prior-session test run evidence recorded in worklog/OPS-003.md:24
  (5/5 passing) and AUD-018.json. This audit session did not re-run Cargo.

### Checks NOT performed (out of audit-only scope)
- cargo test (no Cargo edits allowed; prior result trusted from worklog).
- validate_repository.py / convergence_gate.py (canonical/validator files,
  protected per .github/CODEOWNERS).
- No edits to ralph.json, ralph.completion.json, FEATURES.md, backlog-exhaustion.json,
  operations-ownership-gap.json, or any protected/canonical/product/test file.

## Verdict

OPS-003 owns ONLY the transport-free local cache-store lifecycle (inspect/
lock/mark_fresh/stale-sweep + NoNetworkTransport refusal). That partial slice
is implemented at `crates/foundation/src/repo_cache_store.rs:1-443` with
frozen tests at `crates/foundation/tests/repo_cache_store.rs:1-219` and passes
5/5. However, the controller/exhaustion ledger (`backlog-exhaustion.json:789`)
remains `not-started` with `reasonKey: operations-family-not-decomposed` and
the `ownershipDecision` in `operations-ownership-gap.json:79` is `null`,
because the full repository-cache-materialization contract (clone/
fetch/checkout/reset + Git transport + observability) violates the
authorized no-network boundary and is shared across OPS-002/003/004/006/008
plus frozen INT-002/OPS-007/OPS-009. No task-specific source/path evidence
selects OPS-003 over its surface-identical siblings (closureCriteria at
operations-ownership-gap.json:518).

Therefore OPS-003 must remain **blocked**, not completed. The GREEN tests
are a local verification artifact, not acceptance. Per ralph.completion.json:31
("testCasesAreSpecificationsNotPassingTests") and ralph.completion.json:34
("Unmapped or partially implemented behaviors create mandatory child tasks
... Parent acceptance waits for all children"), the operations family must
be decomposed into task-specific owners with source-grounded evidence before
OPS-003 (or any sibling) can be accepted.

## Remaining unknowns

- Whether the operations family (OPS-002/003/004/006/008/007/009/INT-002) will
  be decomposed into individual task-level ownership with pinned
  source/path evidence. Until then, all remain blocked.
