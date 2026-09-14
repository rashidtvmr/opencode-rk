# AUTO-007 worklog

## Claim

Implement one bounded typed turn-submission state machine: submit -> running -> complete or interrupted, with explicit invalid-transition errors and cancellation cleanup. Only one active turn is owned at a time, so submissions cannot form an unbounded queue.

## Source evidence

- Candidate base revision: `caf050398a84a6aced49b72c1c2bba9b5bb5555e`.
- `tasks/AUTO-007.md`: REQ-002/REQ-012, five test obligations.
- `docs/upcoming-features/codex-harness.md:11`: pinned Codex evidence describes the agent loop as a typed Op over a Thread and calls for a submission state machine.
- Current code: the agents crate has executor/session primitives but no typed turn lifecycle module.

## Observed scenario

There is no native contract representing submitted/running/interrupted/complete turn transitions or the ownership cleanup that happens when a running turn is cancelled.

## Target boundary

- Product implementation: `crates/agents/src/turn_state.rs` only.
- Shared pre-wire: `crates/agents/src/lib.rs` exports `turn_state` before test fan-out.
- Independent RED tests: `crates/agents/tests/turn_submission_state.rs` only.
- Status/evidence: this worklog and `tasks/AUTO-007.md`; `ralph.json` already records AUTO-007 as `in-progress`.

## Tests

- Independent RED file: `crates/agents/tests/turn_submission_state.rs`.
- Frozen SHA-256: `00fee7734af6b388f92c87870ee28f17a890381df1b0d7abccbb19a88dd1c4f2`.
- Initial authoring run failed to compile because the prewired `turn_state` module did not yet exist; this was authoring feedback, not RED evidence.
- After a signature-only scaffold, `cargo test -p opencode-rk-agents --test turn_submission_state` compiled and failed behaviorally: 1 passed, 4 failed. This is the frozen RED baseline.
- GREEN: the same focused command passed 5/5 with the frozen hash unchanged.
- Regression: `cargo test -p opencode-rk-agents` passed 10 existing library tests plus the 5 AUTO-007 integration tests.

## Decisions

- Keep one active turn per state-machine instance; a second submit before terminal cleanup is rejected instead of queued.
- Cancellation/interruption releases the active turn payload/owner state explicitly.
- Invalid transitions return typed errors and leave state unchanged.
- No detached task, queue, or retained tool/provider output is introduced by this state layer.
- Active turn identity is checked before phase transition validation, so wrong-turn requests cannot mutate the owned turn.
- Completion and interruption both clear the owned submission and return the state machine to `Idle`, allowing the next caller-owned submission.

## Remaining unknowns

- None for this slice. Controller/verifier acceptance remains a separate repository-owned step.
