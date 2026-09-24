# APP012-RESTART-RECOVERY-VERIFY

## Claim

- Task: `APP012-RESTART-RECOVERY-VERIFY`. Type: independent verification, no source/test edits.
- Session: `ses_f2b1a028fffeHx6hK43qUNa6re`. Claimed via `tools/completion_claims.py` before file creation.
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-app012-restart-recovery`, branch `verify/APP012-RESTART-RECOVERY`.
- Owned artifact: this file only, plus own ledger row.
- Candidate revision: `f83637d` (lane APP012 restart recovery green). Parent `929acfc` (RED).
- Base HEAD at verify start: `f83637df72c0410cfde84bdee225d0b060248c2c`, clean tree.

## Authority read

- `.agents/WORKER.md`, `docs/TDD.md`, `docs/SECURITY.md`, sqlite-safety skill.
- Accepted contract `worklog/APP012-RESTART-RESUME-CONTRACT.md`, correction `...-CORRECTION.md`, RED record `...-RED.md`, GREEN record `...-GREEN.md`.
- Implementation diff `929acfc..f83637d -- crates/storage/src/facade.rs` (80 lines, `recover_existing` + call).
- Schema `crates/storage/schema/v2/workspace.sql`, owners `execution_v2.rs`, `approvals_v2.rs`, `schema_v2.rs`, protocol `docs/storage/CRASH_CONSISTENCY.md`.

## Frozen hashes (verified this session)

- `crates/storage/tests/app012_restart_resume_red.rs` = `d2745fe142fb1add3cc26c9fe9715438c73a51084d5498373c990c0a8e2a8cce` (matches RED freeze).
- `crates/server/tests/app012_tool_journey_red.rs` = `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` (untouched).
- Test file zero-diff vs `929acfc`: `git diff 929acfc HEAD -- crates/storage/tests/` empty.

## Audit: startup protocol in `facade.rs:148-222`

- One IMMEDIATE tx under FULL: `transaction_with_behavior(Immediate)` (`:149`); `CONNECT_POLICY` with `synchronous=FULL, busy_timeout=5000, FK ON` applied in `open_existing` before recovery (`schema_v2.rs:140`). Single writer (facade `Mutex`). PASS.
- Markers exactly once: one `SELECT owner_generation, clean_shutdown` (`:150-154`), one CAS `UPDATE ... owner_generation=?1, clean_shutdown=0 WHERE id=1 AND owner_generation=?2` (`:160-165`), `changed != 1` fails closed (`:166-168`). Overflow: `checked_add` errors before any write (`:155-159`). PASS.
- Bound 500 deterministic: `STARTUP_RECOVERY_LIMIT=500` (`:18`), all three scans `ORDER BY pk LIMIT ?2` (2 bound params each, <=4 max). Root scan selects `e.owner_generation < ?1 AND e.state=1` prior-generation running roots only. PASS.
- Legal transitions: executions `1->4` allowed (`execution_v2.rs:217-222`); attempts direct `SET state=4, finished_at_us=NULL` matches DDL open-state CHECK (`workspace.sql:183`) and `finish_attempt` semantics; tools `SET state=5, finished_at_us=NULL` matches DDL (`:208`). Terminal rows unselectable (predicates `a.state IN (0,1)`, `t.state IN (0,1)`, `e.state=1`); `finished_at_us=NULL` is the open-state invariant, not fabricated. No inserts, no replay. PASS.
- Trigger safety: `tool_binding_immutable` fires only on binding columns (`:219`), state not listed. No aborts. PASS.
- Order: children before roots, comment `:170-171` correct; join on `e.state=1` stable inside tx (root update last). Same-pass root/child sets aligned only up to per-statement LIMIT (see residual R1).
- Rollback: all `?` returns drop uncommitted tx. `close()`/`open_memory`/fresh-init untouched. PASS.
- Fresh/memory/close compat: `recover_existing` only on nonempty path (`:35-38`); `open_memory` skips it; `close` still writes `clean_shutdown=1` (`:138-146`). PASS.
- Concurrent opens: IMMEDIATE serializes; loser blocks to busy_timeout then errors, or CAS `changed=0` on lost race (no double increment). Exactly-once per successful open. SQLite-level only; no OS owner lock (P3 caveat stands, protocol `:27-31` unimplemented). PASS scoped.
- `clean_shutdown` value read but never branched on (`_clean_shutdown`): clean vs dirty opens behave identically; dirty-store integrity checks absent. Recorded deviation, contract matrix does not require differential handling.

## Root-bounded selection: skip/strand analysis

- Roots never skipped: monotone drain; `e.owner_generation < next` + `state=1` re-selects leftovers next open. Proven by bounded-drain test (501 -> 1 -> 0/501 uncertain).
- R1 (residual, minor): child scans join `e.state=1` evaluated pre-root-update. Children beyond the per-statement 500 cap whose root converts this pass are unselectable on later passes (parent now 4, `4->1` illegal). >500 open children under converted roots strand as open. RED seeds roots without children, so untested. Fix direction (follow-up, not this lane): key child scan on child-side staleness or carry converted root pks in-tx temp set.
- R2 (by design): prior-generation queued(0)/uncertain(4) executions untouched. Matches contract rows 6 (conditional resume only) / 11 (explicit resolve only).
- R3 (open, caller-side): advanced `owner_generation` is recorded but unenforced; `ExecV2::start_execution` takes caller-supplied generation, no equality check against `workspace_state`. No production ExecV2 caller exists yet (P1), so no live hole; enforcement belongs to daemon/startup lane.
- Query plans (read-only EXPLAIN on checked-in DDL, no DB mutation, no test authored): attempts/tools `SEARCH ... USING INDEX attempts/tools_recovery_idx (state=?)` + pk join; execs `SEARCH ... USING INDEX executions_recovery_idx (owner_generation<?)`. All indexed, bounded, LIMIT-capped. Redundant `IN (0,1,4) AND =1` predicates harmless.

## Verification runs (sole Cargo slot, sequential, jobs1/threads1)

- `cargo test -p opencode-rk-storage --test app012_restart_resume_red -- --test-threads=1` -> 18 passed.
- `cargo test -p opencode-rk-storage --lib -- --test-threads=1` -> 122 passed.
- `cargo test -p opencode-rk-storage --tests -- --test-threads=1` -> 205 passed.
- Targeted: `restart` 2 passed, `schema` 6 passed, `facade` 6 passed, `execution` 18 passed (filtered counts).
- `cargo check -p opencode-rk-storage --jobs 1` -> 0 errors, 1 pre-existing warning (duplicate CLI target `oc2`/`opencode-rk` in `crates/cli/Cargo.toml`).
- `git diff --check` clean. No test edits: `929acfc..HEAD` touches only `facade.rs`, `claims.json` (GREEN row), two worklogs.
- Memory: `vm_stat` free 6028 pages + inactive 333235 pages; no parallel Cargo; no `cargo`/`rustc` survivors at end.
- `tools/lane_gate.py`: 4 UNRUN test harnesses (`test:integration_v2/import_v2/quota_v2/perf_modules_v2`), no APP012 lane registration. Pre-existing.
- `tools/convergence_gate.py`: blocked on 95 pre-existing off-plan ledger findings. Pre-existing, unchanged by this lane.

## Verdict

ACCEPT scoped to storage startup recovery only (`StorageFacade::open` fencing + bounded uncertain-scan). Residuals R1 (strand edge >500 children), R3 (generation unenforced), OS owner lock, broker conditional resume, daemon/live-turn caller wiring remain open. Parent APP-012 cannot close. Canonical guard decides integration.
