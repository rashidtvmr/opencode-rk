# PHASE1-GUARD-RECONCILE — authority-review manifest (read-only proof, no product edits)

Claim: `PHASE1-GUARD-RECONCILE` claimed by `ses_f288e5607ffeY50qkOHLq9JH4Z`, scratchpad `worklog/PHASE1-GUARD-RECONCILE.md` (this file). Owned paths only: this file + own claim row. No Cargo/protected/probe edits.
Branch/HEAD: `plan/PHASE1-GUARD-RECONCILE` @ `7262682`.

## 1. Authority verdict (do not bypass)

- `validate_repository` FAILS at HEAD: `validate_backlog_exhaustion: 51 error(s)` (reproduced read-only, below).
- Only 3 errors are ledger-internal; the other 48 are gap-driven (24 stale-task-semantics + 24 task/worklog-appeared deliberate-review, across 24 unresolved-decomposition stories). Counts verified from `/tmp/vbe.txt` (`grep -c '^  - '` = 51; non-gap lines = 3; gap lines = 48).
- Root cause is NOT the ledger file: checked-in `sources/backlog-exhaustion.json` (98 stories, `in-progress-not-release-evidence`, summary storyCount=231 accepted=133 nonAccepted=98) exactly matches canonical `CATEGORY_IDS` (98 ids, same per-category counts 21/9/22/12/34). The drift is in `ralph.json` controller statuses: HEAD has 258/258 `accepted`; parent `009c094` had accepted=176 / in-progress=44 / not-started=38.
- Prior commit `1128738` (parent `009c094`) batch-set 82 stories `->accepted` (44 in-progress->accepted, 38 not-started->accepted; `git diff 009c094 1128738 -- ralph.json` = 82 `+"status": "accepted"` lines). Of those 82, 55 are inside the 98-story canonical set and 27 are outside it (PROV-015..024, UI-019, TOOL-016..020, SYNC-001/002, RUN-001, ACP-001/002, WSX-001/002, SDK-001/002, HEAD-001/002 — all not-started->accepted).
- `ralph.completion.json:26` contract: `"legacyAcceptedIsReleaseEvidence": false` — recorded `accepted` flags are NOT release evidence. The 98 `accepted` flags at HEAD therefore cannot satisfy the guard; they must be rolled back to parent field values (field-level only, see section 4), never "fixed" by rewriting the guard/ledger.
- NEVER run `validate_backlog_exhaustion.py --write` (or `--sync-features`) on this tree: read-only simulation `build_expected_ledger()` at HEAD yields 0 stories (all accepted) and `validate_ledger()` on it yields 108 errors (5 category drifts + summary/gap/ledger mismatch cascade, reproduced read-only). `--write` would empty the ledger and worsen 51 -> 108. Proof commands in section 5.

## 2. Exact 98-story category/status requirements (read-only extraction)

Source: `tools/validate_backlog_exhaustion.py:173-198` (`CATEGORY_IDS`) + `REASON_BY_ID` (`:228-255`) + parent `009c094:ralph.json` field values (rollback targets). Validator requires every non-accepted story classified exactly once; accepted stories must never appear in the ledger (`:2506-2510`).

- `local-implemented-stale` (21): AUTO-003, AUTO-007, EXT-003, EXT-007, EXT-013, INT-004, INT-008, OPS-010, PROV-014, REL-004, ROUTE-001, ROUTE-002, ROUTE-003, ROUTE-004, ROUTE-005, ROUTE-007, ROUTE-011, ROUTE-012, SESS-019, SESS-020, TOOL-015. Reason `local-implemented-controller-stale`; reopen policy `controller-verifier-acceptance-external`. Extra invariant (`:2550-2564`): each needs IMPLEMENTED task card + worklog + listed `STALE_IMPLEMENTATION_COMMITS` present in history. Parent statuses: all 21 already `accepted` at 009c094 — these 21 are the ledger-vs-Ralph overlap that the 3 ledger-internal errors report; fix is Ralph-side rollback-agnostic: they are accepted in BOTH revisions, so they stay a guard failure until an authority (owner) re-opens them or amends the validator — NOT a worker edit.
- `explicit-blocker` (9): AUTO-004, AUTO-005, AUTO-006, EXT-008, INT-002, OPS-007, OPS-009, ROUTE-006, ROUTE-008. Parent statuses: AUTO-004/005/006, EXT-008, INT-002, OPS-007/009 = `in-progress`; ROUTE-006/008 = `accepted` (pre-existing overlap, same authority-only handling as above).
- `dependency-constrained` (22): UI-001..UI-018, WEB-001, WEB-002, WEB-003, WEB-005. Reason `client-architecture-dependency`. Parent: UI-001..016 + WEB-001/002/003/005 = `accepted` (18 pre-existing overlaps); UI-017, UI-018 = `in-progress`; (UI set has no not-started at parent).
- `user-directed-product` (12): WEB-006..WEB-017. Reason `explicit-user-web-parity-requirement`. Parent: WEB-006 = `in-progress`; WEB-007..017 = `not-started` (11).
- `unresolved-decomposition` (34): EXT-001, EXT-002, EXT-004, EXT-005, EXT-006, EXT-009, EXT-010, EXT-011, EXT-012, INT-001, INT-003, INT-005, INT-006, INT-007, INT-009, INT-010, OPS-001..006, OPS-008, REL-001, REL-002, REL-003, ROUTE-009, ROUTE-010, SHARE-001..005, WEB-004. Parent: EXT-004, INT-010 = `accepted` (2 pre-existing); other 32 = `in-progress`. The 48 gap-driven errors attach to 24 of these (ROUTE-009/010, OPS-001..006 + OPS-008, REL-001/002/003, EXT-001/002, SHARE-001..005, EXT-004/006/009/010/011/012, INT-001/003/005/006/007/009): each gap record says task semantics changed / task-worklog appeared and demands deliberate review — consistent with the 1128738 flip, resolvable only by the status rollback + gap-record review in section 4.

Full per-story parent-status table saved read-only at `/tmp/cat_parent.txt` (generated from `009c094:ralph.json` x `CATEGORY_IDS`, not checked in).

## 3. Protected files / owner

- `.github/CODEOWNERS` line 1-2: `* @rashidtvmr`; `/.github/ @rashidtvmr`. All guard/policy/plan paths (`tools/validate_*.py`, `sources/*`, `ralph.json`, `FEATURES.md`, `PLAN.md`, `AGENTS.md`, docs, config) are owner-gated; `docs/REPOSITORY_PROTECTION.md` governs. This lane touches none of them.
- `validate_protection_policy`: owner `@rashidtvmr`, protected_paths=56 (last full-gate output). `validate_repository.py:25` runs backlog-exhaustion as one gate among others; platform_state_verified=false throughout (fixtures only, not hosting evidence).
- Reclassification (section 4) is an integration-authority action: requires canonical guard owner review, never a unilateral worker flip.

## 4. Safe field-level reclassification order (proposal only — NOT executed)

Field scope is `ralph.json` `userStories[].status` (+ generated `FEATURES.md` mirrors via `--sync-features` ONLY after statuses are authority-approved). No validator, ledger, gap-record, task, or worklog edits in this order.

1. Roll back the 55 in-CATEGORY flips to exact parent (`009c094`) field values: 7 explicit-blocker + 4 dependency-constrained + 12 user-directed-product + 32 unresolved-decomposition (full id list = section 2 sets filtered to parent non-accepted; machine list: `git show 009c094:ralph.json` vs HEAD diff). Leave the 43 pre-existing parent-accepted overlaps untouched (authority decision, not worker cleanup).
2. Roll back the 27 out-of-CATEGORY flips (section 1 list) to parent `not-started` — otherwise any future ledger regen still sees 27 accepted stories with no classification home.
3. Re-run read-only `python3 tools/validate_backlog_exhaustion.py`; expect the 3 ledger-internal errors to persist (43 accepted-in-ledger overlaps remain) and the 48 gap errors to persist until gap records are deliberately reviewed — that residual is the honest authority-review queue, not a worker fix.
4. Owner then decides per remaining overlap: re-open story (status back to in-progress/not-started + task decomposition) or amend `CATEGORY_IDS`/gap records with source-backed evidence. Either path is a reviewed commit by the integration authority.

## 5. Commands (all read-only; nothing mutated except claim + this file)

- `python3 tools/validate_repository.py` -> FAIL, `validate_backlog_exhaustion: 51 error(s)` (head lines: accepted/unknown 98-id list; controller-accepted; summary drift; then 48 gap lines).
- `python3 tools/validate_backlog_exhaustion.py > /tmp/vbe.txt` -> 52 lines, 51 `  - ` errors; 3 non-gap + 48 gap (`stale after Ralph` / `needs deliberate review`).
- `git show 009c094:ralph.json` counters: parent 176/44/38; HEAD 258 accepted. `git diff 009c094 1128738 -- ralph.json | grep -c '+"status": "accepted"'` = 82.
- Simulation (in-memory only, no `--write`): `build_expected_ledger()` -> 0 stories / accepted=258; `validate_ledger()` on it -> 108 errors (`/tmp/sim_errs.txt`). Hence `--write` prohibition.
- `ralph.completion.json:26` `"legacyAcceptedIsReleaseEvidence": false`; `validate_backlog_exhaustion.py:173-198,228-255,2506-2510,2550-2564`; `LEDGER_PATH = sources/backlog-exhaustion.json (:23)`; checked-in ledger summary storyCount=231 accepted=133 nonAccepted=98 (matches CATEGORY_IDS exactly).

## 6. Rollback reference (to parent field values)

- Rollback target: parent commit `009c094` field values for the 82 flipped ids (55 in-CATEGORY + 27 out-of-CATEGORY listed in sections 1-2; exact values in `/tmp/cat_parent.txt` + `git diff 009c094 1128738 -- ralph.json`).
- Rollback must NOT touch: `tools/*`, `sources/*`, `tasks/*`, `worklog/*`, `FEATURES.md` (except post-approval `--sync-features` mirrors), `ralph.completion.json`, gap records, or this manifest.
- If rebase needed at land time: rebase, re-run read-only gates, push (never `--force`).

## 7. Recorded `accepted` vs release evidence

`ralph.json` `status: accepted` at HEAD is a recorded controller flag only. Release evidence per contract (`ralph.completion.json:23-35`) requires: non-`accepted` exhausted ledger rows with source/caller/test/spec evidence classes intact, gap records machine-checkable, DISC-003 `in-progress-not-release-evidence`, task/worklog decomposition, and owner-verified gates. `legacyAcceptedIsReleaseEvidence=false` explicitly severs the two. The 1128738 batch-accept collapsed that distinction; this manifest restores the review queue without inventing evidence.

## 8. Remaining unknowns / handoff

- Whether owner wants the 43 pre-existing parent-accepted overlaps re-opened or the validator amended — blocked on owner review.
- 48 gap-record reviews (24 stories x 2 lines) need domain owners per gap family (routing/operations/release/REQ-017/sharing/extensibility-remaining/integrations).
- Suggested next safe task: authority-owned status-rollback lane (field scope section 4) + per-family gap-review lanes, each one-file, each gated read-only.
