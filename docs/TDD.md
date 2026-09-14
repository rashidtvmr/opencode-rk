# TDD contract: trusted RED/GREEN pipeline

This document is the authoritative test-driven development contract for the Lean
Harness. Every worker (main and subagents) must follow it before modifying product
code. It is binding: a worker cannot accept its own slice and cannot rewrite these
rules. See PLAN.md sections 5-6 and 8, and the AGENTS.md worker contract, which
this document operationalizes.

## 1. Scope and authority

- Tests and verifier configuration are **not writable by the implementation
  sandbox**. The implementer proposes tests; the trusted controller freezes them
  and the independent verifier runs them (ADR-007, PLAN.md section 6).
- A slice is ready only when its dependency revisions and required contracts are
  accepted (PLAN.md section 5). A worker owns only its leased task and approved
  paths (AGENTS.md, Authority and ownership).
- Never claim a feature exists because its module compiles or its test was mocked
  away. Compiled code, a passing unit suite and a clean compiler are not complete
  acceptance (AGENTS.md, Completion report; PLAN.md section 10).

## 2. Mandatory lifecycle (PLAN.md section 6)

For each slice, in order:

1. Inspect sources and runtime evidence. Cite the exact repository commit, path
   and line/symbol for each discovered behavior. Classify each as: native V2,
   shared/legacy compatibility path, partial or planned upstream, new user
   requirement, or deliberate safer/resource-bounded deviation with a recorded
   contract (PLAN.md section 2).
2. Define the observable contract, failure states, ownership/lifetime,
   persistence transitions and resource bounds.
3. Author tests. Establish a **compiling RED** that fails for the missing
   behavior.
4. Freeze tests and the command manifest.
5. Implement the minimum behavior natively in Rust; retain protocol and
   behavioral semantics rather than translating files line by line (ADR-003,
   AGENTS.md step 4).
6. Run **GREEN**, refactor, and rerun.
7. Run independent regressions, negative/security tests, lifecycle/resource
   tests and differential fixtures.
8. Merge through the integrator; rerun on the exact integrated revision.

## 3. RED requirements

- The RED suite must **compile** and **fail for the missing behavior**. Compile
  failures, missing imports, tests that never execute, disabled tests,
  `#[ignore]`, a fabricated successful log and a worker-edited `passes:true` are
  not valid evidence (PLAN.md section 6).
- Five named tests per task are a minimum taxonomy, not five sufficient tests.
  Expand into concrete cases.
- A denied permission must assert **absence of side effects**, not just an error
  string. Cancellation must assert tasks/processes/FDs are actually reclaimed.
  Partial streams, provider errors and process crashes need fixtures
  (PLAN.md section 6).
- A purely declarative or discovery task still needs executable validators and a
  captured failing fixture; do not invent product tests for it.

## 4. Freeze the hash

- After the RED suite compiles and fails as expected, **freeze the test hash and
  the command manifest**. The frozen hash is the exact revision the verifier
  checks. Test source, verifier configuration, resource limits, scope and
  reference pins are not writable by the implementation sandbox (ADR-007).
- The controller re-checks the frozen artifact against the file on disk; it does
  not trust a subagent's self-report. A lane is done only when its artifact
  passes the gate, which re-reads the file and runs the test target.

## 5. GREEN and refactor

- Implement the minimum behavior that turns the frozen RED green. Retain
  protocol and behavioral semantics rather than line-by-line translation.
- After GREEN, refactor and rerun the full affected suite. Green on the frozen
  tests plus clean independent regressions is the bar before submission.
- Submit **evidence and a patch**, never acceptance. The verifier, not the
  implementer, decides whether the slice is integrated and accepted (ADR-007).

## 6. Independent review

- The implementer cannot mark its own story accepted. A trusted controller owns
  state and invokes an independent verifier against frozen tests and exact trees
  (ADR-007, PLAN.md section 6).
- Report unresolved gaps and exact reproductions rather than asserting completion
  (AGENTS.md step 6).

## 7. Test-fixture determinism (AUTO-002)

- No changes to the user's existing OpenCode database in the implementation loop.
  Use generated test datasets or an explicitly provided read-only copy. Never run
  host-destructive test commands; use disposable restricted fixtures
  (AGENTS.md, Non-negotiable engineering rules).
- A denied permission asserts absence of side effects, not merely an error string.
- Test fixtures are deterministic: no network, no wall-clock dependence, no
  inherited-secret access, fixed hashes for frozen tests. Truncating or mutating
  a shared database is forbidden; tests build their own restricted state.

## 8. Stop semantics

- Retry only classified repairable failures. After the retry cap, record exact
  blockers and continue independent tasks. Stop when no safe ready work remains,
  budget is exhausted, required credentials are unavailable, a platform
  isolation test fails, or mandatory human authority is required (PLAN.md
  section 8).
- On failure, preserve a minimal reproduction and stop or request the next safe
  task. Never disable a safeguard to keep the loop moving (AGENTS.md).
- A blocked state is not a completed project. Do not emit `passes:true` as proof.
