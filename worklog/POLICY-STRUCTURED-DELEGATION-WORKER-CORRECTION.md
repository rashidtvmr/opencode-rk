# POLICY-STRUCTURED-DELEGATION-WORKER-CORRECTION

## Claim
- Task: POLICY-STRUCTURED-DELEGATION-WORKER-CORRECTION
- Session: ses_f2dd86343ffe7TOPNsJyJiqoZu
- Route: 9router/xk/qwen/qwen3.8-max:free
- Owned file: .agents/WORKER.md
- Scratchpad: this file

## Source evidence
- AGENTS.md (canonical contract): Required workflow steps 1-6, Completion report fields, Authority and ownership, Non-negotiable engineering rules, Subagent lane gating, Task-claim ledger sections.
- docs/TDD.md: section 3 (RED requirements including discovery validators), section 6 (independent review, implementer cannot self-accept).
- docs/SECURITY.md: section 1 (capability-based permissions), section 6 (stop semantics, never disable safeguard).
- docs/CONVERGENCE.md: Immutable-test rule (frozen tests read-only for implementers), Acceptance (claims.json is coordination not authority).
- .agents/WORKER.md (original): sections 1-7 covering claim, scratchpad, status, handback, commit/push, worktree, hard boundaries.

## Verifier findings addressed
1. Checklist parity: added section 8 with 10-item intake checklist mapping to AGENTS.md Required workflow, persistence/lifetime invariants, resource bounds, security posture.
2. Independent verification boundary: added section 9 forbidding self-verification, requiring separate verifier, preserving frozen-test immutability.
3. Emergency stop/revocation: added section 10 with immediate-stop-before-mutations, no-claim-held path (report only, no fabricated ledger op), claim-exists path (update only if safe), stop-exception-cannot-authorize-new-work.
4. N/A validation semantics: added section 11 restricting N/A to no-product-test-only, never bypassing discovery validators, frozen-test status, RED evidence, or contract validation.
5. Handoff schema alignment: replaced section 4's 3-item list with section 13's 13-field canonical ordered schema (task ID, type, role, status, model/route, analysis, changes, commands/results, commit/ref, hashes, resources, unresolved gaps, scratchpad path).
6. Route/allowlist validation: added section 12 requiring assigned route confirmation, allowlist check, canonical N/A handling, and scratchpad recording.
7. Existing safeguards preserved: sections 1-7 untouched; new sections 8-13 are additive only.

## Decisions
- Appended new sections rather than rewriting existing ones to preserve all prior claim, one-file, scratchpad, frozen-test, security, resource, stop-on-blocker, and landing rules.
- Section 4 (Hand back) retained its original text; section 13 supersedes it as the canonical schema reference while section 4 remains for backward compatibility of the narrative flow.

## Remaining unknowns
- Verifier report at worklog/POLICY-STRUCTURED-DELEGATION-VERIFY.md was referenced but not found on disk; corrections derived from task prompt's enumerated findings which cite the verifier verdict ACCEPT WITH CORRECTIONS.
