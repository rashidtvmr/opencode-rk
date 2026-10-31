# Verdict: NEEDS_FIX

P03's composable strategy is directionally better than the original patch, but is **not safe to hand to a new optimizer unchanged**. Require a separate, reviewed P01 dependency patch first. Reject the described Option B fallback. No implementation or verification candidate reviewed.

Blockers:

1. P01-v3 has no `NativeSession::context_ptr()`. `NativeContext::as_ptr()` and `check_thread()` are private. A P03 `extern` declaration resolves a native symbol, not the live private context pointer. Option B works only by reimplementing context/session ownership, not by declaring externs alongside an owned `NativeSession`.
2. Normal close/pump-to-closed can avoid P01 Drop's cancel only after P03 called `NativeSession::close()`, pump reports `OT_PUMP_CLOSED`, and `NativeSession::Drop` successfully reads `OT_SESSION_CLOSED_STATE`. P01 then skips cancel and destroys once. Current P01 Drop ignores a failed `ot_session_get_state`; its default `OPEN` can cause an unintended cancel. It also uses `eprintln!` on cleanup paths. Require a separate P01 change: preserve exact cleanup status without panic; cancel only through an explicit transport-failure path; do not destroy/cancel an unconfirmed incomplete session.
3. Explicit `finish(&mut self, ...)` must be incremental and budgeted. Return `Waiting(deadline)`, `Backpressure`, or `Incomplete` instead of sleeping/spinning. Never return success until pump is `OT_PUMP_CLOSED` and, if setup was accepted, terminal phase is `OT_TERMINAL_RESTORED`. `OT_PUMP_OUTPUT_PENDING` needs ordered output delivery and completion. `OT_PUMP_WAIT_UNTIL` belongs to the host scheduler. A caller deadline cannot guarantee restored escape sequences if output stalls.
4. Drop fallback must be finite, nonblocking, panic-free, and callback-free (or use only a strictly bounded nonblocking writer). Restore host termios on every path. Do not claim native terminal restoration on incomplete Drop; preserve/report incomplete state rather than silently cancelling queued output. `ot_session_pump_exit` is a bounded process-exit work unit, not a bypass for output delivery.
5. Build/attach the session before entering raw mode. Install the host restore guard before the first raw `tcsetattr`, not after it. Setup rejection contract says native state/output are unchanged; immediately restore original host modes, then gracefully dispose the no-terminal session. Create/attach failures happen before raw mode. A raw-mode `tcsetattr` error still needs a best-effort original-mode restore.
6. Drain callback must never block, panic across C ABI, reenter mutation, retain borrowed bytes/user-data, or misreport buffered bytes as delivered. Bound `EINTR` retries; `EAGAIN`/retry exhaustion returns `0`; only a proven permanent write failure or native `OT_OUTPUT_FAILED` permits cancellation. Preserve exact native `status`; never flatten terminal `-15` into retryable render failure.
7. `ot_terminal_flush_input` is process-wide. One composition-root owner calls once only after all sessions have restored modes and disabled mouse reporting. Never flush from per-session Drop.

## Required separate dependency patch

Against exact P01 candidate hash `c612a317ca4bde0d337c20b65370b1866df7386d8ca75e80da83dfcd53044ffd`, add only the required lifecycle seams before P03 implementation:

- `NativeSession::context_ptr(&self) -> Result<*mut ot_context, NativeError>`: run `self.context.check_thread()?` before returning `self.context.as_ptr()`. Document pointer use as immediate same-thread FFI only; never store or send it.
- Expose cleanup-error recording to P03 (`record_cleanup_error` currently private), or provide a typed non-panicking recording method. Remove `eprintln!` from P01 Drop paths.
- Correct P01 Drop to never infer transport failure from failed state query and never cancel an unclosed session implicitly. Provide an explicit cancel-on-confirmed-transport-failure method/state; on incomplete ordinary teardown, record exact failure and avoid destructive cancel/destroy. Keep the existing single owner of successful exactly-once destroy.

The P08 candidate's `DEPENDENCY.patch` exposes `NativeContext::check_thread`/`as_ptr` and cleanup recording for readback (P08 `DEPENDENCY.patch` lines 4-38), but is separate candidate evidence, not an integrated P03 accessor. Compose overlapping P01 visibility changes in one reviewed dependency patch.

## Lifecycle API / rollback matrix

| Stage/outcome | Required action | Cancel? | Host termios |
|---|---|---|---|
| Context/session creation fails | Return exact `NativeError`; no raw mode entered | No | Unchanged |
| Renderer attach fails | Keep local session owned; graceful close/pump/destroy without setup; preserve exact status | No | Unchanged |
| TTY capture fails / stdin not a TTY | Refuse interactive raw setup or take existing headless path | No | Unchanged |
| Raw `tcsetattr` attempt | Guard already owns descriptor + original attributes; arm before syscall | No | Restore immediately on failure |
| `setup_terminal` rejected | Header promises rejected admission changes neither native terminal state nor accepted output; close/pump no-terminal session | No | Restore original immediately |
| Setup accepted | Record setup-owned state; terminal transition remains asynchronous | No | Hold until native restoration complete |
| `finish` sees `AGAIN` | Consume only remaining bounded work budget, then return progress | No | Hold |
| `OUTPUT_PENDING` | Deliver FIFO through nonblocking synchronous writer; complete exact ticket if using copy path; zero bytes means backpressure | No | Hold |
| `WAIT_UNTIL` | Return deadline; host schedules next bounded turn; continue required regular polls | No | Hold |
| `IDLE` or no progress | Return `Pending`/`Incomplete`; do not spin | No | Hold until caller chooses exit fallback |
| `OT_PUMP_CLOSED` | Require `terminal_phase == RESTORED` for accepted setup; then restore host termios; let P01 Drop destroy once | No | Restore |
| Exact permanent transport failure / `OT_OUTPUT_FAILED` | Preserve exact status; mark transport failure; explicit cancel then one destroy | Yes, only here | Restore, but native escape-sequence restoration is not guaranteed |
| Deadline exhausted or Drop before completion | One finite, nonblocking best-effort fallback; record exact cleanup state; no implicit cancel/destroy; do not claim native restore | No | Always attempt restoration; surface failure in cleanup slot |
| All process sessions done | Composition root calls process-wide input flush once | No | Already restored |

`finish` should be resumable and return `Complete`, `Progress`, `Waiting(deadline)`, `Backpressure`, `TransportFailed(status)`, or `Incomplete(status)`. Keep raw FFI status distinct from pump result enum. Drop should not call `drain_stdout` (header permits blocking stdout), sleep, loop until completion, invoke arbitrary writer code, or panic. If a narrowly bounded nonblocking Drop pump step is used, it must not claim success unless the same closed/restored conditions hold.

## P01 Drop after successful P03 drain

When P03 uses P01 `close()`, reaches `OT_PUMP_CLOSED`, observes `OT_TERMINAL_RESTORED`, then drops the sole `Option<NativeSession>` once, current intended P01 behavior is: `closed == true` skips another `ot_session_close`; state query returns `OT_SESSION_CLOSED_STATE`; cancel is skipped; `ot_session_destroy` runs once. Destroy must remain owned by P01. This is safe only on the confirmed-success path. Current ignored state-query errors make the fallback unsafe; the dependency patch above must fix it.

## Evidence and limitations

- Base verified read-only: `7ceb21fb267751e1b3772f482d1673a76aa235df`.
- Upstream checkout verified: `/Users/mymac/Projects/tmp/opencode` at `450e7b92412c780a7b5a75226bb8d49aaca24e22`.
- Header path: `/Users/mymac/Projects/opencode-rk/.git/recovery-evidence/ui-parity-kit-20261028/native-provenance-river/opentui.h-at-8d6065a3-0.6.8.h`; SHA-256 `52190da4b586b7544d7970fe9e4e49da243f540f38b26e8419d95bc721d5e9f3`.
- P03 report SHA-256 `5ac7dcf6e0398324f8608ea9d7125e3ba87eb477772f742a98f199b632c6b3bb`; handoff SHA-256 `ea1ffff3eafafa1a329ed83e1daf9d2eea506b011f360c481ec8a7ab1a53259c`.
- Read P01 `context.rs` lines 426-438, 504-541, 579-653, 656-703; P08 `DEPENDENCY.patch` lines 4-38; header lines 2630-2658, 2697-2716, 2744-2761, 2788-2810.
- Static source/evidence inspection only. No builds, tests, Cargo, PTY, GUI, DB, network or Git mutation. No candidate acceptance.
