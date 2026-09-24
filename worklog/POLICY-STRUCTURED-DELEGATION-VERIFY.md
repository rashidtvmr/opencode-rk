# POLICY-STRUCTURED-DELEGATION-VERIFY

## Handoff

- Model: `9router/oc/space-bunny-free`
- Task ID/type: `POLICY-STRUCTURED-DELEGATION-VERIFY` / independent `verification`
- Ledger: `completed`; session `ses_f2df2827bffepxCodKqLThdFlm`
- Owned paths: this worklog; own `tasks/completion/claims.json` row only
- Verdict: **ACCEPT WITH CORRECTIONS**
- Integration recommendation: **Do not integrate unchanged.** Candidate commits are additive, remotely contained on the topic branch, and preserve existing safeguards, but the policy contract has correctable fail-closed/authorization gaps. Re-review after corrections; do not claim repository acceptance.

## Analysis

Candidate chain: `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b` -> `0f8829a9942191394b83f4055de6d3c877aa31c1` (`AGENTS.md`) -> `c083eabe99a2b6e0abbe613ff9700313699171e8` (`.agents/WORKER.md`). Current policy files equal `c083eab`; only verifier worklog/ledger changes are uncommitted. No product, test, plan, controller, security-policy, or verifier-code files changed in the candidate commits.

`AGENTS.md` adds a self-contained brief contract and reusable Markdown template. `.agents/WORKER.md` adds a pre-claim intake gate, rejection triggers, authorization/emergency behavior, fresh-context/evidence rules, and handoff cross-reference. The additions are not destructive: `git diff --numstat 5d66683 0f8829a -- AGENTS.md` = `168 0`; `git diff --numstat 0f8829a c083eab -- .agents/WORKER.md` = `78 0`; parent lines remain present in order.

## Requirement matrix

| ID | Verification | Exact evidence | Result |
|---|---|---|---|
| 1 | AGENTS requires role/expertise, observable goal, context/authority, identity/worktree/branch/one file, type, scope/non-goals, invariants, deliverables, measurable criteria, exact validation/RED-GREEN, blocker, landing, handoff | `AGENTS.md:25-64`; template `:90-188` | **PASS** for the listed core fields |
| 2 | Reusable template; fresh context; concise/no transcript dump; allowlist; role separation; disk verification | Template `AGENTS.md:84-189`; fresh/concise/role/disk rules `:29-30,66-82,105-111`; worker `:53-77` | **PARTIAL**: prose mandates allowlist, but no canonical template slot/assigned route or explicit no-allowlist `N/A` |
| 3 | Worker validates same fields before claim/file mutation; stops on absent/empty/contradictory brief | `.agents/WORKER.md:7-18,20-44`; canonical reference `:9-15` | **PARTIAL**: strong fail-closed gate; explicit persistence and independent-verification-boundary checklist parity is missing |
| 4 | Ownership conflict, mixed types, implementer/verifier mixing, frozen-test ambiguity, security/resource omission, worktree/branch mismatch, unauthorized route | `.agents/WORKER.md:46-51`; route stop `:60-63` | **PASS**, with unauthorized route covered by authorization stop rather than reject list |
| 5 | Emergency stop/revocation actionable without full brief | `.agents/WORKER.md:58-66` | **PARTIAL/CORRECTION**: immediate stop is clear; no-claim ledger-status action and AGENTS precedence are undefined |
| 6 | Cross-file contradictions, loopholes, MUST/SHOULD ambiguity, circular/one-file issues, handoff drift | `AGENTS.md:25-82,90-189`; `.agents/WORKER.md:7-77,134-148` | **CORRECTIONS REQUIRED**: findings F1-F5 below |
| 7 | Existing TDD/frozen-test, security, convergence, resource, claim, scratchpad, landing safeguards | Parent/current suffix comparison; `AGENTS.md:191-270,272-304,342-375`; `docs/TDD.md:45-66`; `.agents/WORKER.md:79-233` | **PASS with TDD wording correction**; no deletions, but test-free/N/A wording needs explicit reconciliation |
| 8 | Topic-branch upstream mismatch and remote containment | `git config`/`git push --dry-run`; refs below | **Candidate remote-contained; landing path requires correction** |
| 9 | Three brief scenarios | Rules cited in scenario table | Complete implementation passes; incomplete raw instruction rejects; urgent stop partially ambiguous |

## Findings and required corrections

### F1 — Authorization field is prose-only (high)

`AGENTS.md:70-73` says a user-supplied worker/model allowlist is authoritative and **must** be copied into every brief. `.agents/WORKER.md:40,60-63` requires and enforces it. The reusable template `AGENTS.md:90-189` has no routing/allowlist heading, assigned route field, or `N/A — no user allowlist` slot. A literal template copy can therefore omit the allowlist, and omission is indistinguishable from no user allowlist. Add one canonical routing section containing assigned route, exact allowlist, and explicit `N/A` plus reason; require it in the worker checklist.

### F2 — Emergency stop is not mutually complete (high)

`AGENTS.md:25-29` forbids raw/partial briefs and has no `emergency`, `revoke`, or stop-precedence field (whole-file probe: zero matches). `.agents/WORKER.md:64-66` says stop/revocation overrides continued work, stop at once, set an honest ledger status, and report. If a stop arrives without a full brief, the worker can stop, but cannot satisfy “set ledger status” if no claim exists; the raw instruction also conflicts with the AGENTS raw-brief prohibition. Add an explicit stop-first exception: no claim/edit/side effect, preserve/report out-of-band when unclaimed, mark `blocked` only when claimed, and state precedence over brief completeness.

### F3 — Worker checklist is not exact field parity (medium)

AGENTS requires persistence invariants (`AGENTS.md:46-48`) and an independent-verification boundary (`AGENTS.md:66-70`, template `:102-103`). The worker checklist names `resource/lifetime` and task type (`.agents/WORKER.md:24-32`) but omits explicit persistence and the verifier/later-lane field. Add both. The canonical-template reference reduces practical risk but does not make the checklist itself complete.

### F4 — Test-free/N/A language can bypass discovery TDD evidence (high)

`docs/TDD.md:55-56` requires declarative/discovery tasks to retain executable validators and a captured failing fixture. New `AGENTS.md:55-56,150-156` and `.agents/WORKER.md:48-49` allow research/verification/test-free `N/A` based on a stated reason. `AGENTS.md:30-32,81-82` says TDD remains binding, but the exception is not reconciled. Clarify that `N/A` means no product test only; discovery still needs the TDD executable validator/failing fixture. A bare “test-free with reason” must not pass intake.

### F5 — Handoff schema wording drifts (low/medium)

The AGENTS prose list (`AGENTS.md:63-64`) omits task ID/ledger status, while its template includes them (`:178-188`). Worker section 4 first requires task ID/final status (`.agents/WORKER.md:136-142`) but its restatement (`:144-148`) says “task id and role/type” and omits status. Use one canonical ordered schema: task ID, role/type, ledger status, model, analysis, changes, commands/results, commit/ref, hashes, resources, unresolved gaps.

### F6 — Topic branch landing command is currently unusable (operational high)

Local `docs/structured-subagent-prompts` tracks `origin/lane/PHASE1-product-spine-20260923`, not its same-named remote. `rtk git push --dry-run` exits 128: upstream branch name mismatch. `origin/docs/structured-subagent-prompts` is already `c083eab` and contains both candidates, so remote containment is proven; default landing is not. Use an explicit `git push origin HEAD:refs/heads/docs/structured-subagent-prompts` or correct upstream before integration. Do not claim `origin/main` integration: candidate is not an ancestor of `origin/main` (`8a91a7b...`).

### F7 — Candidate process evidence uses an invalid task type (medium)

`worklog/POLICY-STRUCTURED-DELEGATION-WORKER.md:5` says `Task type: policy/documentation`; AGENTS permits only `implementation`, `RED authoring`, `research`, `integration`, or `verification` (`AGENTS.md:66-67`). This is task-artifact evidence, not normative policy, but should be corrected/annotated before relying on that scratchpad as a compliant lane record.

No harmful circular reference was found in the two policy files. The existing `AGENTS.md` <-> `.agents/WORKER.md` cross-reference is intentional canonical ownership. The older `prompts/COMPLETE_APP.md:91-130,153-154` claim/scratchpad mirror does not contain the new structured template; this is a follow-up wiring/documentation drift, not a reason to edit that file in this lane.

## Existing-safeguard comparison

- **TDD/frozen tests:** Parent `AGENTS.md:45-58` maps to current `:213-226`; current `:52-59,164-165,202-205` retains RED/frozen-test prohibitions. `docs/TDD.md:45-66` remains authoritative. F4 is the only material ambiguity.
- **Security:** Parent/current `AGENTS.md:60-75` maps to current `:228-243`; candidate does not touch `docs/SECURITY.md`. Capability broker, secret, sandbox, wildcard, and disposable-fixture rules remain.
- **Convergence:** Parent `:23-43` maps to current `:191-211`; immutable tests, parent-open boundary, and integration/verifier capacity remain.
- **Resources:** Parent `:104-132` maps to current `:272-300`; 8 GiB envelope, reserve, one-heavy-command rule, bounded parallelism remain. New template adds bounded commands/resource observation at `AGENTS.md:146-159`.
- **Claims/scratchpads/landing:** Current `AGENTS.md:342-375` and `.agents/WORKER.md:79-176,222-233` retain fail-closed claim, scratchpad, honest status, no release-by-worker, commit/push, rebase/rerun, and no-force rules. Candidate diff has zero deletion lines.

## Scenario outcomes

1. **Complete implementation brief:** Includes every canonical heading, one `implementation` type, explicit independent verifier/later lane, exact path/ref/one file plus scratchpad/ledger, scope/non-goals, persistence/security/resource invariants, frozen RED hash/command, GREEN command/state, blocker/landing/handoff, and allowlist/route. Worker intake passes (`.agents/WORKER.md:17-40`); no reject trigger applies; implementation may claim only after validation. **PASS**.
2. **Incomplete raw instruction:** “Fix the parser; run tests; do not touch unrelated files.” Missing role, observable goal, context, identity, type, explicit invariants, deliverables, measurable criteria, exact expected states, blocker, landing/handoff, and allowlist. AGENTS rejects raw/partial briefs (`:25-29`); worker stops before claim/edit and requests correction (`.agents/WORKER.md:42-44`). **PASS / fail-closed**.
3. **Urgent stop:** “STOP; revoke authorization; do not edit.” Worker must stop at once (`.agents/WORKER.md:64-66`), but no-claim status handling and AGENTS precedence are undefined (F2). **PARTIAL / correction required**.
4. **Unauthorized route (additional check):** Assigned route absent from copied allowlist; worker stops and reports, never substitutes (`.agents/WORKER.md:60-63`). **PASS** once F1’s canonical slot is added.

## Commands and results

- `rtk git fetch --prune origin` -> OK; `origin/docs/structured-subagent-prompts` = `c083eabe99a2b6e0abbe613ff9700313699171e8`; both candidate commits are ancestors.
- `rtk git diff --numstat 5d66683 0f8829a -- AGENTS.md` -> `168 0`; worker commit diff -> `78 0`; zero policy deletion lines.
- Normalized inline policy matrix -> `checks=32 failed=0` (line-wrapped phrases normalized). Initial unnormalized checker produced two false negatives from line wrapping, not policy findings.
- Omission probe -> template allowlist count `0`; AGENTS emergency count `0`; worker checklist persistence count `0`; independent-boundary count `0`; invalid scratchpad task-type count `1`.
- `rtk git diff --check` on candidate policy diffs and working tree -> PASS (`0`).
- `rtk python3 tools/convergence_gate.py` -> exit 1, `CONVERGENCE BLOCKED`, pre-existing/off-plan `total=92`; no repository acceptance.
- `rtk python3 tools/validate_repository.py` -> exit 1, `validate_backlog_exhaustion: 51 error(s)`; protection fixture says platform state unverified. No acceptance claim.
- `rtk git push --dry-run` -> exit 128, upstream branch name mismatch; no push occurred.
- `rtk sysctl -n hw.memsize` -> `25769803776`; `rtk vm_stat` recorded only. No Cargo/build/browser/database command run.
- `rtk timeout 120 python3 tools/convergence_gate.py` -> exit 127 because macOS `timeout` executable is unavailable; reran with the tool’s 120-second bound.

## Scope, landing, unresolved gaps

Changed paths at verifier handoff: `worklog/POLICY-STRUCTURED-DELEGATION-VERIFY.md`; `tasks/completion/claims.json` own row only. `AGENTS.md`, `.agents/WORKER.md`, product code, tests, plans, controller/security/verifier files unchanged by this verifier.

Required before integration: F1-F5 policy corrections; branch upstream/explicit-ref correction (F6); candidate task-type metadata correction (F7); code-owner review because `AGENTS.md` is protected (`.github/CODEOWNERS:10`, `.github/protection-policy.json:20-36,98-100`); rebase/merge to `origin/main`, rerun canonical checks on integrated revision. Current convergence/ledger debt remains independently blocked. No unresolved policy gap may be treated as accepted.
