# L06 terminal-strategy evaluation scratchpad

Updated: 2026-10-28; role: independent FFI terminal-strategy evaluator; base/canonical SHA: `7ceb21fb267751e1b3772f482d1673a76aa235df`; OpenTUI pin: `8d6065a3e60039d16c6ee697692dcfbc46d3c786`; OpenCode pin: `450e7b92412c780a7b5a75226bb8d49aaca24e22`.

Absolute source/artifact directories:
- Canonical read-only: `/Users/mymac/Projects/opencode-rk-recovery-20261005`
- Evidence: `/Users/mymac/Projects/opencode-rk/.git/recovery-evidence/ui-parity-kit-20261028`
- Owned output only: `/Users/mymac/Projects/opencode-rk/evidence/unattended-four/out/L06-terminal-strategy`

## Goal and success criteria

Validate `parallel-replenishment/P03-strategy-combined/{REPORT.md,PRESERVED-HANDOFF.md}` against exact P01-v3 `context.rs`, P08 readback dependency seam and pinned C header. Deliver safe lifecycle API and rollback matrix or reject unsafe strategy. No source writes, tests/builds, Git mutation, PTY/GUI/DB/network, nested agents.

## Evidence read

- Shared `WORKER_CONTRACT.md` lines 7-35: exact base/pins/header SHA, private-output-only grant, no builds/tests or mutable actions.
- `docs/AGENT_WORKER_PRACTICES.md` lines 27-41, 43-72: verify exact source, stay in role, cite source-specific evidence, distinguish proposal from verification/acceptance.
- P03 combined diagnostic `REPORT.md` lines 90-195, 257-292 and `PRESERVED-HANDOFF.md` lines 22-30: composable design, suggested checked context seam, Option<NativeSession> Drop, terminal I/O proposal and residuals.
- P01-v3 `context.rs` SHA-256 `c612a317ca4bde0d337c20b65370b1866df7386d8ca75e80da83dfcd53044ffd`: `NativeContext::check_thread` private (426-432), `as_ptr` private and unchecked (434-438); `NativeSession::create` five args (504-536), `handle` (539-541), `setup_terminal` (579-602), `close` idempotence (638-653), Drop cancel/destroy ladder (656-703), and `eprintln!` in Drop paths (467-478, 665-702).
- P08 `DEPENDENCY.patch` lines 1-38 exposes `NativeContext::check_thread` and `as_ptr` as separate dependency seams; P08-v2 `ot_lease_readback.rs` lines 451-467 checks owner thread before extracting pointer. P08 is a seam precedent, not a P03 `NativeSession` pointer accessor.
- Pinned header SHA-256 `52190da4b586b7544d7970fe9e4e49da243f540f38b26e8419d95bc721d5e9f3`: setup contract lines 2630-2640; bounded pump/time contract 2697-2716; output callback/blocking and failure behavior 2744-2761; shutdown/exit/cancel/destroy semantics 2788-2810.
- Existing host path `crates/opentui-bridge/src/safe_renderer.rs` lines 63-157 and 507-568: single process-level terminal input ownership/restoration; `crates/cli/src/terminal_host.rs` lines 212-249: current RawModeGuard only marks restoration, does not call termios itself.

## Findings

Confirmed unsafe/incomplete as written: P03's P01 seam suggestion lacks an actual accessor patch; P01-v3 has no `context_ptr()`. Suggested fallback Option B does not gain access to an owned `NativeSession`'s private context pointer merely by declaring externs. P01-v3 Drop unconditionally cancels any session not observed CLOSED/CANCELLED; therefore P03's promised graceful close/pump must be proven complete before its field drops. Bounded close alone leaves a timeout path that reaches this cancel ladder, violating the stated cancel-only-on-transport-failure rule. `pump_exit` is a bounded work-unit fallback, not a deliver-or-close primitive; header still requires host delivery/completion and says failed transports are rejected.

Further blockers: callbacks may block as long as writer; an EINTR retry loop can be unbounded; C callback must not let Rust panic unwind across ABI; callback user-data/bytes may not escape synchronous call. Host termios ownership needs to arm restoration before raw-mode attempt, restore immediately on every setup rejection/error, and avoid panicking/logging in Drop. P01-v3's `eprintln!` in Drop can panic on stderr write failure; cleanup errors should be retained without panic. `OT_OUTPUT_FAILED` (`-15`) is terminal/no-replay and must remain exact/fatal, never retry-looking. Close success/RESTORED alone is insufficient; pump `OT_PUMP_CLOSED` proves drained lifecycle completion. `OT_PUMP_WAIT_UNTIL` requires host wait/deadline handling outside Drop. Process-wide `ot_terminal_flush_input` belongs to composition-root teardown after every terminal session restored, never per-session Drop.

## Decision

Verdict: `NEEDS_FIX`. Accept composable P01 reuse only after separate minimal P01 dependency patch adds a `NativeSession` owner-thread-checked context accessor (or checked-call closure) and makes its Drop non-panicking and policy-compatible. Reject Option B as currently described. Require explicit bounded `finish()` as primary path; Drop performs bounded nonblocking best effort, restores host termios, never loops/sleeps or panics. Since current P01 Drop cancels when not closed, timeout cannot safely fall through; add an explicit non-cancel preservation/leak-on-incomplete path or keep the session alive for caller retry. Only exact `OT_OUTPUT_FAILED`/confirmed disconnected transport permits cancel. Never claim guaranteed native escape-sequence restoration if output is permanently backpressured or transport failed.

## Verification actually executed

Read-only source/evidence inspection, `git rev-parse HEAD` for canonical and upstream, `shasum -a 256` for pinned header and evidence artifacts, and `rg`/file reads. No build, compile, test, PTY, GUI, DB, network or Git mutation. Canonical status observed before source reads: pre-existing `M AGENTS.md`, `?? crates/cli/src/native_text_width.rs`, `?? docs/ui-parity/.DS_Store`; preserved untouched.

## Findings / decision

P03 `Option<NativeSession>` preserves composition but does not itself solve bounded shutdown. Once owner Drop returns, P01 Drop runs and can cancel; incomplete teardown needs a distinct non-destructive abandon/leak-on-incomplete path, or retained owner for retry. `ot_session_pump_exit` still needs output delivery. Drop must avoid callbacks, blocking stdout, sleeping and unbounded pumping. Require terminal phase RESTORED after CLOSED when setup accepted; only then restore host modes and allow P01 single destroy. Cancel only confirmed transport failure. `finish` returns progress/deadline/backpressure/fatal/incomplete explicitly. Flush remains process-wide owner.

Verdict: NEEDS_FIX. Require separate P01 owner-checked context seam and cleanup semantics before P03. Option B rejected as insufficient to access private pointer under NativeSession ownership.

## Verification actually executed

Read-only `git rev-parse HEAD` for canonical and OpenCode checkout; `shasum -a 256` for header and evidence; `rg`/read on cited source. No build/test/Cargo/PTY/GUI/DB/network/Git mutation. Canonical dirty state preserved.

## Artifacts / next step

- `REPORT.md`: lifecycle API and rollback matrix, evidence and limitations.
- `VERDICT.md`: blocking findings and minimal P01 dependency requirement.
- `HASHES.txt`: final artifact and evidence checksums.
- Next owner: separate P01 dependency seam author/reviewer; then P03 optimizer; fresh exact-candidate review required. No integrated acceptance claim.
