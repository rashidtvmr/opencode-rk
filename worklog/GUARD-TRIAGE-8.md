# GUARD-TRIAGE-8 — validator triage (verification lane, read-only)

Date: 2026-09-16. Workdir: /home/rashid/projects/opencode-rk.
ralph.json / FEATURES.md / controller state NOT edited. Only this file written.

## 1. Command results

| Command | Exit | Count |
|---|---|---|
| `python3 tools/validate_repository.py` (log `/tmp/opencode/s13-validate.log`, copy `/tmp/opencode/s13-validate2.log`) | FAIL, backlog exhaustion exit=1 | **122 error(s)** |
| `python3 tools/validate_plan.py` (log `/tmp/opencode/s13-plan.log`) | 1 | **133 error(s)** = 122 + 11 `Unknown task prefix` (ACP-001/002, HEAD-001/002, RUN-001, SDK-001/002, SYNC-001/002, WSX-001/002) |
| `git diff --check` | 0 | clean, no whitespace errors |
| `git status --short` | — | **337 lines: M=204(+1 vs w10), ??=133(+6 vs w10)** (log `/tmp/opencode/s13-status2.log`; rtk-filtered wc undercounts — bare `git status` is authority) |

## 2. Delta vs GUARD-TRIAGE-7 baseline (w10: M=203, ??=127, 329 lines)

- Validator errors: 122 → 122. **Error sets byte-identical** (`diff` of sorted `^  - ` lines: IDENTICAL, order-only WEB-006/007/008 rotation in one rtk-filtered view, sorted content same).
- Composition unchanged: 69 FEATURES.md stale-mirror rows + 53 ledger/gap rows = 122.
- Worktree delta: +7 paths, −0 paths since w10 snapshot:
  - `M crates/sessions/src/share_merge.rs` (modified)
  - `?? crates/sessions/tests/zz_probe_debug_redact.rs` (new test probe)
  - `?? worklog/AUDIT-NONACCEPTED.md`
  - `?? worklog/GUARD-TRIAGE-7.md`
  - `?? worklog/INTEGRATION-6.md`, `?? worklog/INTEGRATION-7.md`
  - `?? worklog/WSX-SDK-HEAD-GATE.md`
- `git diff --stat`: 204 files (207 via bare `git diff --name-only`), +2072/−1231.

## 3. Classification

- **Fixed: none.** Zero validator rows cleared. No wiring landed in controller-owned ledger/FEATURES state.
- **New drift: worklog/test-probe only.** All 7 new paths are worklogs or a `zz_` debug test; none touch validator inputs (`ralph.json`, `FEATURES.md`, task cards, gap ledgers). `share_merge.rs` modification unvalidated by guard (product code, not ledger).
- **Pre-existing (carried from TRIAGE-7, unchanged):**
  - A. 69 stale-FEATURES mirror rows — controller `--sync-features` owns fix.
  - B. Wiring-landed-but-ungated (ROUTE-001, TOOL-015/006/009, OPS-010, PROV-014, AUTO-007, EXT-013, SESS-019/020, ROUTE-012, REL-004, UI-001..018, WEB-006/007/008).
  - C. Genuinely missing: PROV-015 unclassified + 27 non-accepted stories absent from exhaustion ledger; ROUTE-009/010 no task cards; ownership-gap deliberate-review gates (OPS/SHARE/REL/EXT/INT/ROUTE).

## 4. Verdict for controller

Same as TRIAGE-7 §6, still open: (1) FEATURES sync → clears 69; (2) WEB-006/007/008 accept/revert → clears 3; (3) classify PROV-015 + ledger entries for 27 missing → clears 4 header rows (+11 plan unknown-prefix); (4) deliberate ownership-gap reconciliation → clears ~46. No worker-auto-fixable item appeared this round.
