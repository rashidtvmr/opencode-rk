# PHASE1-RELEASE-EVIDENCE-VERIFY

## Claim
- Task: PHASE1-RELEASE-EVIDENCE-VERIFY
- Session: ses_f2dfeb1c1ffeFTnwzLfbXF6te5
- Owned writes: this scratchpad + own ledger row. No candidate/product/test/controller edits.
- Branch: plan/release-evidence. Candidate under review: 73be580 PHASE1-RELEASE-EVIDENCE-MAP.

## Verdict
**ACCEPT WITH CORRECTIONS** (candidate may NOT be consumed as-is; multiple material
false/stale statements; corrected facts below).

## Evidence (read-only commands, all run in this worktree)

### Validator scripts and fixtures EXIST (candidate says they do not)
- `git ls-tree -r --name-only HEAD -- tools/ | grep release`:
  `tools/check_release_accounting.py`, `tools/check_release_tdd.py`, `tools/check_release_safety.py`.
- `wc -l`: 242 / 295 / 343 lines. Real stdlib validators, not stubs.
- `git log --oneline -1 -- tools/check_release_safety.py`: `248f519 feat: add native harness feature batches`.
- `git merge-base --is-ancestor 248f519 HEAD` = YES; origin/main = YES; origin/lane/PHASE1-product-spine-20260923 = YES.
  So validators are landed on main as well as this branch.
- Fixtures exist: `fixtures/release-accounting/{complete,incomplete,not-accepted,pins-only,misclassified}`,
  `fixtures/release-tdd/{receipts,gates,tests,fail-*}` with `receipts/verifier.json`,
  `fixtures/release-safety/{gates,caps.json,quotas.json,fail-*}`.
- Validators also present at the receipt revision: `git ls-tree --name-only 5d66683 tools/` lists all three.
- Test exists: `tests/bootstrap/test_rel002_release_tdd.py` (23726 bytes).

### Corrected REL ledger states (candidate said all NOT STARTED/missing)
`tasks/completion/claims.json` claims:
- REL-001: **ABSENT** from ledger (no row at all). Not "NOT STARTED" row; no claim ever recorded.
  Work existed: `worklog/REL-001.md`, `worklog/REL-001-FINAL.md` (verdict Y, rev 248f519,
  validator sha256 `ab9b350e...`). `ralph.json` says REL-001 accepted.
- REL-002: **blocked**, session ses_f32d2ba72ffeZ7tOBYK7YE5rto, scratchpad
  `worklog/REL-002-STRONG-RED.md`. Blocker is the protected `.github/workflows/ci.yml` caller,
  NOT a missing validator. `origin/lane/REL-002-phase1` tip aad73f0 exists.
  `worklog/REL-002-FINAL.md` = Y at 248f519.
- REL-003: **completed**, session ses_f387af899ffeojhREiG4Rewoar, scratchpad `worklog/REL-003.md`.
  `worklog/REL-003-FINAL.md` = Y, validator sha256 `3a462032...`.
- Task-card text "Status: NOT STARTED" is card-authorship metadata, not ledger state.
  Candidate conflated card text with ledger rows.

### Corrected revision-receipt (5d66683) state
- `git cat-file -t 5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b` = commit, subject
  "APP-010: record integration landing and post-push GREEN evidence".
- `git merge-base --is-ancestor 5d66683 origin/lane/PHASE1-product-spine-20260923` = **YES**
  (it IS on that remote branch; `git branch -r --contains 5d66683` lists
  origin/docs/structured-subagent-prompts and origin/lane/PHASE1-product-spine-20260923).
- `git merge-base --is-ancestor 5d66683 origin/main` = **NO**. Not on main.
- Candidate's "on local branch, not yet pushed to origin" is wrong on the push half:
  it is pushed to the product-spine lane branch, unmerged to main.
- Candidate's own HEAD (`plan/release-evidence`) does NOT contain 5d66683
  (`git rev-list HEAD | grep -c 5d66683` = 0); merge-base with product-spine is 1f4a9e6.
- Packaged binding distinction: `crates/cli/build.rs` (compile-time receipt, c4325e4) exists at
  5d66683/c4325e4 but is **absent at HEAD of this branch**. c4325e4 and 5d66683 are both
  NOT on origin/main. INSTALLED-DEFAULT-CONTRACT-INTEGRATION note (claims.json) states
  `OC2_E2E_REVISION=$(git rev-parse HEAD)` GREEN 5/5, explicitly env-var receipt substitute.
  So: revision receipt is landed on the lane branch, not packaged on main, not main-bound.

### Corrected convergence count
- Candidate: "86 off-plan completed tasks". Actual `python3 tools/convergence_gate.py`:
  `total=88`, with 84 `off-plan` lines and 4 "admits" lines
  (AUD-017, AUD-020, INSTALLED-DEFAULT-CONTRACT-INTEGRATION, PHASE1-RELEASE-EVIDENCE-MAP).
  Off-plan completed = **84**, not 86. Exit=1 (CONVERGENCE BLOCKED).
- Note: the gate's own row for PHASE1-RELEASE-EVIDENCE-MAP fires because the map's
  completedNote contains "missing". Self-referential; not release evidence either way.

### validate_repository
- `python3 tools/validate_repository.py` exit=1, `FAIL backlog exhaustion exit=1`,
  pre-existing. Candidate correct. (51 detail lines emitted; DISC-003 "80 findings" comes
  from DISC-003 worklog, not this output.)

### Corrected signing/notarization citation
- Candidate: "grep found references only in docs/research/ and docs/STORAGE.md".
  Actual: `grep -rli 'codesign|notariz' docs/` = **no matches**.
  `grep -rn -i 'codesign|notariz|signing' .github/workflows/` = none.
  Only hits for "sign" are unrelated ("signed Unix time", "signed actions/checkout",
  "wake signal"). So there is no signing reference in STORAGE.md or REQ-042.
  Conclusion "no release signing implementation" holds; the cited paths are false.

### Corrected "verifier reruns missing"
- Candidate: "No independent verifier rerun receipts found at any revision on this branch".
  Overbroad. Present: `worklog/REL-001-FINAL.md`, `worklog/REL-002-FINAL.md`,
  `worklog/REL-003-FINAL.md` (final-verdict rerun matrices at rev 248f519), and
  `fixtures/release-tdd/receipts/verifier.json`. These are worker-authored FINAL verdicts,
  not controller-signed independent receipts, so the substantive point (no controller-issued
  verifier receipt bound to 5d66683) stands, but the absolute claim is false.

## Corrected classification table
| Item | Candidate | Corrected |
|---|---|---|
| REL-001 validator | missing, script does not exist | script exists (242 lines, 248f519, on main); ledger row ABSENT |
| REL-002 validator | missing | exists (295 lines); ledger blocked on CI caller |
| REL-003 validator | missing | exists (343 lines); ledger completed |
| Fixtures | none implied | release-accounting/tdd/safety all present on main |
| Receipt 5d66683 | local-only, not pushed | pushed on origin/lane/PHASE1-product-spine-20260923, not on main |
| Packaged binding | "requires env var" | build.rs only on lane branch; absent from this branch/main |
| Convergence | 86 off-plan | total=88; 84 off-plan + 4 admits; exit 1 |
| validate_repository | FAIL backlog | FAIL backlog (correct) |
| Signing refs | docs/research + STORAGE.md | no codesign/notariz matches anywhere |
| Verifier reruns | none at any revision | REL-00x-FINAL.md + fixture verifier.json exist; no controller receipt |

## Consumability
Candidate map may be consumed ONLY with these corrections; as written it materially
understates landed release-validator tooling and misstates the receipt branch state.
No edits made to the candidate.

## Verification commands
```
git ls-tree -r --name-only HEAD -- tools/ | grep release
git merge-base --is-ancestor 248f519 HEAD/origin/main/origin/lane/PHASE1-product-spine-20260923
git cat-file -t 5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b
git merge-base --is-ancestor 5d66683 origin/lane/PHASE1-product-spine-20260923   # YES
git merge-base --is-ancestor 5d66683 origin/main                                  # NO
python3 tools/convergence_gate.py      # total=88, exit 1
python3 tools/validate_repository.py   # FAIL backlog exhaustion, exit 1
grep -rli -E 'codesign|notariz' docs/  # no matches
```

## Remaining unknowns
- Whether a controller-issued, revision-bound verifier receipt for 5d66683 will be produced.
- Whether product-spine lane (with build.rs receipt) is intended to merge to main before release.
