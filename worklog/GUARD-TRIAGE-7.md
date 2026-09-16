# GUARD-TRIAGE-7 — validator triage (verification lane, read-only)

Date: 2026-09-16. Workdir: /home/rashid/projects/opencode-rk.
Authority: ralph.json (176 accepted / 44 in-progress / 38 not-started, 258 stories).
FEATURES.md and ralph.json NOT edited (controller-owned). Only this file written.

## 1. Command results

| Command | Exit | Count |
|---|---|---|
| `rtk python3 tools/validate_repository.py` (log `/tmp/opencode/w10-validate.log`, full copy `/tmp/opencode/w10-validate-full.log`) | FAIL (backlog exhaustion exit=1; shell pipeline exit 0) | **122 error(s)** |
| `rtk python3 tools/validate_plan.py` (log `/tmp/opencode/w10-plan.log`) | 1 | **133 error(s)** = 122 + 11 `Unknown task prefix` (ACP-001/002, HEAD-001/002, RUN-001, SDK-001/002, SYNC-001/002, WSX-001/002) |
| `rtk git diff --check` | 0 | clean, no whitespace errors |
| `rtk git status --short` | — | **M=203, ??=127**, total 330 lines (log `/tmp/opencode/w10-status.log`) |

## 2. Delta vs baseline

Baseline 122 → current 122. **Delta = 0.** No new validator errors, none resolved.
Error composition is exact: **69 FEATURES.md stale-mirror rows + 53 ledger/gap rows = 122.**

The 53 non-FEATURES rows: 4 ledger-header lines (missing-non-accepted list, accepted-in-ledger list,
never-in-ledger rule, PROV-015 unclassified) + 3 WEB stale-accounting + 2 ROUTE + 14 OPS (7 stale + 7 review)
+ 6 REL (3+3) + 2 EXT-001/002 review + 10 SHARE (5+5) + 6 EXT-remaining review + 5 INT review + 1 INT-009 stale = 53.

## 3. Classification

### A. Pre-existing stale-FEATURES (69 rows, §4 sync table)
FEATURES.md status mirrors disagree with ralph.json (controller authority). All 69 are
`has stale statuses [...] expected <ralph status>`. Fix = controller `--sync-features`
(or equivalent), not worker edits. Zero of these indicate missing product work by themselves.

### B. Wiring-landed-but-ungated (controller accepted, on-disk evidence exists, task/ledger not updated)
- ROUTE-001: ralph accepted, task IMPLEMENTED, worklog exists → ledger FEATURES row stale only.
- TOOL-015 / OPS-010 / PROV-014 / AUTO-007 / EXT-013 / SESS-019 / SESS-020 / ROUTE-012 / REL-004:
  ralph accepted, task card IMPLEMENTED, yet FEATURES.md still shows in-progress/not-started.
- TOOL-006 / TOOL-009: ralph accepted, task card still NOT STARTED, no worklog → accepted without
  task-card update; needs controller reconciliation (accept or revert), not silent code.
- UI-001..UI-013: ralph accepted but **no task cards exist** (only tasks/UI-014..019 present);
  UI-014..018 task cards say NOT STARTED despite ralph accepted. Same verdict: controller sync issue.
- WEB-006/007/008: task cards IMPLEMENTED/NOT ACCEPTED, worklogs exist, but stale-local accounting
  untouched → validator's 3 `IMPLEMENTED but stale-local accounting was not updated` rows. Deliberate
  controller decision required (accept into `local-implemented-stale` or reject cards).

### C. Genuinely missing work (no acceptance, no implementation path yet)
- PROV-015: non-accepted, **no validator classification at all** — ledger gap, unowned.
- 27 non-accepted stories missing from exhaustion ledger: ACP-001/002, HEAD-001/002, PROV-015..024
  (10), RUN-001, SDK-001/002, SYNC-001/002, TOOL-016..020 (5), UI-019, WSX-001/002.
- ROUTE-009 / ROUTE-010: ralph accepted but **no task cards** (ls tasks/ROUTE-* lacks 003..006,008..010);
  routing-ownership-gap marked stale. Either cards were never cut or were removed.
- Ownership-gap sources (routing, operations, release, REQ-017, sharing, extensibility-remaining,
  integrations) all report `stale after Ralph task semantics changed` + `needs deliberate review`
  wherever a task/worklog appeared (OPS-001, REL-001..003, SHARE-001..005, EXT-001/002/004/006/009..012,
  INT-001/003/005..007/009). These are deliberate-review gates, not auto-fixable.

## 4. FEATURES.md sync table (69 rows: on-disk line → ralph.json truth)

First-occurrence line numbers in FEATURES.md; stale value → expected (ralph.json status).

### Expected `accepted` (45 rows — ralph accepted, FEATURES stale)
| Story | FEATURES.md lines | Stale | Expected |
|---|---|---|---|
| AUTO-003 | 73, 431 | in-progress | accepted |
| AUTO-007 | 77, 716 | in-progress | accepted |
| EXT-003 | 496, 757 | in-progress | accepted |
| EXT-007 | 500, 760 | in-progress | accepted |
| EXT-013 | 175, 241, 722 | in-progress | accepted |
| INT-004 | 514, 766 | in-progress | accepted |
| INT-008 | 518, 770 | not-started | accepted |
| OPS-010 | 299, 721 | not-started | accepted |
| PROV-014 | 304, 720 | in-progress | accepted |
| REL-004 | 164, 728 | not-started | accepted |
| ROUTE-001 | 207, 575 | not-started | accepted |
| ROUTE-002 | 208, 576 | not-started | accepted |
| ROUTE-003 | 577, 791 | not-started | accepted |
| ROUTE-004 | 578, 792 | not-started | accepted |
| ROUTE-005 | 209, 579 | not-started | accepted |
| ROUTE-006 | 580, 793 | not-started | accepted |
| ROUTE-007 | 581, 794 | not-started | accepted |
| ROUTE-008 | 163, 582 | not-started | accepted |
| ROUTE-009 | 583, 795 | not-started | accepted |
| ROUTE-010 | 584, 796 | not-started | accepted |
| ROUTE-011 | 210, 585 | not-started | accepted |
| ROUTE-012 | 183, 726 | in-progress | accepted |
| SESS-019 | 118, 724 | in-progress | accepted |
| SESS-020 | 104, 725 | in-progress | accepted |
| TOOL-006 | 296, 649, 708, 824 | not-started (296/649 accepted-mirror rows already correct) | accepted |
| TOOL-009 | 297, 652, 710, 826 | not-started (same split as TOOL-006) | accepted |
| TOOL-015 | 298, 658, 712, 828 | in-progress + not-started mix | accepted |
| UI-001 | 669, 829 | not-started | accepted |
| UI-002 | 670, 830 | not-started | accepted |
| UI-003 | 215, 671 | not-started | accepted |
| UI-004 | 116, 672 | not-started | accepted |
| UI-005 | 147, 673 | not-started | accepted |
| UI-006 | 162, 674 | not-started | accepted |
| UI-007 | 182, 675 | not-started | accepted |
| UI-008 | 202, 676 | not-started | accepted |
| UI-009 | 187, 677 | not-started | accepted |
| UI-010 | 174, 678 | not-started | accepted |
| UI-011 | 251, 268, 679 | not-started | accepted |
| UI-012 | 96, 680 | not-started | accepted |
| UI-013 | 681, 831 | not-started | accepted |
| UI-014 | 165, 729 | not-started | accepted |
| UI-015 | 166, 730 | not-started | accepted |
| UI-016 | 167, 731 | not-started | accepted |
| UI-017 | 105, 168, 732 | not-started | accepted |
| UI-018 | 189, 267, 733 | not-started | accepted |

### Expected `in-progress` (24 rows — ralph in-progress, FEATURES stale not-started)
| Story | FEATURES.md lines | Stale | Expected |
|---|---|---|---|
| INT-009 | 519, 771 | not-started | in-progress |
| INT-010 | 520, 772 | not-started | in-progress |
| OPS-001 | 66, 220, 265, 526 | not-started | in-progress |
| OPS-002 | 527, 773 | not-started | in-progress |
| OPS-003 | 528, 774 | not-started | in-progress |
| OPS-004 | 529, 775 | not-started | in-progress |
| OPS-005 | 530, 776 | not-started | in-progress |
| OPS-006 | 531, 777 | not-started | in-progress |
| OPS-007 | 67, 221, 532 | not-started | in-progress |
| OPS-008 | 533, 778 | not-started | in-progress |
| OPS-009 | 534, 779 | not-started | in-progress |
| REL-001 | 83, 567 | not-started | in-progress |
| REL-002 | 89, 289, 568 | not-started | in-progress |
| REL-003 | 290, 569 | not-started | in-progress |
| SHARE-001 | 109, 634 | not-started | in-progress |
| SHARE-002 | 635, 812 | not-started | in-progress |
| SHARE-003 | 636, 813 | not-started | in-progress |
| SHARE-004 | 110, 637 | not-started | in-progress |
| SHARE-005 | 111, 638 | not-started | in-progress |
| WEB-001 | 688, 832 | not-started | in-progress |
| WEB-002 | 689, 833 | not-started | in-progress |
| WEB-003 | 690, 834 | not-started | in-progress |
| WEB-004 | 691, 835 | not-started | in-progress |
| WEB-005 | 692, 836 | not-started | in-progress |

Note: FEATURES.md repeats most stories in several sections (obligation index, generated mirrors);
only a subset of occurrences per story carry the stale status — validator reports each story once.

## 5. On-disk GREEN spot-checks (evidence behind classifications)
- tasks/ROUTE-001.md IMPLEMENTED + worklog/ROUTE-001.md present → accepted, mirror stale only.
- tasks/TOOL-015.md, OPS-010.md, PROV-014.md, AUTO-007.md, EXT-013.md, SESS-019/020.md, ROUTE-012.md,
  REL-004.md, AUTO-003.md, EXT-003.md, INT-004/008.md: all IMPLEMENTED → accepted, mirrors stale.
- tasks/TOOL-006.md, TOOL-009.md, OPS-001.md, SHARE-001.md, WEB-001.md, INT-009.md, REL-001.md,
  UI-014.md: NOT STARTED (matches stale FEATURES text, contradicts ralph where accepted/in-progress).
- tasks/ROUTE-009.md, ROUTE-010.md, tasks/UI-001..013.md: do not exist.
- tasks/WEB-006/007/008.md: IMPLEMENTED / NOT ACCEPTED + worklogs present.

## 6. Verdict for controller
1. Run FEATURES.md status-mirror sync from ralph.json → clears 69 rows, zero product risk.
2. Decide WEB-006/007/008 (accept to `local-implemented-stale` with commits, or revert cards) → clears 3.
3. Classify PROV-015 + cut ledger entries for the 27 missing non-accepted stories → clears 4 header rows
   (plan-validator unknown-prefix errors for ACP/HEAD/RUN/SDK/SYNC/WSX need the same ledger/policy fix).
4. Deliberately reconcile ownership-gap files where task/worklog evidence appeared (OPS/SHARE/REL/EXT/INT/
   ROUTE) → clears remaining ~46 gap rows. Not worker-auto-fixable by design.
