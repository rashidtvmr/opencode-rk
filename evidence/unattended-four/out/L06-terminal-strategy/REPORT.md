# L06: P03 terminal-strategy evaluation

**Verdict:** `NEEDS_FIX`, before optimizer. Strategy direction has value; lifecycle proposal is not safe as written. No candidate source approved.

**Base:** `7ceb21fb267751e1b3772f482d1673a76aa235df` (verified read-only). **OpenCode:** `450e7b92412c780a7b5a75226bb8d49aaca24e22`. **OpenTUI:** `8d6065a3e60039d16c6ee697692dcfbc46d3c786`; header SHA-256 `52190da4b586b7544d7970fe9e4e49da243f540f38b26e8419d95bc721d5e9f3`.

## Blocking findings

1. P01-v3 lacks `NativeSession::context_ptr()`. `NativeContext::check_thread` and `as_ptr` are private (`context.rs` 426-438); P08's separate `DEPENDENCY.patch` only proposes visibility changes for its `NativeContext` readback mechanism. An `extern` declaration cannot recover an owned session's inaccessible context pointer. Reject P03's Option B as described. Require a distinct P01 dependency patch with a minimal owner-thread-checked accessor returning `Result<*mut ot_context, NativeError>` (or a checked immediate-call closure); never store/send the pointer.
2. P01-v3 `NativeSession::Drop` closes if needed, ignores a failed session-state query, then cancels unless state is CLOSED/CANCELLED and destroys once (`context.rs` 656-703). Therefore P03's close/pump only avoids cancel when close succeeds, pump reports `OT_PUMP_CLOSED`, and the follow-up state query confirms CLOSED. Unconfirmed/incomplete state must not fall into implicit cancellation. Add dependency change: preserve exact query/cleanup status, no panic-prone `eprintln!`, no implicit cancel/destroy on incomplete ordinary teardown. Explicit cancel only for confirmed disconnected/failed transport.
3. Header guarantees bounded work per `ot_session_pump`, not bounded overall close: each accepted turn takes `work_budget`; WAIT_UNTIL uses monotonic deadlines; OUTPUT_PENDING requires host delivery. `pump_exit` skips cursor-settle waits but still requires output delivery/completion, and rejects failed transport (header 2697-2716, 2793-2798). Require resumable explicit `finish()` returning `Complete`, `Progress`, `Waiting(deadline)`, `Backpressure`, `TransportFailed(status)`, or `Incomplete(status)`. Never spin/sleep in Drop or promise eventual completion under permanent backpressure.
4. Only call `Complete` once `OT_PUMP_CLOSED`, and, after accepted setup, `terminal_phase == OT_TERMINAL_RESTORED`. Then restore host termios; P01 may destroy once. `close()`/`OT_SESSION_CLOSED` alone do not establish terminal restoration. Header says destroy rejects pending output or unrestored terminal unless cancelled (2807-2810).
5. Host-mode setup must be transactional: allocate/attach first; capture exact TTY attributes; arm restore ownership before the raw-mode syscall; on raw syscall failure or `setup_terminal` rejection, restore immediately. Header says setup rejection changes neither terminal state nor accepted output (2630-2640). Ensure every create/attach/setup failure has an owning cleanup path; never discard the `NativeSession` and rely on its current Drop to cancel implicitly.
6. `drain_output` callback may block as long as its writer and receives borrowed bytes synchronously; callback must not retain data, reenter mutation, report buffered-but-unwritten bytes as delivered, or unwind across the C ABI. Use a nonblocking writer; bound EINTR retry; EAGAIN/retry exhaustion -> `0`; permanent disconnect -> exact fatal status and explicit cancel. `OT_OUTPUT_FAILED` is terminal/no-replay (header 2744-2761); do not map it to a retryable render error. Preserve all native `ot_status` values.
7. `ot_terminal_flush_input` is process-wide, after restoration disables mouse reporting (header 2799-2803). Assign to one composition-root owner after all sessions restore; never call per-session Drop.

## Implementable lifecycle and rollback contract

- **Dependency prerequisite:** separate P01 seam patch, independently reviewed: checked `NativeSession` context access; exact, non-panicking cleanup diagnostics; no cancellation/destruction on state-query failure or incomplete ordinary shutdown. P08 seam is analogous only; not an applied P03 dependency.
- **Construction:** create context/session (all five `NativeSession::create` arguments: `&Rc<NativeContext>, chunk_size, span_capacity, max_bytes, control_capacity`); attach renderer mutably before TTY raw mode. Keep sole session owner throughout rollback.
- **Setup:** clone/capture TTY and original termios; arm host restore guard before raw-mode attempt; enter raw; invoke native setup. On any failure, restore host modes immediately, close/drain no-terminal session where possible, retain exact failure.
- **Active path:** each `finish`/pump turn has positive, bounded work budget and monotonic timestamp. Handle outcomes separately: AGAIN -> return progress at budget boundary; WAIT_UNTIL -> return deadline to scheduler; OUTPUT_PENDING -> FIFO nonblocking drain and exact copy-ticket completion if copy API used; no bytes/backpressure -> return pending; CLOSED -> verify terminal phase RESTORED, restore host modes, then release session for P01's one destroy.
- **Transport failure:** preserve exact status; only proven transport disconnection/permanent write failure authorizes cancel. Cancellation explicitly discards queued output; report that native escape-sequence restoration is not guaranteed, but restore host termios regardless.
- **Deadline/Drop:** bounded best effort only; no sleep, unbounded loop, blocking stdout, arbitrary callback, panic, or false `Complete`. If incomplete, record exact failure and do not invoke implicit cancel/destroy. Make `finish` retryable while owner remains live.
- **Process shutdown:** exactly one process-level flush after all terminal sessions restored; no per-session flush.

| Failure point | Native action | Cancel | Host mode |
|---|---|---:|---|
| Context/session create or attach | Return exact status; rollback pre-setup session gracefully | No | Unchanged |
| TTY capture/non-TTY refusal | Return/refuse raw setup or existing headless route | No | Unchanged |
| Raw-mode syscall attempt/failure | Guard already armed; restore immediately on failure | No | Restore |
| Native setup rejected | Roll back accepted session; no mode transition assumed | No | Restore immediately |
| Pump AGAIN/WAIT_UNTIL | Bounded step; host schedules next turn | No | Hold |
| OUTPUT_PENDING | FIFO nonblocking delivery/completion | No | Hold |
| Backpressure/no progress/deadline | Return pending/incomplete; allow explicit retry | No | Hold, restore on final exit |
| CLOSED + RESTORED | Restore host modes; P01 destroys exactly once without cancel | No | Restore |
| Proven permanent transport failure | Preserve status; explicit cancellation then destroy | Yes | Restore; native modes may remain |
| All process sessions finished | Flush queued input once at composition root | No | Already restored |

P01 Drop after the confirmed successful drain is harmless and single-shot if `closed` is set: it skips duplicate close, confirms CLOSED, skips cancel, destroys once. `NativeSession::close()` is idempotent when its local `closed` flag is set and treats native `OT_SESSION_CLOSED` as success (`context.rs` 638-653). `destroy` is not a repeatable API: it invalidates the handle/tickets on success (header 2807-2810), so P03 must not destroy itself and then allow P01 Drop to destroy again. `Option::take()` is appropriate only after the completed/restored predicate, then drop the extracted session exactly once. Current P01 Drop's ignored state-query failure and `eprintln!` panic risk must be corrected first.

If Drop's bounded fallback remains incomplete while transport is not known-failed, simply allowing `Option<NativeSession>` to auto-drop re-enters P01's cancel ladder. The P01 dependency patch must provide a deliberate non-destructive abandon/leak-on-incomplete path (or an owner registry that keeps session/context alive); on a disappearing owner, `mem::forget(session)` is the narrow fail-closed fallback, not normal cleanup. This prevents accidental cancellation of accepted output but leaks native state until process exit. On confirmed transport failure, use one explicit cancel path and mark it consumed so P01 Drop cannot cancel twice. Record exact cleanup status through a non-panicking sink.

## Evidence / verification

P01-v3 `context.rs` SHA-256: `c612a317ca4bde0d337c20b65370b1866df7386d8ca75e80da83dfcd53044ffd`. P03 combined `REPORT.md` SHA-256: `5ac7dcf6e0398324f8608ea9d7125e3ba87eb477772f742a98f199b632c6b3bb`; `PRESERVED-HANDOFF.md`: `ea1ffff3eafafa1a329ed83e1daf9d2eea506b011f360c481ec8a7ab1a53259c`. P08 v2 `DEPENDENCY.patch`: `260c0b651c215d5dd36ca666780a59a1342044e2c7e056dd5e97f8a3732614b4`; report: `3bc7d0eaa6f63dfa152593a2c6dd912d5d7c8f60228d1e2d9f22b4a42d83a4eb`.

Canonical/upstream SHA and artifact hashes inspected; source/header reads and `rg` only. No builds/tests/Cargo/PTY/GUI/DB/network/Git mutation. Canonical dirty state observed (`M AGENTS.md`, untracked `crates/cli/src/native_text_width.rs`, `docs/ui-parity/.DS_Store`) and untouched. This is static strategy review, not compilation, candidate preverification or integrated acceptance.

**Next:** deliver P01 dependency seam to separate optimizer; only then implement P03 against this matrix and request fresh independent review of exact candidate hash.
