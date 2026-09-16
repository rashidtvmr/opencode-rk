# REL-001 FINAL verdict

Rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`. No product edits. Validator hash `ab9b350e17fc727e4d35d18dd30c7d468a7307ce2e0fa29fddf1a28a15554bda` confirmed (`sha256sum tools/check_release_accounting.py`). `git status` clean on `tools/check_release_accounting.py`, `fixtures/release-accounting/`, `ralph.json`. `ralph.json` untouched.

## Matrix (disposable /tmp/opencode/rel7a/ fixtures, reports /tmp/opencode/dC-rel001-*.json)

| Probe | Fixture | Exit | Result |
|---|---|---|---|
| T01 | complete | 0 | pass, empty missing/misclassified |
| T01b | complete rerun | 0 | `cmp` identical DETERMINISTIC |
| T02 | incomplete | 2 | missing `[REL-001]` (+`duplicate-of-existing-owner` detail) |
| T03 | misclassified | 2 | misclassified `[DISC-001,DISC-010,REL-001,REQ-003]`, reasons `inference-without-evidence` |
| T04 | not-accepted | 0 | accounting pass, zero `"accepted": true` bytes, controller hash `ed58eaf0...` unchanged before/after |
| T05 | pins-only | 2 | reason `duplicate-of-existing-owner` |

Rerun commands (serial, timeout 120 rtk):
- `python3 tools/check_release_accounting.py --snapshot /tmp/opencode/rel7a/complete --out /tmp/opencode/dC-rel001-t01.json` (repeat to `dC-rel001-t01b.json`; `cmp` both)
- Same pattern T02 `rel7a/incomplete` -> `dC-rel001-t02.json`, T03 `rel7a/misclassified` -> `dC-rel001-t03.json`, T04 `rel7a/not-accepted` -> `dC-rel001-t04.json`, T05 `rel7a/pins-only` -> `dC-rel001-t05.json`

## Verdict: Y

Rationale: validator tool complete — all 5 obligation probes behave per spec + determinism + lane_gate receipt bF (8 module lanes PASS, no REL lanes defined, JSON `/tmp/opencode/bF-lanegate.json`). No product-behavior RED applicable by nature; tool IS the test harness (entrypoint-removed RED recorded in `worklog/REL-001.md`). Verifier re-run reproducible via recorded commands above.
