# POLICY-BRANCH-UPSTREAM-CORRECTION

Task type: integration. Role: git branch-tracking integration worker.
Session: ses_f2db6c148ffe1v6kFLsyQ5zSdY.
Route: 9router-th-dsv41-flash-free (permitted by allowlist).

## Claim
- Claimed via `tools/completion_claims.py` before touching any file.
- Row: `in-progress`, scratchpad `worklog/POLICY-BRANCH-UPSTREAM-CORRECTION.md`.

## Source evidence (authoritative: actual git config/status/refs)
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/structured-subagent-prompts`.
- `git-branch --show-current` = `docs/structured-subagent-prompts`.
- HEAD = `f2c27569149e04b141efeeeb456ce4639f28d077`.
- REMOTE target ref `refs/remotes/origin/docs/structured-subagent-prompts` = `f2c2756` (same SHA; `git ls-remote origin docs/structured-subagent-prompts` = `f2c2756...`).
- BEFORE: `branch.docs/structured-subagent-prompts.remote=origin`, `.merge=refs/heads/lane/PHASE1-product-spine-20260923`; `@{upstream}` = `origin/lane/PHASE1-product-spine-20260923` (WRONG).
- BEFORE `git push --dry-run`: fatal exit 128, "The upstream branch of your current branch does not match the name of your current branch" — would have targeted `HEAD:lane/PHASE1-product-spine-20260923`.
- Divergence vs target: `git rev-list --left-right --count origin/docs/structured-subagent-prompts...HEAD` = `0 0` (remote contains HEAD; no divergence).

## Scenario / boundary
- Prior verifier report of upstream mismatch is the RED operational evidence; reproduced exactly.
- Branch is the policy worktree branch, distinct from `phase1-integration` (lane/POLICY-STRUCTURED-DELEGATION-WORKER-CORRECTION) and from `phase1-product-spine`. No other worktree's config touched.
- Non-goals honored: no branch switch/rebase/merge, no source/policy edits, no stash, no remote deletion, no force-push.

## Action
- `git branch --set-upstream-to=origin/docs/structured-subagent-prompts docs/structured-subagent-prompts` -> `ok`.

## Tests / commands and results
- `rtk git status --short` -> only `M tasks/completion/claims.json` (own row) plus this worklog before commit; clean otherwise.
- `rtk git branch --show-current` -> `docs/structured-subagent-prompts`.
- `rtk git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}'` -> `origin/docs/structured-subagent-prompts` (AFTER).
- `rtk git push --dry-run` -> `Everything up-to-date` / `ok (up-to-date)`, exit 0 (AFTER).
- `rtk git push --dry-run origin HEAD:docs/structured-subagent-prompts` -> `Everything up-to-date`, exit 0.
- `rtk git diff --check` -> empty, exit 0.
- `rtk git rev-parse HEAD` -> `f2c27569149e04b141efeeeb456ce4639f28d077` (bytes preserved).

## Decisions
- Correction is a local branch upstream-config change only; no ref movement, no content change.
- Did not push `-u` since remote ref already exists and matches HEAD; ordinary push now targets the policy ref.

## Remaining unknowns
- None for this lane. Later combined-policy verifier still owns acceptance.
