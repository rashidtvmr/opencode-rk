# POLICY-STRUCTURED-DELEGATION-WORKER

- Claim: `in-progress`; session `ses_f2dfc25c9ffeJyXsBvNHRgHsSF`.
- Owned file: `.agents/WORKER.md`.
- Task type: implementation. (Corrected from invalid `policy/documentation` to canonical enum per AGENTS.md:131.)
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/structured-subagent-prompts`.
- Branch: `docs/structured-subagent-prompts` (tracks `origin/lane/PHASE1-product-spine-20260923`, ahead 1).
- Source evidence:
  - `AGENTS.md:23-189` structured delegated-task prompt contract + canonical template (commit `0f8829a9942191394b83f4055de6d3c877aa31c1`).
  - `.agents/WORKER.md:1-155` existing worker protocol (claim, scratchpad, status, handoff, landing, boundaries).
  - `tools/completion_claims.py:94-192` claim/update/release fail-closed semantics.
  - `tasks/completion/claims.json:930-935` precedent row `POLICY-STRUCTURED-DELEGATION-AGENTS`.
- Target boundary: add a MUST-level worker-side structured-brief intake gate to `.agents/WORKER.md`, cross-referencing the `AGENTS.md` template (no second contradictory template), preserving every existing safeguard.
- Decisions:
  - New `## 0` section references the canonical template instead of duplicating it.
  - 16 required intake fields enumerated; rejection triggers for incomplete/contradictory briefs; fresh-context, allowlist, evidence-first, bounded-prompt rules.
  - Emergency-stop instructions retained as authoritative and overriding.
  - Section 4 handoff updated to require the full `AGENTS.md` handoff schema fields.
- Tests: docs-only. `rtk git diff --check`; item-by-item review against the ten in-scope requirements; `git diff --name-only` scoped to three paths.
- Remaining unknowns: remote containment of the branch after push (verify local HEAD == `origin/docs/structured-subagent-prompts`).