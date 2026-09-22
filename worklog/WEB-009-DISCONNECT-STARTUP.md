# WEB-009 disconnect startup handshake

## Claim
- Task: WEB-009 (disconnect cancellation)
- Session: ses_f35270c35ffehcQG3Bo2fpQ6oS
- Frozen test: crates/server/tests/runtime_wiring_disconnect_http.rs
  sha256 95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8 (immutable)
- Owned file: crates/server/src/lib.rs (only)
- HEAD: 352db15

## Source evidence
- lib.rs:931 acquire_http_turn_permit — turn permit returned on stream drop.
- lib.rs:1034-1445 create_turn_stream: stream::unfold(TurnStreamState...).
- lib.rs:1223-1299 TurnStreamStage::Executing: directly awaits ToolExecutor::execute.
- executor.rs:124-181 execute_shell: uses tokio::process::Command::output(); drop does
  NOT kill child. This is why "tool parent survived browser disconnect".
- shell_tool.rs:82-314 ShellTool: process group + kill_on_drop(true); kills on Drop.
  No start/readiness handle exposed (only async fn execute).
- turn_parts.rs:169-182 pure state-machine disconnect hook; live HTTP stream unused.

## Prior failed attempts (HANDOFF evidence)
1. Dormant directly-owned future: execute future pending after tool_call chunk;
   PID publication deadline exceeded.
2. Abort-on-drop spawned task: still PID publication deadline exceeded; no fixture
   PIDs published before deadline.

## Concrete repair design
- Replace direct `executor.execute(...).await` in Executing stage with an owned
  abort-on-drop Tokio task that runs ShellTool.
- ShellTool::execute is async fn consuming self (self-referential). Cannot poll
  from outside. Instead: inside owned task, construct ShellTool, then use
  tokio::sync::oneshot to signal successful spawn before awaiting result.
- Design: spawn tokio task that:
  1. constructs ShellTool with broker + config
  2. attempts synchronous validation + spawn via poll_fn on a pinned boxed future
     of ShellTool::execute, polling once to capture Ready error or spawn completion
  3. sends Result<(), startup error> over oneshot
  4. continues awaiting the same future to completion
- HTTP stream awaits startup receiver before yielding tool_call.
- Guard: state owns the task handle; on stream drop the handle is dropped -> abort
  drops ShellTool -> kills process group.
- Handle sender/receiver close explicitly (no panicking).
- Max calls bound enforced by existing LoopController.
- Remove debug prints / obsolete direct-future code.

## Root constraint
ShellTool::execute borrows self mutably and spawns internally. To poll it manually
we must box it as `Pin<Box<dyn Future<Output=Result<ShellResult,ShellError>>>>`.
But execute takes `mut self` (by value) and returns a future that captures the
moved ShellTool. So `Box::pin(tool.execute(config))` works: the future owns the
ShellTool and is pinned. We poll it once to reach spawn, then continue.

Actually simpler: spawn a tokio task that runs the full ShellTool::execute and
uses a oneshot to signal after the child is spawned. But execute() spawns internally
with no external spawn notification. We cannot inject a callback without editing
shell_tool.rs.

Refined: since we can ONLY edit lib.rs, we construct the child ourselves in lib.rs
via ShellTool, then... no. ShellTool::execute hides the spawn.

Alternative within lib.rs: use ShellTool::execute inside the owned task. The task
sends a onesignal AFTER a single poll that reaches spawn. But the future is opaque.

Simplest correct approach: the owned task MUST use ShellTool (process-group kill on
drop). We accept that ShellTool::execute does spawn internally synchronously-ish.
The first poll of ShellTool::execute reaches `cmd.spawn()` (line 217) before reading
stdout. So polling the boxed future once via poll_fn reaches spawn, THEN we can
signal startup via oneshot, then continue polling to completion.

Steps in owned task:
1. let mut fut = Box::pin(shell_tool.execute(config));
2. use std::future::poll_fn to poll fut once in a noop-waker context? No — we need
   real reactor for spawn. Better: poll fut within the spawned tokio task context
   using a manual poll loop:
   - poll with a waker that wakes a oneshot; on first Ready(Ok/Err) after spawn
   we cannot easily detect "spawn reached" vs "completed".

Realization: ShellTool::execute spawns synchronously within the future's first poll
up to the `cmd.spawn()` call, then continues to read stdout/stderr which are async.
So: poll fut once; if it returns Pending, spawn has happened (child is live) ->
  signal startup Ok; if Ready, signal startup with result (early return).
Then keep polling fut to completion.

But tokio::task spawn gives us an async context where poll_fn has a real waker.
We can use `tokio::task::yield_now` + poll. Actually within an async fn we can do:

```
let mut fut = Box::pin(shell_tool.execute(config));
let mut started = false;
let result = poll_fn(|cx| {
    let poll = Pin::new(&mut fut).poll(cx);
    if poll.is_pending() && !started {
        started = true;
        startup_tx.send(Ok(())).ok();  // spawn reached, child live
    }
    poll
}).await;  // but this conflates — we want to signal then continue
```

Wait, poll_fn awaits until Ready. Once it signals Ok on Pending it returns Pending
and the outer await continues polling the SAME fut via poll_fn. But poll_fn's closure
runs each poll; after signaling started=true it just returns the poll result. So:
- first poll: fut is Pending (spawn done) -> signal Ok -> return Pending
- poll_fn continues -> second poll: fut still Pending (reading) -> started already true
  -> return Pending
- eventually fut Ready -> poll_fn Ready -> done.

That works. The startup receiver in the HTTP stream path awaits `startup_rx` before
emitting tool_call. On shutdown signal (sender dropped), rx returns Err -> treat as
startup still pending / error.

## State ownership
TurnStreamState gains `tool_handle: Option<JoinHandle<...>>` or we store the
oneshot Receiver + aborter. On drop of TurnStreamState (stream cancelled), the
JoinHandle is NOT dropped by us — we must explicitly abort. Use
tokio::task::AbortHandle captured in state; drop -> abort -> aborts task -> task
drop aborts ShellTool -> kills process group.

## Plan
1. Write scratchpad (this).
2. Run frozen test RED baseline.
3. Edit lib.rs Executing stage.
4. Run frozen test -> GREEN.
5. Sequential regressions: events, policy, server stream, shell cancellation.
6. Hash, diff check, commit, push origin/lane/WEB-006-integration.

## Formatter spill cleanup
- The targeted `rustfmt --edition 2021 crates/server/src/lib.rs` invocation unexpectedly changed 146 additional tracked paths, including frozen tests.
- Restoration used a bounded script over `git diff --name-only -z 352db1526ffdaa400f96a40d2f7ec4241e68510b`; each path except `tasks/completion/claims.json` was overwritten from `git show 352db1526ffdaa400f96a40d2f7ec4241e68510b:<path>`.
- Restored paths: 146. No `git reset`, `git checkout`, `git restore`, or broad clean used.
- `crates/server/src/lib.rs` and all product/test paths now match HEAD `352db1526ffdaa400f96a40d2f7ec4241e68510b` byte-for-byte.
- Frozen test SHA-256 restored exactly: `95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8`.
- `git diff --check`: PASS. Remaining status: authorized ledger modification, this untracked scratchpad, unrelated untracked dylib only.
