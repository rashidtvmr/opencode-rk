# POLICY-MANDATORY-DELEGATION-W2

- Claim: `POLICY-MANDATORY-DELEGATION-W2`, session `ses_f1d348324ffeMfncDKYS10pGVB`; claimed before editing.
- Base: `cb64da11ea8afe675890bd799c6062daee989d8f`; branch `policy/MANDATORY-SUBAGENT-DELEGATION-W2`.
- Evidence: `AGENTS.md` mandatory-delegation section; `.agents/WORKER.md` requires one owned file and claim/reclaim safeguards; `docs/TDD.md` sections 2-6 require frozen tests and independent verification; `docs/SECURITY.md` requires broker authority and explicit human boundaries; `docs/CONVERGENCE.md` requires exact integrated-revision evidence; `PLAN.md` section 1 and `sources/upstream.lock.json` pin V2 parity behavior.
- Scenario: prior policy allowed repository research to be merely delegable, an undefined integration-lease multi-file exception, incomplete failed-lane handling and unspecified completion receipts.
- Boundary: only the mandatory-delegation section in `AGENTS.md`; scratchpad and own claim row. No changes to worker/TDD/security/convergence policy or V2 contract.
- Change: require unconditional delegation of all source research; prohibit main-agent task research; enforce one-product-file ownership with shared-file serialization; require recorded reclaim/re-delegation for all listed failures; define exact completion receipt fields; retain V2 parity cross-reference.
- Tests: docs-only; no new/frozen test edits. Required verification: `git diff --check`, `python3 tools/validate_repository.py`.
- Resource notes: no resource-intensive build/test run.
- Verification: `rtk git diff --check` passed. `rtk python3 tools/validate_repository.py` failed at baseline and candidate with the same 51 backlog-exhaustion errors (accepted/unknown classifications and stale ownership-gap manifests, including OPS/REL/EXT/INT/SHARE/ROUTE findings); protection-policy checks passed. Baseline reproduced in detached worktree at the exact requested commit. No policy safeguard weakened.
- Remaining: commit/push hash/ref, independent verifier verdict or exact blocker.
