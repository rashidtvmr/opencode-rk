# AGENT-exec worklog

## Claim

`crates/agents/src/agent_executor.rs`: real-loop scaffold pure state machine —
LoopStep (ProviderCall/ToolDispatch/PolicyCheck/Settle), cancellation-checkpoint
helper, ambiguous-effect no-retry classifier, resource-reclaim receipt. Caller-supplied
IO only (no provider/tool calls inside). std only, `#![forbid(unsafe_code)]`.
316 lines.

## Source evidence

- HEAD `5af7884cf7637c0760d985da7a03f2f99ccd0c78`.
- Task brief cites `crates/agents/src/executor.rs:83-110` (ExecutionManager stub
  `execute` returns Ok String, no real loop) and `crates/agents/src/turn_state.rs:62-128`
  (single-turn submit/start/complete/interrupt state machine) — loop scaffold sits
  above both, drives multi-step sequences without touching either file.
- PAR-004 journey (`tasks/completion/parity.json:7`): parent/child crash and
  cancellation release resources, never silently replay ambiguous tool effects.
- Pattern mirrored: `crates/agents/src/app_delegation.rs:97-103` local
  no-blind-retry `classify` (Ambiguous -> NeverReplay); `:242-245,277-280`
  cancel/crash reclaim live slot; `crates/agents/src/delegation_lane.rs:241-248`
  cancel reclaims live_task + drops summary.
- `docs/TDD.md:21-56` RED/GREEN; `docs/SECURITY.md:9-22` owner + bounds, no
  detached task without owner, no auto-replay of ambiguous side effects.

## Observed scenario

- Pre-change: no `crates/agents/src/agent_executor.rs`; no LoopStep type, no
  checkpoint helper, no loop-scoped ambiguous classifier, no reclaim receipt.
- RED probe: full test module + stub bodies (classify always RetryAllowed,
  checkpoint None, complete_step blind advance, request_retry Ok, run_with empty
  report). Tests frozen after RED authoring; impl only changed to GREEN.
- RED build log empty (zero warnings); RED SHA not separately frozen (stub edited
  in place per no-test-edit rule — tests frozen, impl fixed).

## Target boundary

- Owned: `crates/agents/src/agent_executor.rs` only. No other edits
  (executor.rs, turn_state.rs, lib.rs untouched; lib.rs pre-wire left to orchestrator).
- Contract: `AgentExecutor::new` rejects plans over MAX_STEPS=64 (AtCapacity);
  `next_step` peeks, caller does IO, `complete_step(effect)` advances + returns
  `classify(effect)`; `request_retry(Ambiguous)` -> AmbiguousReplayDenied;
  `request_cancel` + `checkpoint()` reclaims live slot, drops tail, emits
  ReclaimReceipt{steps_completed, steps_dropped, reclaimed:true}; `run_with(io)`
  drives bounded sequence to Settle; terminates (cursor==len, live=false).
- Deliberate ceiling: no provider/tool/policy IO inside, no persistence, no
  cross-crate deps, no threads/processes — caller-supplied IO keeps file
  standalone-compilable under `rustc --test`; `ponytail:` none needed, leaf types.

## Tests

- Cmd: `rustc --edition 2021 --test crates/agents/src/agent_executor.rs -o /tmp/opencode/axe && /tmp/opencode/axe`
  (rustc 1.96.1, no cargo build).
- RED: build clean (empty `/tmp/opencode/axb.log` at stub stage); expected
  failures: cancel_reclaims (None), ambiguous_never_retried (RetryAllowed),
  sequence_terminates (settled=false) — 3 fail / 2 pass by construction
  (stub bodies documented RED STUB; run log captured only at GREEN per lane).
- GREEN: 5 passed, 0 failed (log `/tmp/opencode/axr.log`).
- Frozen GREEN SHA `af673750138ee4df410fffdc2a967dd01b2a9a0b9a8b082ce4557ab3f78a2868`
  (316 lines, copy `/tmp/opencode/axe-green.rs`).

## Decisions

- `classify` local const fn (no cross-module dep) so file compiles standalone;
  semantics identical to app_delegation::classify.
- `checkpoint()` returns None while running (no pending cancel) — covers
  `checkpoint_none_when_running` negative; Some(receipt) only on cancel.
- `complete_step` on cancelled loop -> Cancelled (no silent advance past cancel).
- `request_retry` on dead loop (terminal, non-ambiguous) -> Terminal.
- `run_with` checks cancel between steps; io closure returning Ambiguous still
  terminates (reports NeverReplay, no auto-retry loop — bounded by plan len).

## Remaining unknowns

- lib.rs wiring (`pub mod agent_executor;`) not done — owned-file-only lease;
  orchestrator pre-wires.
- Integration with ExecutionManager/turn_state (real loop driving real
  provider/tool calls) spans other slices; this file is the pure scaffold.
