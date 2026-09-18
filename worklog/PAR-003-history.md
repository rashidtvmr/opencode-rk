# PAR-003-history worklog

Task: PAR-003 history workflow types (`app_history.rs`). HEAD 5af7884.

## Claim
New owned-only module `crates/sessions/src/app_history.rs`: immutable
`ForkProvenance`, `RewindCursor` (history vs workspace scope), compact
call/result pair checker, import quota consts + checker, crash-recovery
marker. std only, `#![forbid(unsafe_code)]`. No other files touched.

## Source evidence
- `crates/sessions/src/branch_v2.rs:23-28` — existing `ForkProvenance`
  (Copy, pub fields, `SessionId`/`MessageId` typed) in DB-backed fork lane.
- `crates/sessions/src/branch_v2.rs:395-520` — `fork_before_user_message`
  / `fork_from_message` / `fork_provenance` semantics: parent+seq+boundary,
  depth bound, copy bound.
- `crates/sessions/src/web_008_lane.rs:14-22,53-59` — pure-state
  `ForkProvenance` (String ids) + fork depth/copy consts
  (`MAX_FORK_DEPTH=8`, `MAX_FORK_COPY_MESSAGES=500`).
- `crates/sessions/src/state.rs:1-101` — typed rewind contract
  (`revert` vs `rollback`); new `RewindCursor` extends with explicit
  workspace-effect scope.
- `crates/sessions/src/auto_compact.rs:1-95` — compact policy contract;
  new checker adds call/result pair retention invariant.
- `tasks/completion/parity.json:6` — PAR-003 journeys/tests: fork/rewind
  provenance, compact pairs, import quotas, crash recovery.
- `crates/sessions/src/import.rs:1`, `gc.rs:1` — stubs, no behavior to reuse.
- Current code = DB/pure lanes above; new requirement = PAR-003 test list
  in parity.json; deliberate deviation = String-id pure types (no rusqlite/
  uuid dep) so `rustc --test` verifies standalone.

## Observed scenario
- RED: `rustc --edition 2021 --test ... -o /tmp/opencode/ah` built with
  warnings only (exit 0); run FAILED 0 passed / 5 failed, each panicking on
  `todo!("RED: ...")`. Frozen RED hash
  `4c9592588a86a177c3137962490c54c5a5bb73c15e7b3b61ef9bf7f57c8d7636`
  (log `/tmp/opencode/red-run.log`, compile `/tmp/opencode/red.log`).
- GREEN: same command, build exit 0 with zero warnings, run exit 0:
  `5 passed; 0 failed`. Frozen GREEN hash
  `f366e41bf3e98b8bef5410523fceaf13e28a9ad08175a3ddc7f9656c539d6fc6`
  (logs `/tmp/opencode/green.log`, `/tmp/opencode/green-run.log`).
- Tests frozen after RED; never edited to pass — only `todo!()` bodies
  replaced with real logic.

## Target boundary
- Owns ONLY `crates/sessions/src/app_history.rs` (430 lines). No `lib.rs`
  wiring (orchestrator pre-wires shared files per lane-gating rule).
- Bounded: `HashSet<&str>` scoped to one call; import consts cap sessions/
  messages/bytes; batch math is `div_ceil`; marker encode is one line.
- No queue, no threads, no IO, no secrets, no unsafe.

## Tests
1. `fork_preserves_provenance` — clone/ getters/ equality.
2. `rewind_distinguishes_workspace_effects` — same seq, differing scope and
   `touches_workspace()`.
3. `compact_retains_pairs` — ok pair, SplitPair on dropped result,
   OrphanResult on dropped call.
4. `import_quota_bounds` — ok case, each quota arm, batch ceil (0/1/2).
5. `crash_marker_round_trip` — encode/decode equality, `BadEncoding` on junk.

## Decisions
- Private fields + getters (no setters) for provenance/marker immutability;
  `EmptyParent`/`EmptyBoundary` guards; tabs/newlines rejected in marker sid.
- `CompactError::SplitPair` = kept call missing result; `OrphanResult` =
  kept result missing call; both directions checked.
- Import consts: 1_000 sessions / 100_000 msgs per session (mirrors
  `MAX_MESSAGES` in `types.rs:13`) / 512 MiB total / 500-msg batches.
- Marker format `apphist-crash1\t<sid>\t<seq>\t<epoch>`; strict 4-field
  parse; constructor errors surface as `BadEncoding` on decode.

## Remaining unknowns
- Wiring into `lib.rs` + cross-crate ID types left to integrator.
- No e2e `tests/e2e/session_parity` in repo yet; lane covers unit contract.
