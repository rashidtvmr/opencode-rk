# WEB-006-READINESS scratchpad

Claim: WEB-006 claimed by `ses_f37eafa15ffehrOnUvpgLV5tnT`, scratchpad `worklog/WEB-006-READINESS.md`, ledger `in-progress` at claim time. Prior RED lane left row `not-started` with `reclaimedBy ses_f3c4de578ffelQv59xDXmOs03B`; this session claimed after that reclaim. No collision encountered.
Ownership: exactly one product file `crates/cli/src/main.rs`; plus this scratchpad and own ledger row. Frozen readiness test and all existing tests immutable. No daemon.rs/daemon_client.rs/chat.rs/Cargo/schema/controller/verifier/policy/foreign edits.

## HEAD and source evidence

HEAD `434f438df53b66d16298456318026d4a09cee201` (lane branch `lane/WEB-006-readiness`).
Worktree `/Users/mymac/Projects/opencode-rk-web006-readiness`, isolated from dirty primary worktree.

- `crates/cli/src/main.rs:684-711` (post-edit): `serve` AlreadyRunning branch calls `wait_for_owner_descriptor` instead of one-shot `read_backend_descriptor`.
- `crates/cli/src/main.rs:639-663` (post-edit): private async `wait_for_owner_descriptor`, monotonic 2000ms budget, 25ms `tokio::time::sleep` poll, `Ok(Some)` returns immediately, `Ok(None)` retries to deadline, `Err` propagates fail-closed, `Ok(None)` only at deadline.
- `crates/server/src/daemon.rs:376-417` `read_backend_descriptor`: `Ok(None)` on missing/stale-pid/schema-mismatch/non-loopback origin; `Err` on symlink/foreign-owner/oversize/empty-or-malformed token.
- `crates/server/src/daemon.rs:63-87` `PidLock::acquire`: real advisory singleton lock; `AlreadyRunning` decides the wait path.
- `crates/server/src/daemon.rs:538-561` `publish_backend_descriptor_with_auth`: owner publication the waiter observes.
- `crates/cli/src/main.rs:664-682` `web()`: unchanged one-shot reuse; no unconditional startup delay introduced.

## Observable contract / failure / lifetime / resource bounds

- Contract: while the real PID lock is held by another process, second packaged `opencode-rk serve --listen 127.0.0.1:0` waits boundedly for a valid authenticated `BackendDescriptor`, then exits 0 printing exactly the owner origin, binding no second listener.
- Retryable to deadline: transient absence (`Ok(None)`, e.g. owner not yet published, stale pid, schema mismatch, non-loopback origin).
- Fail-closed immediately: `Err` (symlink, foreign owner, oversize, empty or malformed token). Proven by disposable fail-closed probe: exit nonzero in 0.05-0.10s, stderr names the descriptor refusal, stdout prints no origin.
- Deadline: at 2000ms returns the existing explicit error `backend is already running but its endpoint descriptor is unavailable`. Frozen test overall bound is 3s; the wait fits inside it.
- Lifetime: waiter holds no lock, spawns no poller task, binds no socket; it only reads `backend.json` then prints and exits. Detach-on-exit and explicit shutdown semantics unchanged.
- Resources: at most ~80 bounded 8KiB descriptor reads; no unbounded loop, no detached task, no spawned poller, no shell command, no thread sleep in async path (`tokio::time::sleep`).
- Security: bearer/auth preserved; `oc2`/`opencode-rk` identity, data-dir isolation, loopback-only origin, 8KiB cap, symlink/owner gates all inherited from `read_backend_descriptor`; validated descriptor origin only is printed/opened.

## Tests

- Frozen `crates/cli/tests/web_startup_readiness.rs` SHA-256 `7fad25153f8f08b554541b34c78cf237912e40ee89b97b3c5a2723a57180320a`, unchanged before and after implementation (verified multiple times, including post-edit).
- RED reproduced before implementation via disposable byte-identical server-only harness (cargo integration target unbuildable, see OpenTUI blocker): `rustc --edition 2021 --test crates/cli/tests/web_startup_readiness.rs` linked against `target/debug/deps` server rlibs, run with absolute `OPENCODE_RK_TEST_BIN`; failing assertion `second caller exited during publication window` at test line 145, confirming one-shot AlreadyRunning read.
- Implementation: only `crates/cli/src/main.rs` (`BackendDescriptor` import, `wait_for_owner_descriptor` helper, AlreadyRunning branch wiring). Doc correction applied without behavior change: schema mismatch is `Ok(None)` retryable, not `Err` fail-closed; helper and call-site comments now state the exact `Ok(None)` vs `Err` partition per `daemon.rs:376-417`.
- Zero test edits: no frozen or existing test file modified.

## Commands and results (all in `/Users/mymac/Projects/opencode-rk-web006-readiness`)

- `CARGO_BUILD_JOBS=1 cargo build -p opencode-rk-cli --bin opencode-rk`: exit 0 (468 pre-existing warnings), before and after implementation.
- `CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-cli --bin opencode-rk`: exit 0, 48.15s, 468 pre-existing warnings, zero errors.
- Frozen readiness via disposable harness with absolute `OPENCODE_RK_TEST_BIN=$PWD/target/debug/opencode-rk`: `1 passed; 0 failed`, finished ~3.01s, multiple runs including final post-doc-edit run GREEN.
- Fail-closed probe (disposable `/private/var/folders/.../T/opencode/rk/failclosed_probe.rs`, argv-only spawn, real `PidLock` owner, legacy no-token descriptor, exact cleanup via TempData Drop): `1 passed; 0 failed`, finished 0.05-0.10s. Probe asserts nonzero exit, elapsed under 1500ms, stderr contains `descriptor`, stdout prints no origin. Probe file lives outside the repo; no test or product file edited for it.
- Standalone `web_entrypoint` (byte-identical copy with `CARGO_BIN_EXE` replaced by `OPENCODE_RK_TEST_BIN`, repo file untouched): `1 passed; 0 failed` (~0.09-0.13s).
- Standalone `web_singleton_runtime` (same harness technique): FAILED at line 151 as before: bearer-less POST to `/api/sessions` returns 401 `missing bearer credential`. Pre-existing obsolete-test blocker (WEB-006.md BLOCKER-A); auth not weakened. Recorded separately, not caused by this change (identical failure on unmodified tree per prior lane evidence).
- Resource/cleanup: no `opencode-rk serve` processes remain (`ps` count 0); no `/tmp/opencode-rk-web006-*` or fail-closed fixture dirs remain; frozen fixture RAII joins publisher/health threads; probe TempData removes its dir on drop.

## Decisions

- Wait budget 2000ms with 25ms poll: fits frozen 3s bound with margin for spawn/exit overhead; immediate return on `Some` keeps the common reuse path fast; `tokio::time::sleep` because `serve` is async.
- `Err` propagates via `?` instead of mapping to deadline error: preserves fail-closed bearer/owner/symlink/malformed-token/schema behavior and keeps the existing unavailable message exclusively for genuine absence at deadline.
- No `web()` change: contract says reuse the same helper only where it does not wait when no owner is known; `web()` has no `AlreadyRunning` signal, so adding a wait there would introduce unconditional startup delay. Frozen RED owns only the serve branch.

## Remaining unknowns / unrelated blockers (no acceptance claim)

- Cargo integration-test target (`cargo test -p opencode-rk-cli --test ...`) remains unbuildable on this host: `opencode-rk-opentui-bridge` build script fails, native libopentui artifact missing for `aarch64-apple-darwin`, expected `libopentui.a` or `libopentui.dylib` under `crates/opentui-bridge/native/lib/aarch64-apple-darwin`. Exact stderr captured. Worked around with real-binary build plus disposable rustc harnesses; no product or test edit for it.
- `crates/cli/tests/web_singleton_runtime.rs` bearer-less POST expectation is obsolete vs the RC-01-hardened authenticated router (recorded in `worklog/WEB-006.md` BLOCKER-A). Test-owning integrator must update it to send the published bearer. This lane did not touch it.
