# V2 Native Bracketed-Paste Contract — Candidate Handoff

Package: G5 native PTY bracketed-paste contract
Base: `0c26eab0a57c88404299d9b5d2a5f8123717f774`
Status: **CANDIDATE DONE; product acceptance PENDING**

## Scope

Only the independent fixture was repaired: `tests/e2e/native_bracketed_paste.py`.
The contract remains frozen: bracketed-paste enable/restore negotiation; fragmented
Unicode and CRLF/CR paste inert until explicit Enter; exact normalized provider
input; two settled turns and four durable messages; unterminated and oversized
paste rejection; normal CLI exit code 0; terminal restoration; daemon absence;
bounded provider-thread join; no secret echo; and bounded PTY capture.

## Mechanical repairs

- Added a fixture-local `PasteTerminalScreen` adapter.  It preserves the
  shared helper unchanged while modeling OpenTUI 0.4.5-style text cells:
  East-Asian wide glyphs occupy two cells, continuation cells are private
  non-text sentinels, combining scalars attach without advancing, and ASCII
  spaces remain ordinary visible spaces.  The existing 80x24 cursor,
  erase, bounds, and every assertion are unchanged; in particular the exact
  Japanese assertion remains `screen.contains("日本語")`, not a partial or
  whitespace-stripped substitute.
- Source-only proof: the provider/history inputs, `FIRST_EXPECTED` exact
  normalization, request/history assertions, negative unterminated and
  oversized checks, enable/restore checks, exit/cleanup checks, and all
  timeout/bound constants are unchanged.  Only screen construction is routed
  through the local width-aware adapter.

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

The prior display failure is mechanically superseded: the captured wire/render
evidence showed `> paste contract alpha café`, `日本語  line 🦀`, and
`:quit /connect stays inert`; the old helper advanced every Unicode scalar by
one column, unlike the pinned OpenCode `95daf906...` / OpenTUI `0.4.5`
text-cell behavior.  This candidate changes only fixture interpretation of
those captured bytes.  Runtime freeze and product acceptance remain pending;
the unresolved `ESC[?2004l` observation is not weakened or reclassified.
