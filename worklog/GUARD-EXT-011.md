# GUARD-EXT-011 - reconciliation audit

## Claim and boundary

- Task: `EXT-011`; session `ses_f320db215ffeV0T6c6QGbytnrH`; claimed through
  `tools/completion_claims.py` at `tasks/completion/claims.json`.
- Audit only. Owned writes: this scratchpad and the `EXT-011` ledger row. No
  canonical, product, test, controller, validator, or policy edits. No
  Cargo build/test, no heavy commands.
- Revision audited: `b3e0d01` (branch `lane/PHASE1-integration-20260923`),
  pushed branch `lane/GUARD-EXT-011-20260923` (checked out earlier; this audit
  runs from the integration branch).

## Exact attributable discrepancies

1. **Ralph acceptance versus source authority contradiction.**
   `ralph.json` `userStories[]` marks `EXT-011` `status: accepted` (FEATURES.md
   sync row 12 mirrors `accepted`). The source authority at
   `sources/extensibility-remaining-ownership-gap.json:38-53` keeps
   `ownershipDecision.EXT-011: null`, `taskBindingState.EXT-011.taskCard: null`,
   `taskBindingState.EXT-011.worklog: null`, `controllerStatus: in-progress`.
   `sources/backlog-exhaustion.json` independently classifies the same story
   `category: unresolved-decomposition`, `reasonKey:
   extensibility-family-not-decomposed`, `taskCard: null`, `taskStatus: null`,
   `implementationCommits: []`, `localEvidencePaths: []`. The controller
   acceptance in Ralph has no grounding in a task card, worklog, or
   implementation commit per the authoritative source files.

2. **Task card versus actual implementation location mismatch.**
   `tasks/EXT-011.md:90-94` suggests module boundary
   `crates/ext/src/transform_replay.rs` in crate `opencode-rk-ext` with test
   owner `crates/ext/tests/plugin_transform_replay.rs`. No `crates/ext/`
   directory exists in the workspace (`crates/` contains: agents, catalog, cli,
   contracts, foundation, opentui-bridge, providers, security, server,
   sessions, storage, tools). The actual implementation lives at
   `crates/tools/src/plugin_transform.rs:1-166` (166 lines), wired into
   `crates/tools/src/lib.rs:47` (`pub mod plugin_transform;`), with frozen tests
   at `crates/tools/tests/plugin_transform.rs:1-311` (311 lines, 5 test fns
   `ext011_t01` through `ext011_t05`). The `#[path]` test design means the
   implementation file is pulled into the test compilation directly, so the
   crate-level wiring in `lib.rs:47` does not change observable test behavior.
   However, the task card's module/test path does not match the on-disk
   reality, and no `opencode-rk-ext` crate is declared in any `Cargo.toml`.

3. **Equivalence-group ownership prohibition not resolved.**
   `sources/extensibility-remaining-ownership-gap.json:24-36` places EXT-011 in
   the `generic-disc-placeholder` equivalence group with EXT-004, EXT-006,
   EXT-010 (all `ownershipDecision: null`). The gap file at lines
   `159-164` (closure criteria) and `140-145` (candidate fragments) explicitly
   forbids ownership by "list order, task number, REQ-005 subtraction or
   surface membership." The local implementation cannot resolve this
   multi-task shared-signature ownership block; it requires a controller
   authority patch.

4. **Backlog exhaustion validator failure (shared-repo).**
   `sources/backlog-exhaustion.json` retains EXT-011 as
   `unresolved-decomposition` with `controllerStatus: in-progress` but
   `taskStatus: null`. Per the GUARD-OPS-001 precedent, the convergence gate
   and `validate_backlog_exhaustion.py` emit errors for accepted stories whose
   exhaustion ledger still marks them unresolved. EXT-011 is in that drift
   category: accepted in Ralph/feature index, unresolved in backlog. This is a
   shared repo-wide authority failure (the same pattern blocks OPS-001 through
   OPS-009, ROUTE-009, etc.), not a product-test failure.

## Evidence-based disposition

- **Implementation fragment:** candidate GREEN only. The existing worklog
  (`worklog/EXT-011.md`) records 5/5 frozen tests passing, a ghost-defect fix
  in `set_scope_disabled` at `crates/tools/src/plugin_transform.rs:128-139`
  (unknown scope no longer pushes a ghost entry into `disabled`), fuzz
  verification (10k random ops over 600 scopes), and zero test edits. The
  implementation is native Rust, bounded by `EXT11_MAX_TRANSFORMS = 512`,
  no I/O, no threads, no wall-clock. However, this candidate GREEN does not
  establish task-card authority: the task card points to a non-existent crate
  path (`crates/ext/`), and the source authority keeps `ownershipDecision:
  null` and `taskCard: null`.
- **Ownership:** blocked. The local implementation cannot convert the source
  audit's null ownership into accepted authority. Parent acceptance is
  prohibited by `docs/CONVERGENCE.md:26-28,99-105` and
  `ralph.completion.json:27-35` (`status: implementation-required`). The
  equivalence-group ownership prohibition and the task-card/path mismatch
  require a controller authority patch.
- **Status:** ledger `EXT-011` set `blocked`, not `completed`; reason records
  the authority-state failure: accepted in Ralph/FEATURES while unresolved in
  ownership-gap and backlog-exhaustion, plus module-path mismatch.

## Remaining unknowns

- Controller choice: retire/supersede the local fragment under EXT-004/006/010
  unified ownership, or award EXT-011 a distinct task-specific source/caller/
  test/spec path that resolves the shared-signature blocker.
- Whether the implementation should be relocated from `crates/tools/` to the
  nominally specified (but absent) `crates/ext/` crate after authority is
  resolved.
- Integrated caller wiring and release verification remain unproven; the
  `pub mod plugin_transform;` line at `lib.rs:47` is the only integration wire
  found, and no runtime caller consumes the transform log.
