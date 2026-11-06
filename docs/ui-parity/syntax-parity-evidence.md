# Syntax parity evidence: PAR-03, PAR-09, FFI-07

Evidence captured on Rust base `b66bba92607233cd175b4107b5c080b3162d48d1`.
The upstream reference is OpenCode
`450e7b92412c780a7b5a75226bb8d49aaca24e22`; the native OpenTUI library is
pinned at `8d6065a3e60039d16c6ee697692dcfbc46d3c786` in
`docs/ui-parity/opencode-ui-parity-kit/baseline.lock.json:4-18`.

This is an evidence inventory, not a parity PASS. The checklist's exact
requirements are:

* PAR-03: "Capture the real upstream and Rust production components using
  identical deterministic fixture data."
* PAR-09: "Await asynchronous syntax completion; do not bless blank transient
  code blocks as goldens."
* FFI-07: "Highlighting preserves grammar/query behavior, semantic colors,
  concealment, links and source mappings; late results cannot update
  replaced/destroyed views."

## Rust production paths

The live Markdown presentation has a bounded lexical syntax tokenizer, rather
than the upstream Tree-sitter worker. `crates/cli/src/native_view_chat.rs:2183-2190`
documents and declares the tokenizer path; `:2238-2255` emits token spans for
handled fenced code. The live fence registry maps supported language aliases
to `syntax.tokens` at `:2920-2933`, and `paint_fence_block` consumes that
handler at `:2955-2971`. The live-flow test
`md_007_handled_fence_registers_and_token_spans_reach_painted_rows` at
`:9157-9197` verifies that Rust, number, and comment token flags reach rendered
rows. The unhandled-language fallback is asserted at `:9200-9234`.

There is also `crates/opentui-bridge/src/highlight.rs`, a lifecycle/state model
for bounded results and stale-result rejection. Its module-level boundary
notes explicitly state that it does not implement Tree-sitter parsing or bind
the native text-buffer highlight calls (`:1-34`). Thus the model is not a live
upstream-compatible async highlighter, despite its useful lifecycle tests.

## Pinned upstream comparison

At `packages/tui/src/component/patch-diff.tsx:128-143`, upstream assembles
whole-file text, awaits `getTreeSitterClient().highlightOnce`, and treats
failure as no result. It slices whole-file spans to chunk-local ranges at
`:145-157`. The async callback is attached after the diff renderable creates
its code view by `queueMicrotask` at `:183-189`. The Rust tokenizer and this
upstream Tree-sitter path are materially different; the Rust bridge model does
not close that parser/worker/native-painter gap.

## Item verdicts and limits

* **PAR-03: BLOCKED / evidence obligation unmet.** Rust capture metadata rejects
  mismatched fixture identities (`crates/cli/src/native_capture.rs:385-411`),
  but this is validation of envelopes, not a paired capture from both real
  production implementations. The upstream/OpenTUI capture adapter and
  matched captures are absent. Do not mark PAR-03 passed based on the validator
  or this inventory.
* **PAR-09: PARTIAL, not accepted.** Rust's synchronous lexical tokenizer is
  live for registered fences, but there is no production async syntax result
  to await and no matched capture proving transient blank blocks are excluded
  from goldens. The checklist obligation remains unverified.
* **FFI-07: PARTIAL, not accepted.** A live *limited lexical* highlighter is
  present in the chat rendering path, and the bridge separately models stale
  result/lifecycle behavior. Full grammar/query compatibility and a live async
  native-highlighting producer/consumer are absent. In particular, concealment,
  links, source mappings, and late-result behavior have not been demonstrated
  end-to-end by the production renderer and pinned deterministic fixture.

## Required follow-up

Build a paired capture adapter that executes the pinned upstream component and
Rust production component with the same deterministic fixture and stores both
captures with fixture identity, pins, and provenance. For full FFI-07/PAR-09,
wire a real async parser/highlight producer to the renderer/native painting
path, preserve upstream's whole-source/chunk mapping and lifecycle semantics,
then exercise completion, empty/transient, replacement, and destruction orders
on the actual production view. Capture nonblank final output and await the
highlight completion before accepting parity goldens.
