# GUARD-TRIAGE-9 — validator triage (verification lane, read-only)

Date: 2026-09-16. Workdir: /home/rashid/projects/opencode-rk.
ralph.json / FEATURES.md / controller state NOT edited. Only this file written.

## 1. Command results

| Command | Exit | Count |
|---|---|---|
| `python3 tools/validate_repository.py` (log `/tmp/opencode/t5-validate-raw.log`, rtk copy `/tmp/opencode/t5-validate.log`) | 1, FAIL backlog exhaustion | **122 error(s)** (`grep -c '^  - '` = 122) |
| `python3 tools/validate_plan.py` (log `/tmp/opencode/t5-plan-raw.log`) | 1 | **133 error(s)** = 122 + 11 `Unknown task prefix` (ACP-001/002, HEAD-001/002, RUN-001, SDK-001/002, SYNC-001/002, WSX-001/002) |
| `git diff --check` | 0 | clean, no whitespace errors |
| `git status --porcelain` (log `/tmp/opencode/t5-status-raw.log`) | — | **M=205, ??=161, total 366** (bare git is authority; rtk-prefixed output strips leading spaces and undercounts) |

Note: `timeout 120 rtk` prefix used for validator runs per task; bare `git`/`grep` used for
counts because the rtk filter mangles leading-whitespace porcelain output and `pipestatus`.

## 2. Delta vs GUARD-TRIAGE-8 baseline (s13-status2.log on disk: M=204, ??=131)

- Validator errors: 122 → 122. **Error sets byte-identical** (`diff` of sorted `^  - ` lines vs s13-validate2.log: IDENTICAL).
- Plan errors: 133, same composition (122 + 11 unknown-prefix).
- Worktree delta: **+31 paths, −0 paths**:
  - `M crates/tools/src/plugin_lifecycle.rs` (one new modified product file)
  - 30 new untracked worklogs, all `worklog/`: CLEANUP-TRIAGE, GUARD-TRIAGE-8, INTEGRATION-7,
    PROV-015, PROV-016, PROV-017, PROV-018, PROV-019, PROV-020, PROV-021, PROV-022, PROV-023,
    PROV-024, RED-VALIDITY-INT, RED-VALIDITY-WEB-PROV, RUN-001, SYNC-001, SYNC-002,
    TOOL-016, TOOL-017, TOOL-018, TOOL-019, TOOL-020, UI-019,
    WEB-013, WEB-014, WEB-015, WEB-016, WEB-017, WEB-WSX-HEAD-VERIFY
- `git diff --stat`: 206 files, +2082/−1239.
- Controller inputs untouched: `git status --porcelain -- ralph.json FEATURES.md PLAN.md tools/validate_repository.py tools/validate_plan.py` empty.
- TRIAGE-8 text claims ??=133 but its on-disk s13-status2.log holds 131 `??` rows
  (204+131=335 lines in a 334-newline file — missing trailing newline); file is baseline here.

## 3. Classification

- **Fixed: none.** Zero validator rows cleared. No wiring landed in controller-owned ledger/FEATURES state.
- **New drift: worklog-only + 1 product file.** All 30 new untracked paths are worklogs; none touch
  validator inputs. `crates/tools/src/plugin_lifecycle.rs` modification is product code, unvalidated by guard.
- **Pre-existing (carried, unchanged):**
  - A. 69 stale-FEATURES mirror rows — controller `--sync-features` owns fix.
  - B. Wiring-landed-but-ungated (ROUTE-001, TOOL-015/006/009, OPS-010, PROV-014, AUTO-007, EXT-013, SESS-019/020, ROUTE-012, REL-004, UI-001..018, WEB-006/007/008).
  - C. Genuinely missing: PROV-015 unclassified + 27 non-accepted stories absent from exhaustion ledger; ROUTE-009/010 no task cards; ownership-gap deliberate-review gates (OPS/SHARE/REL/EXT/INT/ROUTE).

## 4. Verdict for controller

Same as TRIAGE-8 §4, still open: (1) FEATURES sync → clears 69; (2) WEB-006/007/008 accept/revert → clears 3;
(3) classify PROV-015 + ledger entries for 27 missing → clears 4 header rows (+11 plan unknown-prefix);
(4) deliberate ownership-gap reconciliation → clears ~46. No worker-auto-fixable item appeared this round.
