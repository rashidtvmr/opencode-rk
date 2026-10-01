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

## Width-contract audit (mechanical review)

The local adapter must not classify East Asian Width `A` as two cells merely from
Python's label. The pinned OpenTUI source at linked checkout
`c01292fd0837bafd07ce458c74416b2b375a41ab` (native library object
`798f30dd7f4fbe36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`) proves the
actual contract: `packages/native/src/utf8.zig`, `eawToWidth`, returns 2 only for
East Asian Width `.fullwidth` or `.wide` and returns 1 in the ordinary fallback.
`GraphemeWidthState.addCodepoint` separately promotes a non-zero grapheme only
for `WidthMethod.unicode_wide`; that explicit profile is not the default
`unicode` profile used by this xterm fixture. Therefore `W/F => 2` and `A => 1`.

The private `_width` helper follows that source (`"WF"`, not `"WFA"`). All
provider texts, frozen assertions, combining and real-space behavior, erase
semantics, limits, negative contracts, and cleanup remain unchanged. No runtime,
PTY, Cargo, build, or network command was run by this worker.
