# PAR-004-deleg worklog

## Claim

`crates/agents/src/app_delegation.rs`: parent/child delegation types —
ChildId, ParentLink, Ownership (ForegroundWait/BackgroundOwned),
IndependentEffort marker, local no-blind-retry classifier. std only,
`#![forbid(unsafe_code)]`.

## Source evidence

- HEAD `5af7884cf7637c0760d985da7a03f2f99ccd0c78`.
- `tasks/completion/parity.json:7` PAR-004 journey + 5 test bullets.
- `crates/agents/src/delegation_lane.rs:125-181` DelegationController
  pattern mirrored: owner token, bounded live cap, reclaimed cancel slot,
  `child_process_count()=0` precedent (`:296-299`), no OS process per child.
- `crates/agents/src/driver_lane.rs:29-152` OwnerToken + single-owner lease
  precedent (`LeaseTable::acquire/release`), no-blind blind-retry contrast
  (`ATTEMPT_CAP`, gate-before-commit).
- `docs/TDD.md:21-56` RED/GREEN; `docs/SECURITY.md:9-22` owner + bounds,
  no detached task without owner.

## Observed scenario

- Pre-change: no `crates/agents/src/app_delegation.rs`; parent/child
  steer/intended-child routing, ownership cancel rules, crash no-replay
  undefined in agents crate.
- RED probe: initial file with stubbed steer/cancel/crash compiled, failed
  3/4 (RED log `/tmp/opencode/adr.log`: steer Unknown, cancel Running,
  crash replay_allowed=true). RED SHA
  `5a0de202d6349af74773b9cc13030d56c820f23d2ae7d4159a1392d063a43ec0`.
  Tests frozen after RED; impl only changed to GREEN.

## Target boundary

- Owned: `crates/agents/src/app_delegation.rs` only. No other edits
  (lib.rs pre-wire left to orchestrator).
- Contract: spawn owner-checked steers to intended child only; background
  cancel reclaims live slot + idempotent; crash reclaims, ambiguous never
  replays; effort independent marker; local classifier, no cross-crate dep.

## Tests

- Cmd: `rustc --edition 2021 --test crates/agents/src/app_delegation.rs
  -o /tmp/opencode/ad && /tmp/opencode/ad` (rustc 1.96.1, no cargo build).
- RED: 1 passed, 3 failed (log `/tmp/opencode/adr.log`, build RC=0
  `/tmp/opencode/adb.rc`, run RC=101 `/tmp/opencode/adr.rc`).
- GREEN: 4 passed, 0 failed (log `/tmp/opencode/adr3.log`, clean build
  `/tmp/opencode/adb3.log` empty = zero warnings).
- Frozen GREEN SHA `e4b20259707dc3428906fc75508174015cdd9b7bb7697bd4a3a4d0932751dbb0`
  (368 lines).

## Decisions

- HashMap<u64, Entry> process-local records, live flag + state; cap 16
  default, AtCapacity over cap. Terminal: cancel-after-crash and
  crash-after-terminal -> Terminal. Crash uses last steered effect when
  present (steer records kind), else call-site kind.
- `ownership_of` accessor added to kill dead_code warning (build clean).
- Deliberate ceiling: no persistence, no cross-provider model select,
  no message/handoff copy — out of leased slice; `ponytail:` none needed,
  file is leaf types.

## Remaining unknowns

- lib.rs wiring (`pub mod app_delegation;`) not done — owned-file-only
  lease; orchestrator pre-wires.
- Full PAR-004 journey (spawn/fork/resume/retry/switch-model, persisted
  history, budgets/queues) spans other slices; this file covers
  steer/cancel/crash/ownership/effort subset only.
