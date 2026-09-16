# REL-002 FINAL verdict

Rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`. No product edits. Validator hash `79be6ef1e7702f3224b378e2864f588a8eed5db1600fbd6917de8bf4071b9c04` confirmed (`sha256sum tools/check_release_tdd.py`). `git status` clean on `tools/check_release_tdd.py`, `fixtures/release-tdd/`, `ralph.json`. `ralph.json` untouched. Frozen rev `863a0019fc6f0b9779d867792fe00c4a9ab84c87` pinned (`fixtures/release-tdd/frozen.json:15`).

## Matrix (disposable /tmp/opencode/rel7b/ fixture copy, reports /tmp/opencode/dD-rel002-*.json)

| Probe | Fixture | Exit | Result |
|---|---|---|---|
| T01 | full proof | 0 | pass, all four checks pass |
| T01b | full proof rerun | 0 | `cmp` identical DETERMINISTIC |
| T02 | fail-no-red | 2 | `red_proof: fail`, `missing-red-proof` |
| T03 | fail-mutated | 2 | `frozen_intact: fail`, `mutated-frozen-evidence` |
| T04a | fail-no-verifier (worker-only) | 2 | `verifier_rerun: fail`, `self-report-not-evidence` |
| T04b | fail-wrong-rev | 2 | `verifier_rerun: fail`, `verifier-revision-mismatch` |
| T05 | pipeline-only | 2 | all fail, `duplicate-of-existing-owner:AUTO-005` |
| import-only | fail-import-only | 2 | `red_proof: fail`, `red-failed-for-wrong-reason` |
| blocked | fail-blocked | 1 | `blocked-pending-authority`, no report |
| malformed | fail-malformed (green.json `{not valid json`) | 1 | malformed receipt, no report |

Rerun commands (serial, timeout 120 rtk, rev `863a0019fc6f0b9779d867792fe00c4a9ab84c87`):
- `python3 tools/check_release_tdd.py --revision <rev> --manifest /tmp/opencode/rel7b/release-tdd/frozen.json --receipts /tmp/opencode/rel7b/release-tdd/receipts --gates /tmp/opencode/rel7b/release-tdd/gates --out /tmp/opencode/dD-rel002-t01.json` (repeat to `dD-rel002-t01b.json`; `cmp` both)
- Same pattern T02 `fixtures/release-tdd/fail-no-red/...` -> `dD-rel002-t02.json`, T03 `fail-mutated` -> `dD-rel002-t03.json`, T04a `fail-no-verifier` -> `dD-rel002-t04a.json`, T04b `fail-wrong-rev` -> `dD-rel002-t04b.json`, T05 `pipeline-only` -> `dD-rel002-t05.json`, import-only `fail-import-only` -> `dD-rel002-importonly.json`, blocked `fail-blocked` -> `dD-rel002-blocked.json` (absent-as-expected), malformed `fail-malformed` -> `dD-rel002-malformed.json` (absent-as-expected)

## Verdict: Y

Rationale: validator tool complete — all obligation probes behave per spec (T01 exit 0; T02/T03/T04a/T04b/T05 exit 2 with expected reasons; determinism cmp clean) + edge probes per spec (import-only exit 2 wrong-reason, blocked/malformed exit 1 no report). Entry-removed RED recorded in `worklog/REL-002.md`. Verifier re-run reproducible via recorded commands above.
