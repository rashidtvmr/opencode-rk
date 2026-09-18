# TUI-008-appr worklog (lane: `crates/cli/src/native_approvals.rs`)

## Claim
Pure-state approval dialog semantics: digest+scope binding, stale/replay
rejection, two-step focus guard via explicit `Confirm` enum, interrupt
receipts with broadcast generation, safe-vs-ambiguous retry classification.

## Source evidence
- HEAD `5af7884` (`git rev-parse HEAD`), file
  `crates/cli/src/native_approvals.rs:1-337` (pre-existing draft, 337 lines).
- Card TUI-008 via `python3 tools/completion_plan.py --card TUI-008`:
  T01 digest/workspace/requester/expiry/policy binding; T02 stale/replay
  rejection without side effects; T03 interrupt cancels real in-flight op,
  updates all clients; T04 retry safe-vs-ambiguous; T05 focus cannot
  accidentally approve destructive/human-only.
- `docs/SECURITY.md` §1/§5: human-only grants never auto-approved, explicit
  capabilities with expiration + policy-version bounds, fake issuers only.
- `docs/TDD.md` §3: RED must compile and fail for the missing behavior.
- Prior art `crates/cli/src/native_composer.rs:1-30` (pure state, bounded
  queues, std only).
- Gap found: pre-existing draft used `scope: String`, `age_ticks: u64`,
  `approve(digest, confirm: bool, armed: bool)` (caller-forgeable bools),
  no scope struct, no confirm enum, no in-flight/interrupt/retry types.
  Also: file is NOT referenced by `crates/cli/src/main.rs` (no
  `mod native_approvals;`); wiring is integrator-owned, not this lane.

## Observed scenario
RED revision: full new API + 10 tests written first; `decide()` ignored the
two-step guard and `interrupt()` always returned `None`.
`rustc --edition 2021 --test crates/cli/src/native_approvals.rs -o
/tmp/opencode/nap-red && /tmp/opencode/nap-red` → compiles, 3 tests fail
(focus guard destructive, human-only, interrupt receipt).
RED sha256 `818ad64e2d5bd2b69f18388a84da86947157aaabdfd9505ebed7adcbc723350b`.
GREEN sha256 `b9994406253f31776c52155e3144ede9a730eadc4cf63b2865b16218b46f437f`
(577 lines, 10/10 pass via `rustc --edition 2021 --test
crates/cli/src/native_approvals.rs -o /tmp/opencode/nap && /tmp/opencode/nap`).
One RED→GREEN fix round only: `history()` return type (`&[Decision]` →
`&VecDeque<Decision>` for rustc --test index compat); guard + interrupt
were the frozen RED behavior.

## Target boundary
OWNED FILE ONLY: `crates/cli/src/native_approvals.rs`. No other edits
(`main.rs` mod wiring left to integrator). ` #![forbid(unsafe_code)]`,
std only (`VecDeque`), no IO/clock/threads; caller supplies `now: u64`.

## Tests
`rustc --edition 2021 --test crates/cli/src/native_approvals.rs -o
/tmp/opencode/nap && /tmp/opencode/nap` — 10 tests, all green (see report).

## Decisions
- `ApprovalScope{workspace, requester, expires_at_tick, policy_version}` —
  expiry/policy bound per grant per SECURITY.md §1/§6.
- `Confirm{Unconfirmed, Confirmed}` explicit enum; Enter/focus activation maps
  to `Unconfirmed`, never approves destructive/human-only. Armed state stored
  on the request via `arm()`, not a caller bool (unforgeable).
- Replay rejected twice: `decide` on absent digest → `StaleOrReplayed`, and
  `offer` rejects digests already pending or in history (window MAX_HISTORY).
- Interrupt: `note_started()` registers the real op, `interrupt(now)` takes
  it and returns `InterruptReceipt{digest, interrupted_at_tick,
  broadcast_seq}`; `broadcast_seq()` bumps on every mutation so clients poll
  updates. `classify_retry()` → `AmbiguousSideEffects` if interrupted, else
  `SafeToRetry` (retry itself always under a fresh digest).

## Remaining unknowns
- `mod native_approvals;` wiring + renderer/focus-key mapping: integrator.
- Replay window bounded by MAX_HISTORY eviction: ancient digests forgettable;
  accepted bound, documented in file.
