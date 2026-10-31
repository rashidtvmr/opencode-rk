# CONV-001 - Phase 1 scope closure: dispositions, durable completion, single source of truth

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes (process gate; unblocks Phase 2 sequencing).
Requirements: none newly introduced (process discipline; see sources/backlog-exhaustion.json as input).
Dependencies: none newly introduced by this slice.
Test obligations: CONV-001-T01, CONV-001-T02.
Ownership locks: tasks/CONV-001.md only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations, tools/, tests/bootstrap/, sources/*.json, protectedPaths files.

## User-observable outcome

A reader asking "what is left in Phase 1" gets one authoritative answer: every one of the 216 existing task cards carries an explicit disposition (closed-done, closed-cut, or deferred-with-reason), `completed` in the claims ledger means "frozen tests passed on the recorded tree hash" and can never be revoked by a later semantic reinterpretation, and a single named source of truth (this card + the ledger) replaces the competing scope documents. Phase 2 cards (RENAME-001, GUI-001) become sequenceable instead of floating.

## Source evidence

- PLAN.md: "The seed contains 193 work items, but the final number may grow" — open-ended seed, no closure criterion.
- Measured repo state (2026-09-28): 216 cards in tasks/*.md; 99 with `Status: NOT STARTED`; ~30 cards with no claim entry in tasks/completion/claims.json (216 cards vs 186 claims).
- Baseline failure (2026-09-26): `python3 tools/validate_repository.py` FAILs with "SHARE-005 ... stale after Ralph semantics changed" plus 21 sibling entries — a global semantic change retroactively un-finished completed work.
- tools/completion_claims.py:13: valid transitions are `not-started -> in-progress -> completed` only; nothing in the ledger supports revoking `completed`.
- tools/validate_backlog_exhaustion.py:262-268 `_task_status()`: validator parses `^Status:\s*(.+?)\.\s*Kind:` from cards — card Status-line format is load-bearing; this card follows the ACP-001 template exactly.
- CATEGORY_IDS local-implemented-stale block (tools/validate_backlog_exhaustion.py:174-182): 22 story IDs whose completed status was invalidated by reinterpretation.
- Competing scope documents: current-checkout-inventory.json (17,169 lines), latest-main-inventory.json (17,158 lines), ralph.json (4,023 lines), prd.json (2,707 lines), 42 sources/*.json reconciliation files.

## Observable contract

- `disposition(card_id) -> {closed-done | closed-cut | deferred-with-reason}`: total function over the 216 existing cards. Every card keeps its file; the disposition is recorded in a single machine-readable manifest written by this lane and pinned in this card's verification record.
- `closed-done` requires: frozen tests green on the recorded tree hash (rerun, not assumed), claim status `completed` in the ledger.
- `closed-cut` requires: recorded cut reason (duplicate, superseded, out-of-scope-with-owner-sign-off). Cut is not deletion: the card stays, the ledger notes the cut.
- `deferred-with-reason` requires: target phase and the blocking condition that would re-activate it.
- `completed` is durable: once a claim records `completed` with a tree hash + frozen-test hash, no later reinterpretation, semantic change, or reconciliation may move it back to `not-started`/`in-progress`. Reopening requires a NEW card (new ID, new RED suite) citing the old one — never a status rewrite.
- Single source of truth after this card: this card's disposition manifest + tasks/completion/claims.json. All other scope documents (inventories, ralph.json, prd.json, FEATURES.md, RAW_FEATURE.md) are explicitly demoted to inputs, not authorities. No edits to those files in this lane (several are protected); the demotion is a statement of authority recorded here, not a file rewrite.
- Explicitly excluded: implementing any of the 99 unstarted product cards; touching tools/, tests/bootstrap/, sources/*.json, PLAN.md, AGENTS.md, ralph.json, or any protectedPaths file; rewriting completed claims' history.

## State and persistence transitions

- Ledger: `completed` rows gain no new transitions; the transition table is unchanged. What changes is the recorded policy: no future tooling or manual edit may regress a `completed` row.
- Manifest: absent -> authored -> pinned (hash recorded in the lane's verification evidence).
- Cards: unchanged files; dispositions live in the manifest, not as card edits (editing 216 cards is churn; the manifest is the queryable surface).
- Persistence: manifest is a new file owned by this lane; format is plain JSON, stdlib-parseable, no new tooling required.

## Failure states

- A card with no determinable disposition: lane BLOCKS and lists it, rather than guessing. (Undeterminable scope is the defect this card exists to surface.)
- A `completed` claim whose frozen tests no longer pass on rerun: disposition `closed-done` is DENIED; the card goes to a NEW follow-up card, the old claim row is annotated (not rewritten) with a pointer to the new card.
- Any attempt to resolve this by editing a protected file or a frozen test: out of scope, stop and hand to the integrator/owner.
- Secret safety: manifest contains card IDs, statuses, hashes only; no credentials, no DB contents. Tests use disposable fixtures.

## Resource bounds

- Manifest bounded by card count (~216 rows, tens of KB). No queues, no retained output, no background tasks, no network.
- Ledger reread is a single bounded file parse; no unbounded scan.

## Test obligations (frozen)

- CONV-001-T01 (disposition totality): given the card list snapshot pinned in the manifest, every one of the 216 cards has exactly one disposition; zero un-dispositioned, zero double-dispositioned.
- CONV-001-T02 (completion durability): given a claim row recorded `completed` with tree hash H and frozen-test hash F, no input consisting of a semantic reinterpretation (simulated by a stale-gap fixture) may change its status; the only legal forward step is a new card ID referencing it. Asserted against the ledger read path, disposable copy, never the live claims.json.

## TDD steps

1. Inspect: tools/completion_claims.py transitions, validator `_task_status` parser, claims.json schema (done for the contract above).
2. Author tests CONV-001-T01..T02 against disposable fixtures; establish compiling RED.
3. Freeze test hash + command manifest.
4. Implement: manifest builder + durability assertion helper (new module owned by this lane; exact path left to the lane but must NOT be a shared file — e.g. a new file under tools/ is FORBIDDEN since tools/ is integration-authority; put it in tasks/ as data or a new standalone file the integrator wires).
5. GREEN, refactor, rerun; negative tests (undeterminable card, stale-completed).
6. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/CONV-001.md
command python3 -c "import tools.completion_claims as c; d=c.load_ledger('.'); print('CONV-001' in d['claims'])"
command python3 tools/validate_backlog_exhaustion.py 2>&1 | tail -3
```

New cards must appear in NEITHER the validator's error list NOR its story set (they are not Ralph stories): confirm zero lines mentioning CONV-001 in the output above. Claims are written by the orchestrator at lease time via `tools/completion_claims.py::claim()` — there is no CLI; do not hand-edit claims.json.

## Ownership note

Task-card model mirrored from tasks/ACP-001.md (Status-line format is validator-parsed). This card owns its manifest only. Dispositions reference other cards but edit none. Validator code, ledger schema, claims history, and all protected files are explicitly out of scope. Follows the program rule: a program card is not leased directly; if this card decomposes, leaves carry their own IDs and single owned files.
