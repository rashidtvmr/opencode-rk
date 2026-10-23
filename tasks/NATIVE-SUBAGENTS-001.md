# NATIVE-SUBAGENTS-001 — Repair delegation and ship built-in agents

Status: PARTIAL — ten-role catalogue/execution and listed focused regressions are implemented; provider transient automatic retry remains UNIMPLEMENTED/UNPROVEN; exact-release installed gates remain pending. Do not reopen the verified role-catalogue/execution portion.
Requirement: `docs/features/BUILTIN_SUBAGENTS.md`.
Base product revision at report: `99ae97617c8155633d1eca3bdf5df616de92fd77`.

## Coherent behavior package

Make native oc2 delegation work end to end with predefined agents, custom initial
and mid-session prompts, safe transient retries/manual retry, child inspection,
parent-routed HITL, and keyboard/mouse controls.

## Execution checklist

- [x] Delegate/read authoritative GSD, OpenCode, Codex and Claude Code research.
- [ ] Capture current delegation failure with disposable provider/daemon fixture.
- [ ] Record role catalogue, policy/prompt contracts and lifecycle scenario matrix.
- [ ] Establish compiling RED tests for failure and missing scenarios.
- [x] Implement default ten-role catalogue preserving configured agents.
- [x] Implement ten-role child execution (documented focused role regression passes; do not reopen this portion).
- [x] Implement active-child follow-ups and bounded exponential retry of pre-side-effect admission-capacity exhaustion only.
- [ ] Implement bounded exponential automatic retry for eligible transient provider failures without replaying completed or ambiguous side effects.
- [ ] Implement accessible inspect/stop/retry/follow-up native controls.
- [x] Test all ten roles for canonical identity and execution (candidate regression evidence; installed acceptance pending).
- [ ] Test remaining required lifecycle/error/security scenarios.
- [ ] Independently review final candidate, including permission boundaries.
- [ ] Release-build/install and verify exact-SHA real native journeys.
- [ ] Report measured coverage and remaining unproven scenarios accurately.

No queue/controller migration is required to execute this product repair; recorded
Phase-1 user override remains in effect. One integration writer; bounded builds.

## Verification progress (dirty candidate; not installed acceptance)

- Native CLI: 606 tests passed, including agent catalogue and child metrics.
- Server library: 221 tests passed, including catalogue policy/alias contracts.
- Child lifecycle: 6 tests passed; real foreground and background tests passed.
- Real saved task denial and router/storage restart-outcomes tests passed.
- Automatic provider transient retries, remaining lifecycle/security cases, final
  independent review and exact-release installed journeys remain pending.

Evidence and current path owners: `docs/worklogs/NATIVE_UX_SUBAGENTS_SCRATCHPAD.md`.
Earlier results must be rerun when later source changes affect their gates.
