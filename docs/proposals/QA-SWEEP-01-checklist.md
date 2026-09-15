# QA-SWEEP-01: proposal + task-card audit checklist

Scope: all 16 files in `docs/proposals/` + new task cards (`tasks/REL-005.md`,
`tasks/REL-007.md`, `tasks/TOOL-021.md`). Auditor: Feathers. Date: 2026-09-15.
Standard: AGENTS.md no-stub policy, docs/TDD.md sections 2-5.

## 1. Per-file matrix

Y = present, P = partial, N = missing. "5T" = five frozen product tests
(T01..T05) or, for RES, inventory/research table.

| # | File | Lines | 5T/Table | Opt-out default-on | No secret logging | Verification cmds |
|---|------|------:|:--------:|:------------------:|:----------------:|:-----------------:|
| 1 | CV-RES-01-caveman-research.md | 61 | Y (inventory + risk table) | P (default lite; ultra opt-in) | P (never compress approvals; no log ban) | N |
| 2 | CV-SLICE-01-render-spec.md | 97 | Y (CV-RND-T01..05) | Y | N | Y (`cargo test -p opencode-rk-render`, check, status) |
| 3 | CV-SLICE-02-clarity-spec.md | 110 | Y (CV-CLR-T01..05) | Y (guard locked on) | N | P (grep counts + status; no cargo target) |
| 4 | HR-RES-01-telemetry-research.md | 54 | Y (metric + privacy tables) | N (default OFF, deliberate) | Y (never logs secrets/keys/prompts/bodies) | N |
| 5 | HR-SLICE-01-collect-spec.md | 123 | Y (HR-COL-T01..05) | Y (`enabled:true` default) | P ("no secrets" line 13, no absence test ref) | Y (test, check, fmt, determinism) |
| 6 | HR-SLICE-02-report-spec.md | 97 | Y (HR-RPT-T01..05) | Y | Y (absence asserted, redaction-first) | Y (test, check, fmt, golden text) |
| 7 | REL-SLICE-01-ralph-integration.md | 104 | Y (12-story table) | Y (REQ-039 text) | N | Y (owner ordered validate_* chain) |
| 8 | RTK-RES-01-rtk-research.md | 53 | Y (filter table + issue list) | P (passthrough safe; heuristics never default) | P (I-07 secret-branch note only) | N |
| 9 | RTK-SLICE-01-core-spec.md | 94 | Y (RTK-CORE-T01..05) | Y | N | Y (test, check, fmt) |
| 10 | RTK-SLICE-02-test-spec.md | 96 | Y (RTK-TEST-T01..05) | Y (`--no-filter`/env) | N | Y (test, clippy, fmt, status) |
| 11 | RTK-SLICE-03-pass-spec.md | 96 | Y (RTK-PASS-T01..05) | Y (global + per-cmd) | N | Y (test, clippy, fmt, status) |
| 12 | RW-RES-01-repowise-research.md | 56 | Y (capability inventory) | P (10 tools default; risky 7 opt-in; telemetry off) | Y (fingerprint only) | N |
| 13 | RW-SLICE-01-index-spec.md | 77 | Y (RW-IDX-T01..05) | Y | Y (hashed, never logged) | Y (test, check, fmt, no-I/O proof) |
| 14 | RW-SLICE-02-query-spec.md | 100 | Y (RW-QRY-T01..05) | Y | Y (never write q/snippets to logs) | Y (test, check, fmt) |
| 15 | RW-SLICE-03-risk-spec.md | 124 | Y (RW-RSK-T01..05) | Y (`disabled()`) | N | Y (test x2, clippy, status) |
| 16 | SET-SLICE-01-optout-spec.md | 82 | Y (SET-OPT-T01..05) | Y (IS the opt-out; all `true`) | N (wrong-type Err; no log ban) | Y (test, check, stash note) + ADR-006 boundary |

New cards: `tasks/REL-005.md` (5T, mirrors HR-SLICE-01, `enabled:true`),
`tasks/REL-007.md` (exists), `tasks/TOOL-021.md` (exists). No `tasks/SET-*.md`
exists. No `tasks/TOOL-023.md` exists.

## 2. Gaps

1. Integration table stale: 12 stories cover only 7 slice specs.
   RTK-SLICE-03-pass-spec.md has NO story (needs TOOL-023).
2. SET-SLICE-01 filed generically as REL-008 "default-on opt-out (new)".
   Source column must name the SET spec; no SET-prefix cards exist.
3. SET vs REL numbering undecided: SET prefix needs `plan_model.py` phase
   change + validator edit. Cheaper: keep REL-008, point source at SET spec.
4. Research slices CV-RES-01, RTK-RES-01, RW-RES-01 have no REL stories
   (only HR-RES-01 has REL-007). Record deliberate exclusion or add REL-010..012.
5. REL-007 collision: `tasks/REL-007.md` already exists in repo; table
   redefines it as HR-RES research. Confirm content matches or renumber.
6. HR default contradiction: HR-RES-01 says telemetry default OFF;
   HR-SLICE-01 and REL-005 card say `enabled:true`. Reconcile before freeze.
7. CV-SLICE-02 verification weak: grep counts only, no `cargo test` target
   unlike every sibling slice. Add crate test command.
8. RES files lack executable validators (TDD sections 4,7): all 4 RES files
   have no Verification section. Add validator/fixture commands.
9. No secret-logging ban in CV-SLICE-01/02, RTK-SLICE-01/02/03,
   RW-SLICE-03, SET-SLICE-01, REL-SLICE-01. HR-SLICE-01 and RTK-RES-01
   partial only. Add one-line ban + absence-assert pointer each.
10. Obligation-ID mismatch: CV specs use CV-RND-/CV-CLR-Txx, RW/RTK/HR specs
    use family-Txx, but integration table mandates `{tid}-Txx`
    (TOOL-021-T01..05). Harmonize before freeze; recompute the "57 new IDs"
    claim after adding TOOL-023 + research validators.

## 3. Fix list for owner (ordered)

1. REL-SLICE-01 table: add row TOOL-023 = RTK-SLICE-03-pass-spec.md,
   RTK-PASS-T01..05; set REL-008 source to SET-SLICE-01-optout-spec.md.
2. Decide research coverage: exclusion note or REL-010..012 rows.
3. Resolve REL-007 collision against existing `tasks/REL-007.md`.
4. Settle HR default (OFF per research vs ON per slice/card); update the
   losing files in one commit.
5. CV-SLICE-02: add `cargo test` target to Verification.
6. All 4 RES files: add Verification with executable validator commands.
7. Add secret-logging ban line to the 7 N-files (and harden the 3 P-files).
8. Rename all spec test IDs to `{tid}-Txx` (or record mapping table);
   recount obligations; regenerate ledger via `--write`, `--sync-features`,
   `validate_plan.py`, `validate_repository.py`.
