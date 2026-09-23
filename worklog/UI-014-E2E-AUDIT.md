# UI-014 E2E audit

## Claim

- Task: `UI-014`
- Session: `ses_f33322bacffeKBSM2WcJPS8mHa`
- Scope: one standalone caller-boundary test, this scratchpad, own ledger row.
- Candidate revision: `9ded2b901b2376a4a5ee1815a41c7a1234ece157`

## Source evidence

- `crates/cli/src/main.rs:224-268`, `run`: no-subcommand computes the launch plan and, for `LaunchMode::NativeTui`, calls `tui_entry::run_with_dir` at lines `242-255` when native mode is selected.
- `crates/cli/src/tui_entry.rs:564-680`, `native_interactive_loop`: native path owns a plain `String` draft, reads one byte at a time from `stdin`, handles only raw `\r`/`\n` as submit, and directly clears/submits the draft. It has no `native_composer::Composer`, `ShiftEnter`, `CtrlJ`, queue, or interrupt-draft path.
- `crates/cli/src/tui_entry.rs:827-887`, `run_with_dir`: the `native` feature dispatches the real `native_interactive_loop`; the compatibility line loop is only under `not(feature = "native")`.
- `crates/cli/src/native_composer.rs:438-616`, `Composer`: actual native composer owns multiline edit state, keymap routing, bounded busy queue, interrupt preservation, and `finish_turn`; this module is not called by `native_interactive_loop`.
- `crates/cli/src/native_composer.rs:127-147`, `decide_key`: `ShiftEnter` is newline for both keymaps; `CtrlJ` is submit only for `SubmitKeymap::CtrlJ`.

## Observable target

The real native caller must route physical input through the native composer, not a parallel `String` state:

1. Enter submits a non-empty draft.
2. Shift+Enter inserts a newline without submitting.
3. Ctrl+J inserts a newline under the Enter keymap and submits under the Ctrl+J keymap.
4. Interrupt clears in-flight status while retaining the draft and queued drafts.
5. A busy turn queues drafts FIFO under the composer byte/count bounds.

The no-subcommand launch test also pins the native caller reachability already present at `main.rs:237-255`.

## RED test

- Owned test: `crates/cli/tests/ui014_native_caller.rs`
- Method: standalone `rustc --edition 2021 --test`; reads the checked-in `main.rs` and `tui_entry.rs` with `include_str!`, then asserts semantic caller wiring. No mocked product API, no copied composer implementation, no existing-test edits.
- Expected RED: entrypoint reachability passes; native caller composer-routing tests fail because the actual caller still uses the byte/String fallback.
- RED command: `rustc --edition 2021 --test crates/cli/tests/ui014_native_caller.rs -o /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/ui014_native_caller && /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/ui014_native_caller --test-threads=1`
- RED result: compiles; `1 passed; 4 failed`; failures are T02 Enter/composer routing, T03 Shift+Enter, T04 Ctrl+J, T05 interrupt/queue. No product code or existing tests edited.
- Frozen test hash: `5338e5cf3bb2d8bb2616a9115fdd11ec5e520df344bc7d5e7710727c412c80bc` (`sha256sum crates/cli/tests/ui014_native_caller.rs`).
- Required minimal production prewire before independent behavioral RED: expose a testable native input adapter at the caller boundary, for example
  `pub(crate) fn native_input_event(byte_or_terminal_event: TerminalEvent) -> Option<crate::native_composer::Key>`
  plus a caller-owned
  `pub(crate) fn native_composer_step(composer: &mut crate::native_composer::Composer, key: crate::native_composer::Key) -> Result<crate::native_composer::KeyHandled, crate::native_composer::ComposerError>`.
  The live loop must call this adapter for decoded terminal events and route `Submitted`/`Queued`/`Edited`; interrupt and turn completion must call `Composer::interrupt`/`finish_turn`. A renderer-independent adapter is required because current `native_interactive_loop` hardcodes `stdin`, `NativeRenderer`, and a private function body.

## Remaining blocker

No production edits authorized. Current internals do not expose a renderer-independent callable native input boundary, and current native caller does not wire `native_composer`. Lane remains blocked after RED as required; integrator must prewire the adapter and native caller before a behavioral E2E can run against the real loop.
