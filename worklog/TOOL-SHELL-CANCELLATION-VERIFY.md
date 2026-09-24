# TOOL-SHELL-CANCELLATION-VERIFY

## Claim
- Task: `TOOL-SHELL-CANCELLATION-VERIFY`, type verification, role independent Tokio process-lifecycle verifier.
- Session: `ses_f2b90ba47ffe31lk54so1DWZci`.
- Branch: `verify/TOOL-SHELL-CANCELLATION`. Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-tool-shell-cancellation`.
- Base/head: `56f8e2b3d9bd1f50481d7c8b0ee9fc0a13b662d7`. Seam source under review: `1fcd3838e2a849eadfa4220c3d298750e99c52b3`.
- Accepted contract: `c630f3f` (`worklog/TOOL-SHELL-CANCEL-CONTRACT.md`, corrected, C1-C5 accepted).
- Candidate suite (NOT RED, NOT FROZEN): `crates/tools/tests/phase1_shell_cancellation.rs` at `56f8e2b`.
- Owned file: this worklog only. No source/test edits.

## Authority and evidence read
- `docs/TDD.md`, `docs/SECURITY.md`, `.agents/WORKER.md`, `AGENTS.md` budget/lane rules.
- Contract `c630f3f` via `38593f3:worklog/TOOL-SHELL-CANCEL-CONTRACT.md` (672 lines): normative seam/RED ordering, public types, watch-cancellation semantics, cwd identity, biased precedence, bounds, Unix-only, Drop best-effort.
- Seam worklog `worklog/TOOL-AUTHORIZED-PROCESS-SEAM.md` (c15c09f scope).
- Seam verifier `2699dc7:worklog/TOOL-AUTHORIZED-PROCESS-SEAM-VERIFY.md` (BLOCKED sole gap: lossy cwd bytes; all else passed incl. deleted-probe evidence for invalid inputs, startup-timeout 1ns breach, env_clear, grant replay, error bounds).
- Cwd repair verifier `1c81b4c:worklog/TOOL-PROCESS-CWD-BYTES-GREEN-VERIFY.md` (ACCEPT seam 1fcd3838 for RED authoring; raw_os_bytes fix).
- Candidate RED worklog `worklog/TOOL-SHELL-CANCELLATION-RED.md` (NOT RED/NOT FROZEN, 9/9 GREEN, abort assertion removed per best-effort Drop rule).
- Source audit (prior lane, reused): `crates/tools/src/executor.rs` types `:51-145`, `raw_os_bytes` `:200-208`, `prepare_canonical_cwd` `:211-249`, limits/request validation `:252-368`, `cleanup_process` group-kill/child-kill/wait/readers `:466-537`, `execute_authorized_process` `:731-1098` (pre-auth cancel `:768`, exact intent+baseline/grant gates `:779-811`, pre-spawn recheck `:815-823`, exact spawn+process_group(0)+kill_on_drop `:827-843`, startup window `:869`, pid0 `:897`, one readiness `:919`, readiness latch `:944`, biased child/cancel/deadline loop `:971-1007`, Exited Reaped kills-NotRequired `:1036-1069`, Cancelled/TimedOut full cleanup `:1070-1092`, non-Unix UnsupportedPlatform `:741-755`). No `Drop` impl on seam; `kill_on_drop(true)` last-resort only.

## Verdict
**ACCEPT for Unix cancellation behavior only**, bounded as below. Source plus candidate suite jointly prove the deterministic cancellation contract on Unix. This verdict explicitly does NOT authorize Windows support, server/registry wiring, parent/release acceptance, or any frozen-RED claim.

## Noncompliant historical RED ordering (mandatory note)
- The process violated the planned RED ordering. Contract c630f3f required: seam, independent seam verification, then a compiling behavioral RED that fails for missing behavior.
- The seam prerequisite (c15c09f, repaired 1fcd3838) overimplemented the deterministic cancellation behavior before any cancellation RED was authored, so the intended RED lane found no missing deterministic behavior: candidate suite compiled and ran 9/9 GREEN against accepted seam revision 1fcd3838.
- No RED failure was fabricated. The candidate suite is therefore recorded as NOT RED and NOT FROZEN. It is verification evidence, not a frozen RED artifact. No test was edited to obtain GREEN; the owner-abort delayed-side-effect assertion was removed before running because contract c630f3f `:422,:564` defines dropped/aborted ownership as best-effort only and an assertion of deterministic reap without awaiting completion is not valid behavior to demand.

## Command evidence (sequential, sole Cargo lane)
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test phase1_shell_cancellation -- --test-threads=1` -> `9 passed; 0 failed; finished in 0.25s`. Exit 0.
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=2` -> `3 passed; 0 failed`. Exit 0.
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools --test process_cwd_bytes_red -- --test-threads=1` -> `1 passed; 0 failed`. Exit 0.
- `CARGO_BUILD_JOBS=2 cargo check -p opencode-rk-tools` -> 0 errors, 3 pre-existing warnings (`shell_tool.rs` dead code, unused imports); none in `executor.rs` seam. Exit 0.
- Hashes (`shasum -a 256`): candidate `44c228804107b65451c04fbbad9e93d14f5494605c2bcab40ad740c8aafc6827`; broker `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8` unchanged; cwd `f4eea500a6aedffd6d6a93b4289bf449fabfa73ed3c0e2b1db8ec707c3d65aa7` unchanged.
- `git diff --check` -> exit 0. No source/test modifications by this lane.
- Survivors: `pgrep -f "fixture.sh|sleep 30"` empty; no `sentinel/marker/cancelled/phase1` residue under TMPDIR. `kill: <pid>: No such process` lines in output are bounded Fixture::drop probes against already-reaped children, not leaks.
- Memory: `vm_stat` pages free 20736 x16KiB (~324MB) + inactive ~5.7GB reclaimable; single sequential Cargo lane, no pressure, 2GiB OS headroom floor intact.

## Nine-scenario assertion audit (all real public-API behavior, no mocks)
1. Pre-start cancel (`cancellation_already_true...`): `CancelledBeforeStart` + `CleanupStatus::NotRequired`; `authorizer.audit_len()==0` and `broker.audit_len()==0` prove no grant burn; `ready_rx` error proves no readiness; pid/marker absence. STRONG. Cosmetic: request uses `/bin/sleep`, fixture pid paths vacuous; no-spawn proof rests on audit counters, which is the correct signal.
2. Post-ready group reap (`cancellation_after_readiness...`): exactly one readiness (`ready.pid>0`, owner-did-not-complete-first select), bounded pid publication, cancel sent while owner alive, `Cancelled` + `Reaped`, `assert_dead(parent,descendant)` via `kill -0` poll, 100ms settle then no sentinel. STRONGEST: proves owned group+descendant death and no late marker.
3. Timeout reap (`timeout_reaps...`): 100ms deadline, readiness observed, `TimedOut` + `Reaped`, `process_group==pid`, both dead, no sentinel. STRONG.
4. Readiness-receiver drop (`dropped_readiness...`): typed `ReadinessReceiverClosed{pid}`, `!is_alive(pid)`, no sentinel. STRONG: proves bounded cleanup precedes typed error, never ordinary success.
5. Bounded dual output (`output_cap...`): exact `stdout=="0123\n[truncated]"`, flag true, total len `<= cap + marker`. STRONG: proves retained-prefix bound, drain (exact 10-byte fixture against 4-byte cap), single marker outside cap.
6. Closed false sender (`closed_false...`): sender dropped with value false -> `Exited` + `Reaped` + readiness ok. STRONG: proves closed receiver is "no signal", not cancellation, per contract `:329-337`.
7. Deterministic precedence (`completed_child_wins...`): marker-observed exit then late cancel -> `Exited`. ADEQUATE: proves latched terminal cannot be rewritten; narrow residual race (marker write vs process exit) bounded by short-lived fixture; source biased child-first order (`:971-988`) guarantees exit-wins when wait already resolved.
8. Normal completion (`owner_completion...`): exact `Reaped{NotRequired,NotRequired,Succeeded(wait),Succeeded(readers)}` + readiness ok. STRONG: exact-step match, kills correctly NotRequired with no latched termination.
9. Deny/HumanRequired pre-spawn (`broker_deny...`): typed `Denied` / `HumanRequired`, no readiness, no marker. ADEQUATE with note: fixture ignores argv so `!marker.exists()` is vacuous; real no-spawn proof is typed pre-spawn error + no readiness + source ordering (deny returns `:786-796` before spawn `:846`). Compensated, not hidden.

## Invalid-abort and Drop audit
- `grep abort` over candidate suite: NO_ABORT_TEST. Invalid owner-abort deterministic-reap test is absent, as required.
- `grep Drop` over seam: no `Drop` impl on process owner. `kill_on_drop(true)` (`:842`) retained only as last-resort signal; deterministic cleanup requires awaiting owner future. Contract `:520-522` honored. Fixture `Drop` uses best-effort `/bin/kill -KILL`, test-side only.

## Residual gaps (bounded, non-blocking for this verdict)
- In-flight pre-readiness cancel (borrow latch `:944-969`) has no dedicated runtime test; covered by source audit + post-readiness proof of same cleanup path.
- Cancel/timeout tie ("cancellation wins") not runtime-forced with both observable before one poll; covered by source audit of biased branch order (`:971-1006`).
- Spawn-failure, invalid limits/request, startup-timeout breach, env_clear, grant replay/single-use, error-byte bounds: not in 9-test candidate but exercised by prior seam verifier deleted probes (2699dc7) and frozen cwd RED (1/1); hashes confirm no regression.
- No unrelated-process-survival probe in candidate; cleanup scope is source-audited (`kill_process_group` on owned group only, `:479-490`).
- Windows: explicitly unsupported (`UnsupportedPlatform`, `#![cfg(unix)]` suite). No server/registry wiring claimed. Stale `execute_success`/`execute_timeout` lib tests untouched, untouched by design.

## Landing
- `git diff --check` clean; working tree holds only this worklog (new) and own ledger row.
- Commit/push/fetch verification recorded in handoff message; remote==HEAD required before handback.
