# AUTO-005 Validation Round 2 (independent verifier rerun, disposable fixtures)

Status: verification only. No edits to `tools/check_tdd_pipeline.py`, `tools/ralph_loop.py`, `tools/lane_gate.py`, `ralph.json`.
Workdir `/home/rashid/projects/opencode-rk`. Rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
Source: `worklog/AUTO-005-FREEZE-PROPOSAL.md` §§1–4.

## Pins

- Tool sha256: `eea1ac0f21d5d047f9455b6d9b77c5a8f3942ad4ad1ff34efcff9b723e288474` (`tools/check_tdd_pipeline.py`, matches proposal §1).
- Pinned CLI rev: `auto005-rev-001` (proposal §1 caveat: tool compares receipt `revision` vs CLI `--rev`, manifest field informational only).
- Fixtures: copied repo `fixtures/tdd-pipeline/` → disposable `/tmp/opencode/auto005x/tdd-pipeline/` (`cp -a`, no repo fixture edits). Manifest base-relative paths resolve identically under copy.

## Commands (exact argv, cwd=repo root, `timeout 120`, stdlib python only, no cargo)

- PASS: `timeout 120 python3 tools/check_tdd_pipeline.py --manifest /tmp/opencode/auto005x/tdd-pipeline/pass/manifest.json --rev auto005-rev-001 --receipts /tmp/opencode/auto005x/tdd-pipeline/pass/receipts --out /tmp/opencode/aE-auto005-pass.json`
- MUTATED: `timeout 120 python3 tools/check_tdd_pipeline.py --manifest /tmp/opencode/auto005x/tdd-pipeline/fail-edited/manifest.json --rev auto005-rev-001 --receipts /tmp/opencode/auto005x/tdd-pipeline/fail-edited/receipts --out /tmp/opencode/aE-auto005-edited.json`
- SELF-REPORT: `timeout 120 python3 tools/check_tdd_pipeline.py --manifest /tmp/opencode/auto005x/tdd-pipeline/fail-self-report/manifest.json --rev auto005-rev-001 --receipts /tmp/opencode/auto005x/tdd-pipeline/fail-self-report/receipts --out /tmp/opencode/aE-auto005-self.json`
- WRONG-REV: `timeout 120 python3 tools/check_tdd_pipeline.py --manifest /tmp/opencode/auto005x/tdd-pipeline/fail-wrong-rev/manifest.json --rev auto005-rev-001 --receipts /tmp/opencode/auto005x/tdd-pipeline/fail-wrong-rev/receipts --out /tmp/opencode/aE-auto005-wrongrev.json`

## Results (exit captured via `echo EXIT:$?`, direct redirect — no tee masking)

| case | exit | report | report sha256 |
|------|------|--------|---------------|
| PASS | 0 | `passed:true`, 4/4 pass | `8050033600cf4f4d46eef171652cf7f828add5cf68074453178780c358f225d5` (= proposal F01 report hash) |
| MUTATED | 2 | `reason:mutated-frozen-evidence`, `failing_fixture:.../fail-edited/tests/test_slice.py` | `f996ba5f9555eb55130ad1a3aea4b6375e9a9657078aa3e9cdfc21977561426d` (path prefix differs — disposable copy; reason/checks match proposal F02) |
| SELF-REPORT | 2 | `reason:self-report-not-evidence` | `deccdd027878c954e7b51f6a2f431df5a7476e7f30a8a8ee60b3b7274f7f956e` (path prefix differs; reason/checks match proposal F03) |
| WRONG-REV | 2 | `reason:verifier-revision-mismatch` | `acbed663abade7407ac1e3096d3dc6ca7a3fbcf672a48b5c52e0f7c1e6962fcf` (path prefix differs; reason/checks match proposal F04) |

PASS report bytes identical to proposal F01 hash. Fail-case report shas differ only in `failing_fixture` absolute-path prefix (proposal ran on `fixtures/...`-rooted or `uG` paths; this round on `/tmp/opencode/auto005x/...`); `reason` + `checks` + exit match exactly.

## Guard checks

- `git diff --stat -- tools/check_tdd_pipeline.py tools/ralph_loop.py tools/lane_gate.py ralph.json` → empty (no edits to frozen tools).
- Bounds: python stdlib only, no cargo, single `timeout 120` run at a time.

## Verdict

4/4 exits match proposal §3 (0 / 2 / 2 / 2). Reasons match. Tool sha matches. Freeze proposal confirmed reproducible on disposable fixtures.
