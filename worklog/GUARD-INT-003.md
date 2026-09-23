# GUARD-INT-003 - Audit-only guard for INT-003

## Claim
- Task: INT-003 (Provider-specific integration method dispatch boundary)
- Session: `ses_f320cfa07ffeGyV62fZG63ZZUe`
- Scratchpad: `worklog/GUARD-INT-003.md`
- Status held: in-progress -> blocked (see Blocker)
- Owned files (write scope): `worklog/GUARD-INT-003.md` only; `tasks/completion/claims.json` ledger row only.

## Scope
Audit-only guard lane. No product/test/source edits, no Cargo, no validator runs. Trace INT-003 through
ralph/card/worklog/FEATURES/validator and identify discrepancy. Retain auth/permission/cancellation/bounded-resource blockers.

## Source evidence (exact path/lines)

### Task surface
- `ralph.json:1059-1071` — INT-003 `id`, status `accepted`, userStory:
  "Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json",
  testObligations T01-T05.
- `prd.json:835-843` — INT-003 mirrors ralph.json (accepted, T01-T05).
- `FEATURES.md:58` — `| INT-003 | accepted | I18 s7 r16, gate c | quiesced-tree re-run formality |`.
- `FEATURES.md:633` — INT-003 accepted row, surface `opencode.integrations`.
- `FEATURES.md:885` — INT-003 (accepted) discovered during DISC-002.

### Task card + implementation actually present
- `tasks/INT-003.md` (142 lines) exists on disk — full observable contract, failure states,
  resource bounds, frozen test obligations T01-T05, TDD steps.
- `crates/providers/src/int_methods.rs` (244 lines) exists — `MethodTable`, `dispatch`,
  `register`, `ProviderMethodKind`, `MethodError`, consts `MAX_PROVIDERS=64`,
  `MAX_METHODS_PER_PROVIDER=16`, `MAX_ID_LEN=64`, `MAX_INPUT_BYTES=4096` matching T03/T04/T05.

### Discrepancy sources (stale state)
- `sources/integrations-ownership-gap.json:42` — INT-003 `taskCard: null, worklog: null`
  (stale: task card + worklog actually present on disk).
- `sources/integrations-ownership-gap.json:31-45` — equivalenceGroup lumps
  INT-001/INT-003/INT-005/INT-006/INT-007/INT-009 as source-identical residual; conclusion:
  controller status alone is not semantic ownership.
- `sources/integrations-ownership-gap.json:101-117` — `taskBindingState.INT-003` reports
  `controllerStatus:in-progress`, same generic DISC-002 story, no requirements, taskCard:null, worklog:null.
- `sources/backlog-exhaustion.json:481-493` — INT-003 category `unresolved-decomposition`,
  `taskCard: null`, `worklog: null`, `localEvidencePaths: []`, `implementationCommits: []`,
  `reasonKey: integration-family-not-decomposed`, `reopenPolicy: source-grounded-task-decomposition-required`.
- `sources/behavior-surface-rules.json:456-487` — broad `opencode.integrations` surface
  (workspace + external protocols) lists INT-003 alongside INT-001..INT-010.
- `sources/completion/audits/AUD-009.json:40` — INT-003 in `assignedLegacy`; `:98-106` sample
  truncates obligations to T01-T03 (not full T01-T05); `:126` verdict "no acceptance claimed".

### Retained auth/permission/cancellation/bounded-resource blockers
- `sources/integrations-ownership-gap.json` sections:
  - `ptyBoundaryConstraints` (ticketTtlSeconds 60, ticketCapacity 10000, singleUseConsume,
    websocketOutboxBoundEstablished:false, gracefulShutdownTrackingEstablished:false) — INT-009.
  - `integrationAuthConstraints` (attemptLifetimeMinutes 10, terminalRetentionMinutes 1,
    scrubIntervalSeconds 30, refreshWindowMinutes 5, providerCallbacksExecute:true,
    autoCallbackRunsInBackground:true, credentialPersistenceMutates:true,
    environmentConnectionsReadProcessState:true, committedCredentialChangesPublishEvents:true,
    refreshDirectIntegrationTestEstablished:false, lifecycleConstantSpecEstablished:false).
- `sources/completion/audits/AUD-009.json:51` dispatch trace references server/src/acp_bridge.rs,
  tools/src/mcp.rs, etc.; findings mark MCP/LSP/Git lanes `unverified`/`missing`/`unwired`.

## Observable contract (INT-003, from tasks/INT-003.md)
- `ProviderMethodKind`: Key | OAuth | Env | ProviderNamed { provider, method } (validated `a-z0-9-_`, max 64 B).
- `MethodTable::register(provider, method, handler_id) -> Result<(), MethodError>`
  bounded static: MAX_PROVIDERS=64, MAX_METHODS_PER_PROVIDER=16; dup replace in place; TableFull on overflow.
- `dispatch(provider, method, &MethodInput) -> Result<MethodReceipt, MethodError>`;
  UnknownProvider/UnknownMethod/InvalidId/TableFull; receipt {handler_id, accepted}; NO network/secret/env/event.
- `list_methods` / `providers` sorted; determinism by order-insensitive set.
- Module boundary: `crates/providers/src/int_methods.rs` additive; integrator wires exports.

## Discrepancies found
1. Equivalence-class drift: INT-003 (and sibling INT-00x) declared
   `unresolved-decomposition`/`integration-family-not-decomposed` in
   `sources/integrations-ownership-gap.json` and `sources/backlog-exhaustion.json`,
   yet `tasks/INT-003.md` + `crates/providers/src/int_methods.rs` exist and are
   implemented. Gap JSON `taskCard:null`/`worklog:null`/`implementationCommits:[]` is stale.
2. Controller/ledger drift: `validate_repository.py` (run earlier) reports
   "INT-003: integrations ownership-gap is stale after Ralph semantics changed" and
   "backlog exhaustion classifies accepted stories" (INT-003 in that accepted set).
   Ledger has not been reconciled to the local decomposed task/implementation.
3. Real-caller gap: AUD-009 sample truncates INT-003 obligations T01-T03, while
   ralph.json/prd.json enumerate T01-T05; no `localEvidencePaths` bind the task card
   to the audit despite `int_methods.rs` existing on disk.
4. Duplicate/equivalence: INT-001..INT-010 share identical generic DISC-002 story +
   `opencode.integrations` surface; INT-003 overlaps INT-004/INT-005 in overlap tables.

## Decision
Audit-only. No product/test edits. Blocked by staleness ledger drift + obligation
truncation + auth/cancellation resource bounders that belong to separate INT rows
(owned by their integrators). INT-003 implementation exists; the gap is the
stale ledger binding, which is outside this lane's authority (controller/ledger/policy files).

## Remaining unknowns / out-of-scope
- Whether INT-003 should be reclassified from `unresolved-decomposition` to a bound
  decomposed slice in backlog-exhaustion.json (requires policy edit — out of lane scope).
- AUD-009 obligation truncation (T01-T03 vs T01-T05): audit artifact only; not edited.
- Auth/refresh/network/resource-lifetime blockers retained per INT-002 (frozen),
  INT-010 (enterprise-remote missing-spec), and PTY ticket boundary.
