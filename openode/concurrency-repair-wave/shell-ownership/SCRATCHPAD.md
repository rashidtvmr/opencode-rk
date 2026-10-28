# shell-ownership-research: exact source state machine for child spawn/publication/cancel/shutdown

Updated: 2026-10-26; role: shell process ownership researcher; session: ses_e86a76bb4ffdbQ9dD6NoLUIv9M
Base/candidate SHA: f83a4b183d4e4144fc1a5b7bd9776a8bede6377a (canonical main-v2 HEAD)
Upstream SHA: 450e7b92412c780a7b5a75226bb8d49aaca24e22 (/Users/mymac/Projects/tmp/opencode)
Canonical STRICTREADONLY: /Users/mymac/Projects/opencode-rk-recovery-20261005
Canonical source: crates/server/src/lib.rs, crates/server/src/run_owner.rs, crates/server/src/sdk_spawns.rs, crates/tools/src/executor.rs, crates/tools/src/shell_tool.rs
Prior evidence consulted: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/audit61-shell/AUDIT.md (v1 auditor), atomic-spawn-v2/ (empty), atomic-spawn-v3/SCRATCHPAD.md (v3 negative), atomic-spawn-v4/PATCH.diff + SCRATCHPAD.md (v4 UNEXECUTED candidate)

## Goal and success criteria

Produce the exact source-level state machine for shell child spawn, ownership publication, cancellation, and shutdown in the canonical Rust tree. Identify the minimal invariant test schedule (no-orphan, no-unapproved-spawn) that must hold for a reviewer. Compare the latest private v4 candidate (UNEXECUTED) against the current canonical without reimplementing it. Write only to the sibling private scratchpad dir. No Cargo/PTY/network/DB/git mutation/children spawned.

## Evidence read (file, symbol/line, revision)

### Canonical Rust spawn-to-cancel state machine

#### 1. Turn admission -> RunOwner (run_owner.rs:77-101)
```rust
pub struct RunOwner {
    pub session_id: SessionId,
    pub run_id: u64,
    cancellation: watch::Sender<bool>,       // line 80 — cancellation signal
    explicit_cancel: watch::Sender<bool>,     // line 81 — explicit cancel signal
    finished: watch::Sender<bool>,            // line 82 — turn finished
    pending: Mutex<Option<PendingPermission>>, // line 83 — pending human approval
    effect_gate: Mutex<()>,                   // line 84 — serializes begin_effect check vs cancel()
}
```
- `effect_gate` (Mutex<()>) is the ONLY mechanism that linearizes `begin_effect()` (the cancellation check) vs `cancel()`.
- `begin_effect()` (run_owner.rs:114-117): locks effect_gate, checks `!is_cancelled()`, returns guard.
- `cancel()` (run_owner.rs:123-130): sets cancellation watch to true, clears pending. Does NOT lock effect_gate.
- `cancel_explicit()` (run_owner.rs:132-135): sets explicit_cancel watch, then calls cancel().
- `TurnExecutionPermit::acquire` (run_owner.rs:400-438): semaphore + ACTIVE_TURNS registry, creates Arc<RunOwner>.
- `TurnExecutionPermit::drop` (run_owner.rs:445-459): calls owner.finish() -> cancel() + finished broadcast, removes from ACTIVE_TURNS.

#### 2. Native shell stream entry point (lib.rs:878-935)
`native_shell_stream` handler:
- Acquires TurnExecutionPermit (lib.rs:884) — creates RunOwner.
- Builds ToolExecutor with broker + cwd (lib.rs:889-892).
- Spawns a tokio task (lib.rs:935) that:
  1. Calls `execute_native_tool(...)` (lib.rs:945).
  2. The tokio task owns the permit via `let _permit = permit;` — permit dropped when task ends.
- Returns an HTTP NDJSON streaming response with `NativeShellReceiver` wrapper (lib.rs:955-969).

#### 3. NativeShellReceiver::drop = cancellation trigger (lib.rs:868-880)
```rust
struct NativeShellReceiver {
    receiver: tokio::sync::mpsc::Receiver<Result<Bytes, Infallible>>,
    owner: Arc<RunOwner>,
}
impl Drop for NativeShellReceiver {
    fn drop(&mut self) {
        self.owner.cancel();  // line 879 — cancel the turn on stream drop
    }
}
```
- When HTTP client disconnects OR server drops the response stream, `NativeShellReceiver::drop` fires: calls `owner.cancel()`.
- This sets the cancellation watch to true, which ShellTool's `select!` races.

#### 4. execute_native_tool — the TOCTOU window (lib.rs:1889-2012)
For shell commands ("bash"|"shell"), the non-file path:
```rust
// line 1983-1989 (CONFIRMED TOCTOU):
let Some(_effect) = owner.begin_effect() else {
    return failed("tool cancelled".to_owned(), true);
};
// comment line 1987: "a blocking mutex guard must never cross an async process wait"
drop(_effect);  // <-- GUARD DROPPED HERE (line 1989)
let call_id = call.tool_id.clone();
let executor = executor.clone()
    .with_cancellation(owner.cancellation_receiver());  // line 1991-1992
let execution = async {
    match approval {
        Some(approval) => {
            executor.execute_human_approved(call, approval, ...).await  // line 1998
        }
        None => executor.execute(call).await,  // line 2001
    }
};
let result = execution.await;  // line 2003
```
**Critical state machine gap**: `begin_effect()` returns a MutexGuard on `effect_gate`. The guard is dropped at line 1989 BEFORE the executor.async calls ShellTool::execute(). Between drop and spawn:
1. Another task calls `owner.cancel()` (e.g. NativeShellReceiver::drop on client disconnect).
2. cancel() sets cancellation watch = true.
3. ShellTool::execute() runs, checks cancellation at line 203-208 (non-atomic watch::Receiver::borrow), passes if cancel fires between check and `cmd.spawn()` at shell_tool.rs:270.
4. Child process spawned AFTER cancellation was acknowledged to the HTTP layer.

#### 5. ShellTool::execute — spawn + cancellation race (shell_tool.rs:195-346)
```rust
// line 203-208: early cancel check (watch::Receiver::borrow — non-atomic w.r.t. spawn)
if self.cancellation.as_ref().is_some_and(|receiver| *receiver.borrow()) {
    return Err(ShellError::Cancelled);
}
// line 210-244: broker authorization (may involve async/human approval)
// line 270: actual cmd.spawn() — tokio::process::Command::spawn
let child = cmd.spawn().map_err(|e| ShellError::Spawn(e.to_string()))?;
// line 267: kill_on_drop(true) set on Command — child killed if ShellTool drops
// line 278-310: tokio::select! biased { cancelled, timeout, completion }
```
- The `select!` at line 304-309 races cancellation ONLY during the wait phase (child.wait()), NOT during the spawn call itself (line 270).
- `kill_on_drop(true)` (shell_tool.rs:267) means: if ShellTool is dropped while child exists, tokio kills the child. This is the orphan-prevention backstop, but it relies on ShellTool being dropped — if the tokio task at lib.rs:935 continues to completion, the child is NOT orphaned but IS spawned post-cancel.

#### 6. ShellTool::cancel + Drop (shell_tool.rs:348-396)
```rust
pub fn cancel(&mut self) {         // line 349
    self.kill_group();
    if let Some(child) = self.child.as_mut() {
        let _ = child.start_kill();  // line 352
    }
}
impl Drop for ShellTool {          // line 390
    fn drop(&mut self) {
        self.cancel();  // line 393 — kills child group + child on drop
        self.child = None;
    }
}
```

#### 7. File operations — correct pattern (lib.rs:1956-1985 for read; 2336-2348 for write)
File operations use `spawn_blocking` with `begin_effect` GUARD HELD across the blocking call:
```rust
// lib.rs:1961-1968 (read):
let result = tokio::task::spawn_blocking(move || {
    let Some(_effect) = owner.begin_effect() else {  // GUARD HELD inside blocking thread
        return None;
    };
    ... // file operation
}).await;
```
```rust
// lib.rs:2336-2338 (write):
let Some(_effect_guard) = owner.begin_effect() else {
    return outcome("write cancelled".to_owned(), true);
};
// _effect_guard NOT explicitly dropped before .await — held across file I/O
```
- These are safe: begin_effect guard held across the actual I/O. No TOCTOU window.
- Write path guard held across `.await` (lib.rs:2339) — safe because file I/O is not a process spawn; the guard serializes against cancel() via the watch check. No deadlock risk because cancel() does NOT lock effect_gate (confirmed: cancel() only sets the watch, line 128).

#### 8. sdk_spawns.rs — separate module, NOT used by shell path
sdk_spawns.rs is the SDK-002 backend spawner (Server/Process/Tui backends) with its own SpawnPort trait, ChildHandle kill-on-drop, MAX_SPAWNS=8 cap. This is NOT invoked by native_shell_stream or ShellTool — it's a separate code path for spawning OpenCode backend daemons. Not relevant to the shell tool TOCTOU.

#### 9. Upstream OpenCode V2 — scope-bound spawn (shell.ts:264-452, cross-spawn-spawner.ts:288-319)
```typescript
// shell.ts:306-307 — spawn is inside Effect.scoped:
runFork(
  Effect.scoped(           // <-- scope controls lifecycle
    Effect.gen(function* () {
      const handle = yield* environment.spawner.spawn(...)  // line 309-318
      // ... stream setup ...
      yield* Deferred.succeed(ready, command)  // line 443 — resolves before any await
      yield* Deferred.await(command.done)      // line 446 — holds scope open until exit
    }).pipe(
      Effect.catchTag("AppProcessError", (error) => Deferred.fail(ready, error)),
    ),
  ),
)

// cross-spawn-spawner.ts:288-319 — launchProcess:
const launchProcess = (command, opts) =>
  Effect.callback<Spawned>((resume) => {
    const proc = launch(command.command, command.args, opts)  // line 292 — spawn
    // ... proc.on("spawn", () => { resume(Effect.succeed(...)) }) — line 310-315
    // ... proc.on("exit"/"close") handlers
    return Effect.sync(() => { proc.kill("SIGTERM") })  // line 316-318 — release
  })

// cross-spawn-spawner.ts:442-450 — acquireRelease wraps spawn:
const [proc, closed, exited, stopOutput] = yield* Effect.acquireRelease(
  spawn(command, opts),       // <-- scope-bound: spawn is the acquire
  Effect.fnUntraced(function* ([proc, closed, exited, stopOutput]) {
    // release: kill+reap on scope close
    ...
    yield* Effect.ignore(stop(command, proc, closed, stopOutput, {...}))
  }),
)
```
**Upstream's no-orphan contract**: `Effect.acquireRelease` ensures the spawned process handle is scope-bound. When the scope closes (due to cancellation/interruption), the release function runs `stop()` which kills the process group + reaps. The spawn happens entirely within the scope's acquire phase, so **cancellation before/during spawn prevents the process from ever being detached** — Effect's interruption mechanism fires before `launch()` executes. `ready` resolves (line 443) only AFTER the spawn callback's `proc.on("spawn")` fires (line 310), but the handle `proc` is already scope-bound by `acquireRelease`.

### Lock ordering and deadlock analysis

Locks in canonical: `effect_gate` (Mutex<()>), `pending` (Mutex<Option<PendingPermission>>), spawn_mutex (NOT YET PRESENT).

Current canonical lock graph:
- `begin_effect()`: locks `effect_gate` only.
- `cancel()`: sets cancellation watch (no lock), then `clear_pending()` locks `pending` only.
- `NativeShellReceiver::drop` -> `cancel()` -> sets watch + `clear_pending()` (locks `pending`).
- File-op path: `begin_effect()` inside spawn_blocking closure (locks `effect_gate`).
- **NO path locks two mutexes simultaneously.** No deadlock risk in current canonical.

The v4 candidate adds `spawn_mutex` (line 48 of run_owner.rs) and requires:
- `acquire_spawn()` locks `spawn_mutex` only.
- `cancel()` locks `spawn_mutex` (NOT `effect_gate`) around the watch commit.
- `clear_pending()` locks `pending` only.
- **V4 lock graph is also acyclic**: `spawn_mutex`, `effect_gate`, `pending` — no nesting.

### Deadlock proof for v4 (from v3 scratchpad, confirmed in v4 patch)
- `lib.rs:2341` holds `effect_gate` (`_effect_guard`) across `file_tool.execute_human_approved().await`.
- `NativeShellReceiver::drop` (lib.rs:879) calls `owner.cancel()`.
- If `cancel()` locked `effect_gate`, it would deadlock with the holder at lib.rs:2341 (same thread cannot re-lock a std Mutex).
- V4 correctly makes `cancel()` lock `spawn_mutex` instead — which is NOT held by `begin_effect` or `execute_human_approved`. No deadlock.
- `spawn_mutex` is never nested with `effect_gate` or `pending`. ACYCLIC. CONFIRMED SAFE.

## Findings (confirmed / hypothesis / refuted)

### CONFIRMED: TOCTOU on shell-spawn cancellation gate
- **Defect**: `begin_effect()` (lib.rs:1983) acquires `effect_gate` MutexGuard, then `drop(_effect)` at line 1989 releases it BEFORE `ShellTool::execute()` reaches `cmd.spawn()` (shell_tool.rs:270).
- **Window**: drop(_effect) @ line 1989 -> ShellTool cancel check @ shell_tool.rs:203 -> cmd.spawn() @ shell_tool.rs:270. During this window:
  1. HTTP client disconnects -> `NativeShellReceiver::drop` -> `owner.cancel()` sets cancellation watch = true.
  2. ShellTool's non-atomic `receiver.borrow()` check at line 203-208 may pass (read before cancel committed).
  3. `cmd.spawn()` at line 270 creates child process AFTER cancellation acknowledged.
  4. Child process is orphaned: the tokio task at lib.rs:935 continues executing ShellTool::execute, spawns child, but the HTTP client has disconnected.
- **Root cause**: The guard must never cross an async process wait (existing comment at lib.rs:1987). But dropping it before spawn creates the TOCTOU window.

### CONFIRMED: ShellTool's own cancellation check is non-atomic
- shell_tool.rs:203-208 checks `*receiver.borrow()` — a `watch::Receiver<bool>` borrow. This is a point-in-time read, not atomic w.r.t. `cancel()`'s `send_replace(true)`.
- The `select!` at line 304 only covers the wait phase (child.wait()), not the spawn call (line 270).

### CONFIRMED: Upstream V2 has no TOCTOU — scope-bound spawn
- Upstream uses `Effect.acquireRelease` (cross-spawn-spawner.ts:442) wrapping `spawn()`. The spawned process handle is scope-bound: scope close = kill+reap.
- `Effect.scoped` (shell.ts:307) wraps the entire spawn+wait lifecycle. Cancellation interrupts the fiber BEFORE `launch()` (cross-spawn-spawner.ts:292) executes.
- `ready` resolves (shell.ts:443) only after `proc.on("spawn")` (line 310), but the handle is already scope-bound.
- No window between cancellation-check and spawn: Effect's interruption mechanism IS the synchronization.

### CONFIRMED: File-op paths are correct (no TOCTOU)
- File operations (read lib.rs:1956-1968, write lib.rs:2336-2348) use `spawn_blocking` with `begin_effect()` GUARD HELD across the blocking I/O call. No drop-then-spawn gap. These are safe.
- Write path holds `_effect_guard` across `.await` (lib.rs:2339) — safe because file I/O is not a process spawn; the guard serializes against `cancel()` via the watch check. No deadlock: `cancel()` does NOT lock `effect_gate` (line 128).

### CONFIRMED: v4 candidate is UNEXECUTED
- grep for `acquire_spawn|AcquireSpawnGuard|spawn_mutex|spawn_fn` in canonical crates/ returns NOTHING.
- v4 exists only as a patch + scratchpad in `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/audit61-shell/atomic-spawn-v4/`.
- v4 design: per-instance child slot + mutex-based serialization. `spawn_mutex` on RunOwner, separate from `effect_gate`. `cancel()` locks `spawn_mutex` around watch commit. `ShellTool` gets a `spawn_callback` that acquires `spawn_mutex` + checks `is_cancelled()` atomically before `cmd.spawn()`.

### CONFIRMED: Upstream vs canonical divergence is structural
- Upstream: `Effect.acquireRelease` + `Effect.scoped` = spawn is scope-bound. Cancellation interrupts the Effect fiber before `launch()`. No manual lock needed.
- Canonical: manual `Mutex<()>` + `watch::Receiver<bool>`. Spawns via `tokio::process::Command::spawn()` (sync) inside `ShellTool::execute`. Cancellation via watch channel. The manual approach requires a lock-serialized check-then-spawn to match upstream's atomicity.

## Decisions and rationale

The TOCTOU is real and confirmed at the source level. The exact state machine:

```
BEGIN_EFFECT (lib.rs:1983)
  -> locks effect_gate
  -> checks !is_cancelled() (watch borrow)
  -> returns MutexGuard<effect_gate>
DROP(_effect) (lib.rs:1989)  <-- TOCTOU WINDOW BEGINS
  -> unlocks effect_gate
  -> cancel() can now lock nothing and set watch=true
SHELLTOOL.execute (executor.rs -> shell_tool.rs:195)
  -> checks cancellation (non-atomic borrow, line 203-208)  <-- may pass
  -> broker authorization (async, lines 213-244)
  -> cmd.spawn() (line 270)  <-- CHILD CREATED POST-CANCEL
  -> select! { cancelled, timeout, completion } (line 304-309)  <-- races wait only
END

CANCEL PATH:
HTTP client disconnect
  -> NativeShellReceiver::drop (lib.rs:872-880)
  -> owner.cancel() (run_owner.rs:123-130)
  -> cancellation.send_replace(true)  <-- sets watch
  -> clear_pending()  (locks pending, not effect_gate)
```

The v4 candidate correctly closes the window by adding `spawn_mutex` to serialize `cancel()`'s watch-commit against `cmd.spawn()` + child publication. The guard is acquired immediately before `spawn()` and released before the async wait — matching upstream's scope-bound acquire-release semantics in Rust.

## Alternatives rejected and why

1. **Hold `effect_gate` across spawn**: Would deadlock with file-op path (lib.rs:2341 holds effect_gate across await; NativeShellReceiver::drop calls cancel() on disconnect). V4 correctly uses a separate `spawn_mutex`.

2. **Re-check cancellation atomically before spawn (watch flag)**: Reduces but does not eliminate window — `spawn()` is a sync syscall but the watch borrow + spawn sequence is still non-atomic. V4's mutex serialization is the correct fix.

3. **Reuse `ShellBounds` module (shell_bounds.rs)**: ShellBounds uses `std::process::Command` + `std::thread` readers, different async model from ShellTool's `tokio::process::Command`. Larger change. V4's inline callback is more surgical.

## Verification actually executed
NONE — researcher role is read-only. No builds, tests, PTY, network, DB, or git mutation.

## Minimal invariant test schedule for reviewer

### Test 1: No-orphan on pre-cancel (no child spawned post-cancel)
- **Setup**: RunOwner, call `owner.cancel()`, then invoke ShellTool with spawn_callback.
- **Assert**: `ShellError::Cancelled` returned. An `AtomicBool` set inside the spawn callback (if it executes `cmd.spawn`) remains false. No child process created.
- **Invariant**: `no-unapproved-spawn` — cancellation before spawn prevents child creation.

### Test 2: No-orphan on concurrent cancel (barrier race — cancel-first)
- **Setup**: RunOwner + Barrier(2). Thread A: barrier.wait(), then `acquire_spawn()` + check `is_cancelled()`. Thread B: barrier.wait(), sleep 10us, then `owner.cancel()`.
- **Assert**: Thread A observes `is_cancelled() = true` after acquiring spawn_mutex. No child published.
- **Invariant**: `no-orphan` — cancel racing spawn always wins.

### Test 3: No-orphan on concurrent cancel (barrier race — spawn-first)
- **Setup**: RunOwner + Barrier(2). Thread A: barrier.wait(), `acquire_spawn()`, check `!is_cancelled()`, publish child (set AtomicBool), drop guard. Thread B: barrier.wait(), sleep 5ms, then `owner.cancel()`.
- **Assert**: Thread A completes (spawn saw no cancel). Child published. Cancel still eventually commits.
- **Invariant**: `no-orphan` — spawn racing cancel always observes the pre-cancel state correctly.

### Test 4: Poison terminal (fail-closed after panic)
- **Setup**: Set `SPAWN_LOCK_POISON = 1`. Attempt `acquire_spawn()`.
- **Assert**: Returns `Err(())`. ShellTool returns `ShellError::Cancelled`. No child spawned.
- **Invariant**: `no-unapproved-spawn` — poison blocks all spawns.

### Test 5: Deadlock safety (cancel during file-op await)
- **Setup**: Hold `effect_guard` across an await (simulating lib.rs:2341 file op). Call `owner.cancel()` from another thread.
- **Assert**: `cancel()` does NOT block (locks `spawn_mutex`, not `effect_gate`). Completes within timeout.
- **Invariant**: `no-deadlock` — cancellation path never locks a mutex held across an await.

### Test 6: Normal completion marks no explicit cancellation
- **Setup**: `TurnExecutionPermit::acquire` → `owner.finish()`.
- **Assert**: `explicit_cancel` watch stays false (existing test at run_owner.rs:657-662, preserved).

## Artifacts and hashes
- Scratchpad: /Users/mymac/Projects/opencode-rk-recovery-20261005/openode/concurrency-repair-wave/shell-ownership/SCRATCHPAD.md
- Canonical source SHA (HEAD): dd0c16d7cb8632c0ba92b65fa9e62e5050576376 (f83a4b18 ancestor)
- Upstream pin: 450e7b92412c780a7b5a75226bb8d49aaca24e22 (/Users/mymac/Projects/tmp/opencode)
- v4 candidate (UNEXECUTED): /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/audit61-shell/atomic-spawn-v4/
- v3 negative analysis: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/audit61-shell/atomic-spawn-v3/SCRATCHPAD.md
- v1 auditor report: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/audit61-shell/AUDIT.md

## Remaining uncertainties, blockers and next steps
- The v4 candidate is UNEXECUTED — no compilation or test verification performed. The patch exists only as a diff.
- The v4 design's `spawn_callback` signature (`Fn(&mut Command, &mut Option<Child>, &mut Option<u32>) -> Result<(), ShellError>`) requires ShellExecutor to pass `Arc<RunOwner>` into the builder chain. The full wiring diff (executor.rs + lib.rs changes) is not in the PATCH.diff — only run_owner.rs and shell_tool.rs are patched.
- For implementation: integrate v4 design (spawn_mutex + acquire_spawn + spawn_callback) into the three files (run_owner.rs, shell_tool.rs, executor.rs) + wire in lib.rs:1983-1989 area. The minimal change: replace `drop(_effect)` at lib.rs:1989 with a spawn_mutex-guarded callback passed through ShellTool.
- Telemetry: UNKNOWN — no provider usage data queried.