# V2 Native UTF-8 Live Candidate

## Package

- Package/gate: G5 Native TUI — native interactive UTF-8 input repair
- Base: `47549f06f9c5c39cd2a54f81ad6ae4866ed89596`
- Frozen contract: `tests/e2e/native_utf8_input.py`, SHA-256
  `f5a4058cc51d6d84e02523633a2601e1bfa741fe8834848353606939a7f80619`
- Status: candidate; independent verification and integration are required.

## Change

`crates/cli/src/tui_entry.rs` now assembles terminal input with a fixed four-byte
UTF-8 carry. Complete valid scalars are appended as `char`s only after bounded
validation. ASCII remains on the same draft/dialog/submission path. Backspace
pops one completed scalar, and submit, dialog switching, clearing, and other
control actions discard incomplete carry. Draft byte limits remain 32 KiB for
ordinary input and 16 KiB for API-key input; the append check is overflow-safe.

No queue, task, thread, runtime, dependency, input descriptor, masking, or
terminal cleanup behavior was changed. Invalid or incomplete sequences are
discarded without unsafe or lossy conversion.

## Upstream evidence

The pinned checkout at `95daf90670b7c039c436c85537da5fbfe2205b41` was inspected.
The native V2 source at
`packages/tui/src/component/prompt/index.tsx` implements the corresponding
complete-text behavior: `onContentChange` at lines 1377-1382 reads
`input.plainText`, stores it, forwards it to autocomplete, synchronizes
extmarks, and updates the cursor version. `submitInner` at lines 947-955 reads
and synchronizes `input.plainText` before downstream reads, explicitly covering
the final composed character. The downstream session submission text part at
lines 1104-1106 passes `inputText` as the text payload. This is classified as
implemented native V2 behavior and the Rust bridge preserves its complete-text
observable semantics.

The shared/legacy `packages/opencode/src/cli/cmd/run/footer.view.tsx` and
`footer.prompt.tsx` path was also inspected as corroborating evidence, but is
not the basis for the native V2 classification.

## Verification boundary

Source-only work was performed. No Cargo build, test, clippy, browser, PTY,
archive, or install command was run by this implementation worker. The parent
independent verifier must run the frozen absolute fixture and the required
native/G2/auth/PTY/CLI regressions on this exact candidate commit.
