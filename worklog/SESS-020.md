# SESS-020 worklog

## Claim

Implement auto-compaction policy thresholds, warning states, a three-consecutive-failure circuit breaker, and explicit compact/auto-compact kill switches without introducing a background queue or detached owner.

## Source evidence

- Candidate base revision: `991ae5bb550cb102552bb7eec70b9789f1f77d22` plus current task work.
- `tasks/SESS-020.md`: REQ-006, five test obligations.
- `docs/upcoming-features/claude-code-harness.md:53-59`: source `src/services/compact/autoCompact.ts` records summary max output 20,000, auto buffer 13,000, warning/error buffer 20,000, manual buffer 3,000, three-strike breaker, and `DISABLE_COMPACT` / `DISABLE_AUTO_COMPACT` kill switches.
- `docs/upcoming-features/SYNTHESIS.md:9,39`: adopts the threshold/breaker shape under REQ-006.

## Observed scenario

Foundation config currently has a generic `CompactionSettings` threshold, but there is no session-level auto-compaction decision state, warning state, failure breaker, or kill-switch contract.

## Target boundary

- Product implementation: `crates/sessions/src/auto_compact.rs` only.
- Shared pre-wire: `crates/sessions/src/lib.rs` exports `auto_compact` before test fan-out.
- Independent RED tests: `crates/sessions/tests/auto_compact_policy.rs` only.
- Status/evidence: this worklog and `tasks/SESS-020.md`; `ralph.json` already records SESS-020 as `in-progress`.

## Tests

- Independent RED file: `crates/sessions/tests/auto_compact_policy.rs`.
- Frozen SHA-256: `b7da170a2ed32740ddd156359db2bb0cbba79627918cd90571189b748380a53e`.
- The worker's initial run failed at compile time because the intentionally prewired product module did not exist; this was authoring feedback only.
- With a signature-only scaffold, `cargo test -p opencode-rk-sessions --test auto_compact_policy` compiled and failed behaviorally: 1 passed, 4 failed. This is the frozen RED baseline.
- GREEN: `cargo test -p opencode-rk-sessions --test auto_compact_policy` -> 5 passed, 0 failed.
- Regression: `cargo test -p opencode-rk-sessions` -> 30 library tests, 5 auto-compact tests, and 5 history-rewind tests all passed; doc tests passed.
- Frozen hash was rechecked unchanged before GREEN.
- Plan validation: `python3 tools/validate_plan.py` -> `validate_plan: OK stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `git diff --check` for the SESS-020 product/test/task/worklog paths passed.

## Decisions

- Keep the policy synchronous and caller-owned; it decides whether a caller may/should compact but never spawns work.
- Represent kill switches as explicit policy inputs so tests need not mutate process-global environment; the runtime adapter can map env values into that policy.
- Failure breaker opens after three consecutive failed compactions and resets after success.
- The failure counter saturates at three, avoiding overflow and preventing repeated failed auto-compaction retries from growing state.
- Manual compaction remains available when only auto-compaction is disabled; the global compact kill switch suppresses both.
- The policy is a pure synchronous decision over caller-owned input/state and creates no queue, task, timer, or retained transcript.

## Remaining unknowns

- Formal acceptance remains verifier/controller-owned; this worklog records implementation and test evidence only.
