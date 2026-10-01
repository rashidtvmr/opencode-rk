# V2 Native Bracketed-Paste Contract — Candidate Handoff

Package: G5 native PTY bracketed-paste contract
Base: `edd84569b5c49254148c37861164b5fb5c06be91` (declared parent base `75ad`)
Status: **CANDIDATE DONE; product acceptance PENDING**

## Scope

Only the independent fixture was repaired: `tests/e2e/native_bracketed_paste.py`.
The contract remains frozen: bracketed-paste enable/restore negotiation; fragmented
Unicode and CRLF/CR paste inert until explicit Enter; exact normalized provider
input; two settled turns and four durable messages; unterminated and oversized
paste rejection; normal CLI exit code 0; terminal restoration; daemon absence;
bounded provider-thread join; no secret echo; and bounded PTY capture.

## Mechanical repairs

- Oversized framed input drains PTY output concurrently with bounded writes, avoiding
  producer/renderer PTY backpressure deadlock while retaining the strict 32 KiB
  payload cap.
- First and second responses now require canonical full-history persistence after
  the visible assistant response, rather than treating a streamed delta as settled.
- Normal Ctrl-C exit drains all bounded trailing PTY output before evaluating
  `ESC[?2004l`; macOS `EIO` is treated as terminal EOF. Capture flags therefore
  represent the actual completed exit, and termios is checked afterward.

## Evidence and limits

The source was inspected with AST parsing and `git diff --check`; no Cargo, PTY,
native, container, network, or runtime test was executed by this worker. The
fixture intentionally remains RED until product behavior exists. Pinned upstream
and current requirements are recorded in the fixture docstring and convergence
contracts (`PLAN.md`, `docs/CONVERGENCE.md`, `docs/TDD.md`, `docs/SECURITY.md`).
The existing `edd8456` fixture history is preserved; no product code, frozen
helper, Cargo file, unrelated test, user database, or secret was touched.

## Verification manifest

Required verifier command (integration owner):

```text
python3 -m py_compile tests/e2e/native_bracketed_paste.py
python3 tests/e2e/native_bracketed_paste.py --self-check
git diff --check
```

Candidate source hash and exact integrated acceptance SHA must be recorded by the
trusted verifier. This candidate must not be marked ACCEPTED on the worker branch.
