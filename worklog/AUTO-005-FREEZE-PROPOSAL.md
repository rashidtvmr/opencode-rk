# AUTO-005 Freeze Proposal (controller decision, NOT applied)

Status: proposal only. No edits to `tools/check_tdd_pipeline.py`,
`tools/ralph_loop.py`, `tools/lane_gate.py`, `ralph.json` made by this lane.
Workdir `/home/rashid/projects/opencode-rk`.

## 1. Source pins

- Repo HEAD: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`
- Tool: `tools/check_tdd_pipeline.py`
  sha256 `eea1ac0f21d5d047f9455b6d9b77c5a8f3942ad4ad1ff34efcff9b723e288474`
  (`tools/check_tdd_pipeline.py:31` PARTITION, `:58-61` CLI contract,
  exits 0 pass / 2 assurance-fail / 1 tool-error).
- Existing fixtures (no new files proposed; freeze these bytes as-is):
  - test bytes PASS `ca32e83d968f0bc526d1b276c7f5c080fbaf1311606a3935e87907a02b83c6ad`
    (`fixtures/tdd-pipeline/pass/tests/test_slice.py`, identical in
    fail-self-report / fail-wrong-rev / fail-blocked).
  - test bytes EDITED `dcefc12d38510f2d852dc153da0988ac0449c4f90fb6ab0d57a26af266699fa6`
    (`fixtures/tdd-pipeline/fail-edited/tests/test_slice.py`, 1-byte
    GREEN->GREEM flip).
  - manifest bytes `309ddfa24363fe7042c4c90dd3b5f19d5f1a174679b9a8e9d141e49c4d38c014`
    (identical across all 5 cases; pins `tests/test_slice.py` + partition
    `trusted-red-green-pipeline` + owner AUTO-005 + revision
    `auto005-rev-001` + `commands.{red,green,verifier}` templates).
- Caveat (verifier must note): tool compares receipt `revision` against
  CLI `--rev`, never against `manifest.revision`. Manifest `revision` field
  is informational only. Freeze must pin the CLI `--rev` value
  (`auto005-rev-001`), not rely on the manifest field.

## 2. Frozen command manifest (exact argv, cwd=repo root, `timeout 120`, `rtk` prefix)

F01 PASS:
`rtk timeout 120 python3 tools/check_tdd_pipeline.py --manifest fixtures/tdd-pipeline/pass/manifest.json --rev auto005-rev-001 --receipts fixtures/tdd-pipeline/pass/receipts --out /tmp/opencode/uG-auto005-pass.json`
F02 MUTATED:
`rtk timeout 120 python3 tools/check_tdd_pipeline.py --manifest fixtures/tdd-pipeline/fail-edited/manifest.json --rev auto005-rev-001 --receipts fixtures/tdd-pipeline/fail-edited/receipts --out /tmp/opencode/uG-auto005-edited.json`
F03 SELF-REPORT:
`rtk timeout 120 python3 tools/check_tdd_pipeline.py --manifest fixtures/tdd-pipeline/fail-self-report/manifest.json --rev auto005-rev-001 --receipts fixtures/tdd-pipeline/fail-self-report/receipts --out /tmp/opencode/uG-auto005-self.json`
F04 WRONG-REV:
`rtk timeout 120 python3 tools/check_tdd_pipeline.py --manifest fixtures/tdd-pipeline/fail-wrong-rev/manifest.json --rev auto005-rev-001 --receipts fixtures/tdd-pipeline/fail-wrong-rev/receipts --out /tmp/opencode/uG-auto005-wrongrev.json`
F05 BLOCKED:
`rtk timeout 120 python3 tools/check_tdd_pipeline.py --manifest fixtures/tdd-pipeline/fail-blocked/manifest.json --rev auto005-rev-001 --receipts fixtures/tdd-pipeline/fail-blocked/receipts --out /tmp/opencode/uG-auto005-blocked.json`
F06/F07 suite-byte proofs (manifest `commands` templates, `<case>` substituted):
`rtk timeout 120 python3 -m pytest fixtures/tdd-pipeline/pass/tests -q`
`rtk timeout 120 python3 -m pytest fixtures/tdd-pipeline/fail-edited/tests -q`

## 3. Fixture set and expected results (validated live 2026-09-16)

| cmd | manifest | receipts | expected exit | expected reason |
|-----|----------|----------|---------------|-----------------|
| F01 | pass | red(compiled,exit1,missing-behavior)+green(0)+verifier(independent,0) rev auto005-rev-001 | 0 | passed:true, 4/4 pass |
| F02 | fail-edited (GREEM byte) | same receipts, hash mismatch | 2 | mutated-frozen-evidence, failing_fixture=.../fail-edited/tests/test_slice.py |
| F03 | fail-self-report (worker passes:true, no verifier) | worker.json only + red + green | 2 | self-report-not-evidence |
| F04 | fail-wrong-rev (verifier rev auto005-rev-002) | verifier on wrong rev | 2 | verifier-revision-mismatch |
| F05 | fail-blocked | blocked.json | 1 (tool-error) | stderr blocked-pending-authority, no report written |
| F06 | pass suite pytest | — | 0 | 1 passed |
| F07 | fail-edited suite pytest | — | 1 | 1 failed (AssertionError GREEM) |

Validation logs (this lane): `/tmp/opencode/uG-auto005-pass.log/json`
(8050033600cf4f4d46eef171652cf7f828add5cf68074453178780c358f225d5),
`-edited` (ab9161e8496087bad83314a20e1f9cb42f29302d41c19b096d120d0361d82d80),
`-self` (1ae1cc556eb525a6c81711f1417c13d5f8965c9c14663ab9167a3538f834f8b0),
`-wrongrev` (82a6e1f55bc44b3470606a16ad0bc102287f582489cdd9b57f0e9d6c408b76d4),
`-blocked.log` (86b349f5ada2e3a1d8694253bab13404e92d1daf17450b3ae1325a0bab585892),
`-pytest-pass.log`, `-pytest-edited.log`, `-hashes.log`, `-artifacts-sha.log`.
Exits measured with direct redirect (pipe via tee masks exit with tee's 0).

## 4. Verifier-rerun procedure (independent, fresh read)

1. `git rev-parse HEAD` must equal frozen HEAD above; else reject (wrong-rev).
2. `sha256sum tools/check_tdd_pipeline.py` must equal §1; else reject (tool drift).
3. `sha256sum` all 10 fixture paths in §1; any mismatch vs frozen hashes rejects.
4. Rerun F01–F05 exactly as §2 on exact frozen rev; compare exits + `reason` +
   `failing_fixture` + report sha against §3. Any delta rejects.
5. Rerun F06/F07; pytest exits must be 0 / 1.
6. Worker text / `passes:true` never counts; only gate exit + report bytes.
7. Append receipt with exact commands + hashes; acceptance is verifier decision.

## 5. Wiring diff sketch (PROPOSAL TEXT ONLY — do not apply)

Rationale: `tools/ralph_loop.py:58-61` verification list today is
`validate_repository.py` + `lane_gate.py --run`; `tools/lane_gate.py:31-44`
LANES cover storage modules only. Pipeline tool unwired, so controller can
accept without RED/GREEN enforcement.

Sketch A — `tools/ralph_loop.py:58-61` (append mandatory entry):
```python
MANDATORY_VERIFICATION_COMMANDS = [
    [sys.executable, "tools/validate_repository.py"],
    [sys.executable, "tools/lane_gate.py", "--run"],
    # PROPOSED (controller-owned; manifest path frozen per §2, rev pinned):
    [sys.executable, "tools/check_tdd_pipeline.py",
     "--manifest", "fixtures/tdd-pipeline/pass/manifest.json",
     "--rev", "auto005-rev-001",
     "--receipts", "fixtures/tdd-pipeline/pass/receipts",
     "--out", "state/tdd-pipeline-report.json"],
]
```
Note: `load_settings()` merges custom `verificationCommands` after mandatory,
so this entry cannot be dropped by operator config. Receipts dir must be
controller-written; workers must not gain write access (docs/SECURITY.md:54-60).

Sketch B — `tools/lane_gate.py:31-44` (append declarative lane, no test-target run):
```python
{"id": "pipeline:AUTO-005", "kind": "pipeline", "owner": "controller",
 "tool": "tools/check_tdd_pipeline.py",
 "manifest": "fixtures/tdd-pipeline/pass/manifest.json",
 "rev": "auto005-rev-001", "receipts": "fixtures/tdd-pipeline/pass/receipts"},
```
with a `check_pipeline()` that runs the frozen argv via subprocess, requires
exit 0 + `passed:true` in the report, and maps exit 2 -> FAIL(detail=reason),
exit 1 -> BLOCKED. Gate must re-read tool + fixture hashes (§1) before run.

Controller decisions left open: frozen manifest path (repo fixture vs
`state/` ledger copy), per-task vs global receipts dir, blocked-exit mapping
in `run_verification` (nonzero = FAIL transcript).
