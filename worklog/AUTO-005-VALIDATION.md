# AUTO-005 Validation (disposable fixtures /tmp/opencode/auto005v/)

Rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b` (matches proposal §1).
Tool: `tools/check_tdd_pipeline.py` sha256 `eea1ac0f21d5d047f9455b6d9b77c5a8f3942ad4ad1ff34efcff9b723e288474` (matches proposal §1).
No edits to `tools/check_tdd_pipeline.py`, `ralph_loop.py`, `lane_gate.py`, `ralph.json`.

## Fixtures
Copied repo fixtures to disposable dir (no repo writes):
- `/tmp/opencode/auto005v/pass` ← `fixtures/tdd-pipeline/pass`
- `/tmp/opencode/auto005v/fail-edited` ← `fixtures/tdd-pipeline/fail-edited`
- `/tmp/opencode/auto005v/fail-self-report` ← `fixtures/tdd-pipeline/fail-self-report`
- `/tmp/opencode/auto005v/fail-wrong-rev` ← `fixtures/tdd-pipeline/fail-wrong-rev`
Test bytes confirmed: PASS `ca32e83d…02b83c6ad`, EDITED `dcefc12d…266699fa6` (match proposal §1).

## Results (exact argv, cwd=repo root, `rtk timeout 120 python3`, `--rev auto005-rev-001`)

| case | manifest | receipts | exit | reason | report sha256 | log |
|------|----------|----------|------|--------|---------------|-----|
| PASS | auto005v/pass/manifest.json | auto005v/pass/receipts | 0 | passed:true, 4/4 pass, reason null | `8050033600cf4f4d46eef171652cf7f828add5cf68074453178780c358f225d5` | /tmp/opencode/v6-auto005-pass.log |
| MUTATED | auto005v/fail-edited/manifest.json | auto005v/fail-edited/receipts | 2 | mutated-frozen-evidence | `0cf09169ee97d52ad8a1a7f6e6dc6ceeb74f58ed5dbc0944d381a19e8ab2b704` | /tmp/opencode/v6-auto005-edited.log |
| SELF-REPORT | auto005v/fail-self-report/manifest.json | auto005v/fail-self-report/receipts | 2 | self-report-not-evidence | `0855482098239a3a14897fd42ff930155c3d82d35af8a8623bb97f99600ff877` | /tmp/opencode/v6-auto005-self.log |
| WRONG-REV | auto005v/fail-wrong-rev/manifest.json | auto005v/fail-wrong-rev/receipts | 2 | verifier-revision-mismatch | `5625c92e2a1dc5904938f51399289780f25828aad91a11c8c4b4f38a02ae685e` | /tmp/opencode/v6-auto005-wrongrev.log |

Reports: `/tmp/opencode/v6-auto005-pass.json`, `/tmp/opencode/v6-auto005-edited.json`, `/tmp/opencode/v6-auto005-self.json`, `/tmp/opencode/v6-auto005-wrongrev.json` (each byte-identical to its `.log`; stdout redirect only, no tee masking).

## Deltas vs proposal §3 (expected, path-embedded)
- Exits + reasons match proposal F01–F04 exactly.
- `failing_fixture` paths point at `/tmp/opencode/auto005v/...` (disposable copy), not `fixtures/tdd-pipeline/...`.
- Report shas differ from proposal `uG-*` logs except PASS (`80500336…` identical — null fixture path). MUTATED/SELF/WRONGREV shas differ solely due to embedded disposable `failing_fixture` path.
- F05 BLOCKED out of scope for this lane (not run).

## Verifier note
Worker text never counted; verdicts from gate exits + report bytes only. Acceptance is verifier decision.
