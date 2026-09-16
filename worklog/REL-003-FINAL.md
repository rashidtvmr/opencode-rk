# REL-003 FINAL verdict (owned file, no product edit)

- Rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`
- Validator hash: `3a46203217557be88135eafa5b9725cba3b1176aaf0c74a9d552b56bc5849b45` (prefix `3a462032` confirmed)
- Scope clean: `git status --short -- tools/ fixtures/ ralph.json validators/` empty; `validators/ralph.json` untouched; `ralph.json` untouched. (Unrelated worktree modifications exist outside owned scope; not mine, not touched.)
- Serial run: every validator call `timeout 120`, serial (no parallel lanes).

## Matrix (exits verified, reports /tmp/opencode/dE-rel003-*.json; copies /tmp/opencode/rel7c/)

| case | fixture | exit | report |
|---|---|---|---|
| T01 | fixtures/release-safety/gates | 0 | dE-rel003-t01.json: passed true, all four pass, partition safety-resource-correctness-release-validator, failing_fixture null |
| T01b | same, rerun | 0 | dE-rel003-t01b.json byte-identical (`cmp` clean) |
| T02 | fail-star-bypass | 2 | dE-rel003-t02.json: mandatory_intact fail, reason mandatory-bypass, fixture mandatory.json#SsrfBlock |
| T03a fs | fail-side-effect-fs | 2 | dE-rel003-t03a.json: denied_no_side_effects fail, reason denied-side-effect, fixture side_effects.json#write:/tmp/rel003-probe-blocked |
| T03b proc | fail-side-effect-proc | 2 | dE-rel003-t03b.json: denied_no_side_effects fail, reason denied-side-effect, fixture side_effects.json#exec:/bin/rel003-probe-blocked |
| T04a oversize | fail-oversize | 2 | dE-rel003-t04a.json: bytes_bounded fail, reason unbounded-retention, fixture resources.json |
| T04b history | fail-history-deletion | 2 | dE-rel003-t04b.json: quotas_enforced fail, reason silent-history-deletion, fixture retention.json |
| T05 tdd-only | tdd-only | 2 | dE-rel003-t05.json: reason unverified-release-state |
| malformed | fail-malformed | 1 | no report written (tool error) |
| blocked | fail-blocked | 1 | no report written (gates not a directory) |

## Canary / secret safety

- `grep -c REL003-CANARY-9f8e7d6c5b4a /tmp/opencode/dE-rel003-t01.json` = 0 (grep exit 1 = no match).
- `grep -r -c` over /tmp/opencode/rel7c/ (9 reports): every file 0.
- Disposable dirs only (/tmp/opencode/dE-rel003-*, /tmp/opencode/rel7c/); no user-DB touch; no writes outside `<out>`.

## RUN-001 bonus

- `cargo test -p opencode-rk-sessions --test runner`: 5/5 pass (run001_t01..t05), exit 0.

## Verdict

**Y (PASS).** Full T01..T05 (+T01b cmp clean, canary 0, malformed/blocked exit 1 no-report) GREEN on rev 248f519, hash 3a462032 unchanged, owned scope clean. RUN-001 runner 5/5 bonus confirmed.
