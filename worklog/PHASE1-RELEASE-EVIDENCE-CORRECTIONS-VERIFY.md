# PHASE1-RELEASE-EVIDENCE-CORRECTIONS-VERIFY

## Claim

- Task: PHASE1-RELEASE-EVIDENCE-CORRECTIONS-VERIFY
- Session: ses_f2d52634effenzrjL3XPqs1xLo
- Branch: plan/release-evidence
- Owned file: worklog/PHASE1-RELEASE-EVIDENCE-CORRECTIONS-VERIFY.md
- Permitted: this file + own claims.json row only
- Role: independent release-evidence wording verifier. I did not author 0e16363.
- No product test applies (documentation verification); no Cargo/heavy process run.

## Verdict

**REJECT caveat 2 (partial correction); PASS caveat 1.**

Commit `0e16363c5fb5a423b033fa4e4a57abb9f70394d6` resolves the convergence-count
caveat fully. It fixes the `crates/cli/build.rs` wording in ONE of TWO locations;
the exact ambiguous phrase flagged by verifier `42889fc` survives verbatim in the
Release Evidence Ledger section. No release acceptance is conferred by this lane.

## Evidence under review

- Subject/product-spine revision: `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b`
- Evidence-map candidate at prior verdict: `68b837c0b8dd7e5a46f54efc127e96e3cdfc3bfe`
- Prior verifier: `42889fc` (`worklog/PHASE1-RELEASE-EVIDENCE-FINAL-VERIFY.md`)
- Correction under review: `0e16363c5fb5a423b033fa4e4a57abb9f70394d6`
- Correction scratchpad: `worklog/PHASE1-RELEASE-EVIDENCE-FINAL-CORRECTION.md`

## Two-finding matrix

| # | Verifier 42889fc caveat | Required outcome | Observed at 0e16363 | Verdict |
|---|---|---|---|---|
| 1 | Convergence count 88/84 must be historical/non-normative; current truth only CONVERGENCE BLOCKED | Count anchored to named revision, not a criterion | Both convergence blocks now label 88/84 as "historical, non-normative observation bound to commit 73be580"; drift recorded (90/86 at 68b837c); normative state stated as CONVERGENCE BLOCKED (exit=1) | **PASS** |
| 2 | build.rs wording must distinguish tree-absence at evidence-map revision from presence on product-spine | Exact revision identities, no ambiguous "HEAD" phrasing | Fixed at map line 58; **NOT fixed at map line 119**, which still reads "absent at HEAD of candidate branch" | **FAIL (partial)** |

## Finding 1 detail: convergence count (PASS)

- Map line 43: `**Convergence gate (anchored historical, at candidate commit 73be580):**`.
- Map line 46: "This 88/84 figure is a **historical, non-normative observation bound to commit 73be580**, NOT a release criterion and NOT a current reading. Final verifier 42889fc observed live drift to `total=90`, 86 `off-plan`, 4 `admits` at 68b837c; the count continues to drift... the normative state is simply CONVERGENCE BLOCKED (exit=1)."
- Map line 129: "(historical, anchored to commit 73be580; non-normative - live count drifts with ledger churn, e.g. 90/86 at 68b837c)".
- Map line 130: "count is observational history, not a release criterion".
- Live re-run at current HEAD: `CONVERGENCE BLOCKED`, `total=93`, exit=1. Further drift from the 90 the map records confirms the count is non-normative; the map's characterization is accurate and now criteria-free.
- Exact command:
  - `python3 tools/convergence_gate.py` => `CONVERGENCE BLOCKED`, `total=93`, exit=1.
- The count is anchored to a named revision (73be580) and explicitly excluded as a criterion. No unanchored count remains. PASS.

## Finding 2 detail: build.rs tree wording (FAIL)

Verifier `42889fc` Recheck 3 flagged map line 118 (at 68b837c):
`- Source-built receipt: crates/cli/build.rs exists at 5d66683 (compile-time
GIT_COMMIT receipt), absent at HEAD of candidate branch`.

At 0e16363 that same line is line 119 and is **unchanged**:
```
$ git show 0e16363:worklog/PHASE1-RELEASE-EVIDENCE-MAP.md | sed -n '119p'
- Source-built receipt: `crates/cli/build.rs` exists at 5d66683 (compile-time GIT_COMMIT receipt), absent at HEAD of candidate branch
```

The correction instead added a new, correctly-anchored sentence at line 58 only:
```
- `crates/cli/build.rs` exists at product-spine 5d66683 (commit-time GIT_COMMIT receipt); tree-absent at the evidence-map revision 68b837c (`git ls-tree 68b837c -- crates/cli/build.rs` = empty).
```
So the Source Evidence section is correct, but the Release Evidence Ledger section
retains the exact "absent at HEAD of candidate branch" phrase the verifier asked to
anchor. The phrase conflates tree-absence with worktree/HEAD ambiguity, which was the
whole caveat. The correction is one of two occurrences; the flagged occurrence remains.

Git-tree proof (exact identities):
- `git ls-tree 5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b -- crates/cli/build.rs` => `100644 blob f880e89704d7a7be658d5d65e3461223a202b925	crates/cli/build.rs` (present).
- `git ls-tree 68b837c -- crates/cli/build.rs` => empty, exit 0 (tree-absent).
- `git ls-tree 0e16363 -- crates/cli/build.rs` => empty, exit 0 (tree-absent at this revision too).
The substance (no build.rs receipt on the evidence branch) holds; the ledger line's
phrasing does not meet the "exact revision identities" invariant.

## Scope integrity: correction diff limited to two caveats + lane evidence

`git diff --name-status 0e16363^ 0e16363`:
```
M	tasks/completion/claims.json
A	worklog/PHASE1-RELEASE-EVIDENCE-FINAL-CORRECTION.md
M	worklog/PHASE1-RELEASE-EVIDENCE-MAP.md
```
`git diff --stat` = 3 files, +54/-4. The MAP.md diff is two hunks only: the
convergence-count block and the build.rs sentence. No product/Cargo/validator/
workflow/controller files touched. Confirmed via `git diff 0e16363^ 0e16363 -- worklog/PHASE1-RELEASE-EVIDENCE-MAP.md`.

## Classification / blocker regression check (no regression)

`git diff 0e16363^ 0e16363 -- worklog/PHASE1-RELEASE-EVIDENCE-MAP.md` touches no
`Classification:` line. Classifications remain identical to 68b837c:
- line 80 REL-001 **source-built**
- line 95 REL-002 **local-only**
- line 107 REL-003 **source-built**
- line 121 Revision Receipt **source-built** (packaged binding open)
- line 127 Convergence **stale/invalid**
- line 138 validate_repository **stale/invalid**
- line 147 Signing **external unavailable**
- line 158 Final Verifier Reruns **local-only**
- line 172 Platform-Specific **missing**

Open blockers preserved, unchanged by this commit:
- unsigned-only scope (no codesign/notarization receipt)
- missing packaged/installed proof on every platform
- no controller-issued revision-bound receipt for 5d66683
- `validate_repository` FAIL backlog exhaustion exit=1 (DISC-003, 80 findings unresolved)

The correction does not imply release acceptance. Map states release remains blocked;
correction scratchpad states "No product/test/validator/workflow/controller edits".
The ledger row for the correction itself says "Pending independent verification of the
two corrections" -- consistent with this lane, and not an acceptance claim.

## Validation commands and results

```sh
rtk git diff --check                                    # clean, exit 0
rtk git diff --name-status 0e16363^ 0e16363             # 3 files: ledger, lane scratchpad, map
rtk git diff 0e16363^ 0e16363 -- worklog/PHASE1-RELEASE-EVIDENCE-MAP.md  # 2 hunks only
rtk git ls-tree 68b837c -- crates/cli/build.rs          # empty, exit 0 (tree-absent)
rtk git ls-tree 5d66683 -- crates/cli/build.rs          # blob f880e89..., present
rtk git ls-tree 0e16363 -- crates/cli/build.rs          # empty, exit 0 (tree-absent)
rtk git show 0e16363:worklog/.../MAP.md | sed -n '119p' # ambiguous phrase survives
rtk git grep -n "absent at HEAD" 0e16363 -- worklog/.../MAP.md  # line 119 only
rtk python3 tools/convergence_gate.py                   # CONVERGENCE BLOCKED, total=93, exit 1
rtk python3 tools/validate_repository.py                # FAIL backlog exhaustion, exit 1
```

- Frozen-test status/hash: none — documentation verification; no product test applies.
- Contract validation: two-caveat matrix + Git-tree/count checks, reproduced above.
- Resource observation: all commands bounded text/Git/Python; no Cargo; elapsed under
  five minutes wall clock; no long-lived process.

## Expected vs actual state

- Expected RED: verifier 42889fc has two residual caveats. Confirmed in that worklog.
- Expected GREEN: evidence-map-specific ACCEPT only; convergence command stays blocked
  and its count is observational. **Not met for caveat 2**: the correction is
  incomplete, so the evidence-map-specific ACCEPT cannot be issued for the build.rs
  wording. Caveat 1 is GREEN.

## Remaining unknowns / blockers (unchanged)

- No controller-issued revision-bound verifier receipt for 5d66683 (or successor).
- Product-spine merge-to-main intent; packaged receipt injection.
- DISC-003 reconciliation unblock timeline; validate_repository FAIL backlog exhaustion.
- Convergence gate remains CONVERGENCE BLOCKED (exit=1); count drifts (observational).

## Repair note (not performed)

This lane does not repair. The bounded next step for the correction owner is to
replace line 119's "absent at HEAD of candidate branch" with the revision-anchored
wording already used at line 58 (present at 5d66683; tree-absent at the evidence-map
revision). That is the sole residual defect; everything else passes.