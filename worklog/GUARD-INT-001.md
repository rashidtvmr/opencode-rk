# GUARD-INT-001 - reconciliation audit

## Claim and boundary

- Task: `INT-001`; session: `ses_f320d3f47ffetlu4xdWszcQ71X`.
- Candidate: `b3e0d012822da9fe48e43126c9085e9782c2b082`.
- Branch: `lane/GUARD-INT-001-20260923`.
- Scope: this audit file plus the `INT-001` ledger row only.
- No canonical plan, product source, test, validator, feature index, or source-gap edit.
- No acceptance claimed.

## Exact disposition

**BLOCKED. `INT-001` remains not accepted and cannot authorize release or parent
completion.** Applied authority patch: ledger row only, `in-progress` to `blocked`,
with this bounded blocker:

> Controller reconciliation required. Ralph/FEATURES say `accepted`, while the
> card says `NOT STARTED`, the integrations gap keeps ownership `null`, and the
> backlog ledger classifies `INT-001` as unresolved decomposition with no task
> receipt, worklog binding, or implementation commit. Lane tests are isolated
> module suites; the lane-variant file is not wired and no real application
> caller or protocol acceptance is proven. Preserve secret/credential,
> env, auth/refresh-callback, network, persistence, and event blockers.

Controller-only follow-up (explicit reconciliation patch, not implementation):

1. Reconcile `ralph.json`/`FEATURES.md` accepted state versus
   `sources/integrations-ownership-gap.json` null ownership and
   `sources/backlog-exhaustion.json` unresolved-decomposition under the
   canonical repository guard. This lane cannot choose which authority wins.
2. Keep `integration-family-not-decomposed` and the generic
   `opencode.integrations` equivalence class unless a separately authorized
   source audit assigns exact per-story ownership. Do not convert lane test
   receipts into product acceptance.
3. Before any future promotion, require a real caller/wiring path, direct
   caller-visible failure tests against the wired crate surface, and preserved
   no-secret/no-network/no-env guarantees. Otherwise retain blocked status.

## Trace

### Plan, Ralph, card, index

- `ralph.json`: `INT-001` reports `accepted`, generic DISC-002 story
  ("Discovered during DISC-002 surface extraction; scope described by
  behavior-surface-rules.json"), `requirementIds: []`, obligations
  `INT-001-T01..T05`.
- `tasks/INT-001.md:1-7`: card reports `NOT STARTED`, mandatory product slice,
  requirements none, same five test obligations.
- `tasks/INT-001.md:11-19`: secret-free scoped registry projection; stores no
  credential material, reads no environment, performs no network or OAuth
  callback/refresh work, publishes no events.
- `tasks/INT-001.md:92-95`: bounds `MAX_INTEGRATIONS=64`,
  `MAX_METHODS_PER_INTEGRATION=8`, `MAX_SCOPES=16`, name/method/label caps.
- `tasks/INT-001.md:98-107`: suggested module `crates/providers/src/registry.rs`,
  test owner `crates/providers/tests/integration_registry.rs`; excludes INT-004
  attempt machine, INT-002 connection APIs, credential/env/callback/network/
  persistence/events.
- `FEATURES.md:56`: `INT-001` listed `accepted` with caveat
  `quiesced-tree re-run formality`; `:631` and `:883` repeat accepted generic story.

### Existing lane and worklog

- `worklog/INT-001.md:3-9`: lane owns `crates/providers/src/int_registry_lane.rs`,
  frozen tests in `crates/providers/tests/int_registry_lane.rs` (path-include,
  integrator-owned `lib.rs` untouched).
- `worklog/INT-001.md:51-61`: frozen test sha `cbd315d4...`, product sha
  `49644643...`; RED not independently witnessed at first pass (pre-existing
  tree); GREEN 5/5 INT-001; full providers regression lib 51 + targets ok.
- `worklog/INT-001.md:81-96`: addendum RED via temp stub (restored
  byte-identical): `get()` forced `None` => 2 passed / 3 failed; GREEN re-run
  5/5 each; rustfmt drift at `int_registry_lane.rs:160` left untouched.
- `worklog/AUDIT-NONACCEPTED.md:42`: `INT-001` in-progress; task-candidate
  `crates/providers/src/registry.rs` Y on disk and wired; lane-variant
  `int_registry_lane.rs` with test Y; wired Y lane-variant.
- `crates/providers/src/lib.rs:49`: `pub mod registry` present (canonical file
  wired); lane-variant `int_registry_lane` has no `pub mod` line in `lib.rs`
  (unwired local module). Passing `#[path]`-include lane suites therefore prove
  local module behavior only, not crate or application wiring.
- `worklog/INTEGRATION-18.md:211` / `INTEGRATION-19.md:219`: `INT-001`
  in-progress with `y-caveat`; CAVEAT is quiesced-tree RED re-run formality.
  Candidate receipts only, not acceptance.

### Source gap and authority blockers

- `sources/integrations-ownership-gap.json:33`: `ownershipDecision.INT-001`
  is `null`; status `source-reviewed-no-exact-task-owner`.
- Same file `taskBindingState.INT-001`: `controllerStatus: in-progress`,
  generic ralph story, `requirementIds: []`, `taskCard: null`, `worklog: null`.
  The pre-existing `tasks/INT-001.md` / `worklog/INT-001.md` pair is not bound
  in the machine-checked record, so it cannot transfer ownership.
- `excludedOwners`: `INT-002` explicit-blocker (repository-contract-mixes-side-
  effects), `INT-004` local-implemented-stale (`d8fa4854`), `INT-008`
  local-implemented-stale event-wire-compatibility, `INT-010`
  guarded-enterprise-remote-spec-gap. INT-001 must not re-own any of these.
- `sources/enterprise-remote-spec-gap.json`: residual `INT-010`, `SHARE-003`,
  `WEB-004` only; `INT-001` is not an enterprise residual.
- Preserve per card + gap: no secret/key material, no env lookup, no provider
  authorize/refresh callbacks, no OAuth auto-callbacks, no network, no HTTP
  routes, no persistence, no events, no background scrubber.

### Backlog, validator, convergence

- `sources/backlog-exhaustion.json` INT-001 row: `unresolved-decomposition`,
  `controllerStatus: in-progress`, `reasonKey: integration-family-not-decomposed`,
  `taskCard: null`, `worklog: null`, `implementationCommits: []`,
  `surfaceIds: ["opencode.integrations"]`.
- `tools/validate_backlog_exhaustion.py` (51 error lines repo-wide): 1
  attributable INT-001 guard error:
  `INT-001: integrations ownership-gap is stale after Ralph semantics changed`.
  Validator at `:2042` expects ralph status `in-progress` for INT-001
  (`not-started` only for INT-009); live `ralph.json` shows `accepted`, so the
  freshness check fails. No `task/worklog appeared` error fires for INT-001
  (card/worklog predate the gap baseline), unlike OPS/SHARE rows.
- `tools/validate_backlog_exhaustion.py:2071-2090` requires the exhaustion row
  to remain unresolved with no local task ownership or implementation commits.
- `tools/validate_backlog_exhaustion.py:2010-2060`
  (`integrations_ownership_gap_errors`) pins null ownership, binding set, live
  surface signatures, equivalence group, and the INT-002/004/008/010 exclusion
  set. No worker may patch these.
- Convergence/gate state is controller-owned and unchanged by this lane; no
  heavy commands run here per lane bounds.

## Verification

- Claim held via `tools/completion_claims.py` before this file was created.
- Existing receipts only (INT-001 lane 5/5, providers regression per
  `worklog/INT-001.md`); no test command run in this audit.
- Pending after ledger update: `git diff --check` and ledger row inspection
  only. No product or test verification claimed.

## Remaining unknowns

- Controller must choose one canonical reconciliation of the `accepted` plan
  state versus the null-owner/unresolved source-gap/backlog model.
- No authority in this lane to wire the lane-variant module, assign the
  canonical-vs-lane file question, or authorize secret/auth/network side effects.
- Acceptance, release readiness, and parent completion remain external
  controller/verifier decisions.
