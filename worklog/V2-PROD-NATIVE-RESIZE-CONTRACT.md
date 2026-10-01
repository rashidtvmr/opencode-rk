# V2 production native resize contract (NATIVE-RESIZE)

## Candidate scope

This independent test-owner artifact covers G5's missing production observation:
an installed release native CLI receives a real kernel PTY geometry change while
idle and repaints using the new bounds. It is source-only preparation from base
`0bf0274ad863a7a60fd6631042871ec3c8016256`; no product implementation is
claimed here.

The test starts a fresh release artifact with a loopback fixture provider, sends
exactly two normal turns, then performs `TIOCSWINSZ` from 80x24 to 60x8 without
input or a provider request. The bounded post-ioctl transcript must contain a
fresh full repaint whose explicit CUP/HVP cursor rows never exceed 8, while a
session/composer surface remains observable. It also asserts that the fixture
received exactly two requests. Cleanup must reap the CLI, stop only the validated
owned daemon, join the provider thread, and close the PTY.

## Authority and source evidence

- Pinned upstream authority is OpenCode `95daf90670b7c039c436c85537da5fbfe2205b41`
  (`sources/upstream.lock.json`, lines 7-14). The pinned checkout is not vendored
  in this worktree, so this contract does not claim upstream internals.
- Current native entrypoint `crates/cli/src/tui_entry.rs`, symbols
  `native_size` and `native_loop` (lines 530-565, 598-716), currently samples
  `COLUMNS`/`LINES` once and paints with `renderer.rows()`; it has no observed
  terminal geometry query or resize event path.
- Current native bridge `crates/opentui-bridge/src/safe_renderer.rs`, symbol
  `NativeRenderer::resize` (lines 603-616), exposes safe native resizing, but
  its availability is not evidence that the CLI wires kernel resize events.
- Current host/app contracts already model `HostEvent::Resize` and dirty-frame
  handling (`crates/cli/src/native_host.rs`, lines 229-240 and 333-340;
  `crates/cli/src/native_app.rs`, lines 579-585). These are current native
  abstractions, not proof of the installed end-to-end path.
- Existing `tests/e2e/native_stream_tool.py` provides the real release artifact,
  authenticated loopback Responses fixture, bounded PTY capture, and owned
  process cleanup patterns reused by this test. Existing tests establish input,
  UTF-8, escape and streaming behavior; they do not establish live resize.

## Oracle and limitations

The oracle deliberately observes terminal output rather than renderer internals:
the kernel resize ioctl must lead to a fresh full repaint, and all explicit
addressable rows in that repaint must fit the new eight-row PTY. This detects a
renderer left at its original 24-row geometry, while avoiding invented APIs.
The provider request count proves resize did not become accidental input.

This is not a claim of complete OpenTUI parity: it does not assert pixel layout,
SIGWINCH implementation details, mouse/clipboard behavior, or a specific header
placement at tiny sizes. The current fixed native layout may require a follow-up
engineering decision if the approved product contract demands a minimum-height
composer policy below eight rows. The test is intentionally a candidate and must
be run by the parent against the later integrated artifact to establish genuine
RED/GREEN evidence.

## Verification status

No Cargo build, runtime, HTTP, PTY, provider, or heavy command was run in this
source-preparation lane by instruction. The allowed changes are exactly
`tests/e2e/native_resize.py` and this worklog. Status: **CANDIDATE / not frozen
RED / not PREVERIFIED / not ACCEPTED** until the trusted parent runs the release
artifact command and records the exact integrated SHA.
