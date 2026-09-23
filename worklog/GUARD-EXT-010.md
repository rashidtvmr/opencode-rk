# GUARD-EXT-010 audit (audit-only, no acceptance)

## Claim
- Ledger `EXT-010`: `in-progress`, session `ses_f320de58affePYMhgJFtkWUXAu`, scratchpad `worklog/GUARD-EXT-010.md`.
- Owns ONLY this scratchpad + EXT-010 ledger row. No canonical/product/test/validator edits. No Cargo/network/heavy commands.

## Source evidence (exact)
- Card: `tasks/EXT-010.md:1-9` Status NOT STARTED, kind product, runtime-optional True, test obligations EXT-010-T01..T05; `:29-52` owns only configured-external-discovery intent recording, cites `sources/disc-003-reconciliation.json` finding `opencode.extensibility` (partial/partial) + `sources/extensibility-remaining-ownership-gap.json` generic-disc-placeholder group (EXT-004/006/010/011, ownershipDecision null).
- Ralph: `ralph.json` EXT-010 `status: accepted`, story generic DISC-002 placeholder, requirementIds [], testObligations T01-T05.
- FEATURES: `FEATURES.md:53` EXT-010 accepted (I18 s7 r11, gate d); `:623` + `:881` accepted, scope via behavior-surface-rules.json.
- Gap: `sources/extensibility-remaining-ownership-gap.json` status `source-reviewed-no-exact-task-owner`, commit `95daf90670b7c039c436c85537da5fbfe2205b41`; ownershipDecision.EXT-010 null; equivalence `generic-disc-placeholder` (EXT-004/006/010/011, req [], sig opencode.extensibility); reviewedPartitions: `plugin-v2-lifecycle` owner null (shared-lifecycle blocker), `configured-external-js-ts-plugin-loading` owner null (blocker: requires JS/npm runtime + package loading, outside native lane unless new pinned ownership/runtime authority), `built-in-plugin-composition` owner null; taskBindingState.EXT-010 controllerStatus in-progress, ralphStory generic, requirementIds [], taskCard null, worklog null.
- Ledger: `sources/backlog-exhaustion.json` EXT-010 `unresolved-decomposition` / `in-progress` / reasonKey `extensibility-family-not-decomposed` / taskCard null / worklog null / implementationCommits [] / surfaceIds [opencode.extensibility].
- Validator: `tools/validate_backlog_exhaustion.py:1851-1960` `extensibility_remaining_gap_errors`; story set `:102`, groups `:103-106`, unresolved partitions `:107-113`; ralph must be `in-progress` + generic story (`:1883-1900`); task/worklog absence enforced (`:1908-1910`); ledger projection must stay null/empty (`:1940-1952`).
- Prior impl lane: `worklog/EXT-010.md` claims `crates/tools/src/plugin_discover.rs` wired in lib.rs, canonical `plugin_discover` 5/5 + lane mirror 5/5, frozen test sha `ea6c2c68...`, no RED re-run (impl predates lease at `248f519`), dedupe refused. `crates/ext/` absent on this tree (card-suggested `crates/ext/src/external_discovery.rs` never created here).

## Attributable guard errors (exact, EXT-010 only)
1. `EXT-010: remaining extensibility gap is stale after Ralph semantics changed` - ralph `accepted` vs validator expects `in-progress` + generic story. FEATURES accepted corroborates ralph, same conflict.
2. `EXT-010: task/worklog appeared; remaining extensibility gap needs deliberate review` - `tasks/EXT-010.md` + `worklog/EXT-010.md` exist on disk vs gap binding + validator require absent.
- Ledger projection row itself still null/empty (no drift on row fields); drift is disk-vs-record, controller-owned.
- Repo-wide context (not attributed): sibling guard lanes report `validate_backlog_exhaustion.py` 51 errors / convergence total 80; not re-run here per lane bounds.

## Isolated evidence vs wiring vs acceptance
- Isolated: prior lane asserts 5/5 GREEN on discovery-boundary unit tests (record/list, revoke, validation/caps/drain, no-loading, zero-cost-off). Taken as lane self-report only.
- Wiring: `lib.rs` wiring + `#[path]` twin + no-RED-predates-lease all asserted in `worklog/EXT-010.md`, not independently verified by this guard; twin noted as integrator-owned dedupe refusal.
- Acceptance: NONE. Ralph/FEATURES `accepted` contradicts gap (`ownershipDecision` null, binding nulls) + ledger (`unresolved-decomposition`, `in-progress`). Guard grants no ownership, no acceptance.

## Preserved blockers
- Sandbox/runtime: real JS/npm runtime + package loading/install/import outside authorized native lane; boundary is record-intent/refuse-loading by design.
- Permission: no new pinned ownership or runtime authority has appeared; `candidateTaskOwner` null on the exact partition.
- Resource: deliberate deviation cap `EXT10_MAX_DEFERRED=128` stands; upstream supplies none; zero-traversal/zero-network/zero-activation invariant must hold for any follow-up.

## Proposal (controller/integrator only)
- Either retire Ralph/FEATURES `accepted` to `in-progress` (or decompose EXT-010 with source-grounded ownership + runtime authority), or deliberately reconcile gap+binding+ledger to the landed boundary with caller-wiring proof + frozen RED receipt. Guard makes neither edit.

## Verification
- No tests run (audit-only per orders). Evidence: read-only greps/reads listed above. No product/test/canonical/validator modifications.
