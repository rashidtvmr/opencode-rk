# PAR-02 — Rust bridge/component API inventory

**Evidence revision:** `2c477c8afb06d6b34d53353c7686bc4c40d67c04`  
**Upstream oracle:** `/Users/mymac/Projects/tmp/opencode` at
`450e7b92412c780a7b5a75226bb8d49aaca24e22`  
**Method:** source inspection only. “Present” means the API is implemented in
the checked-out Rust source; “consumer” means a live call site was located.
No status is inferred from a filename, checklist prose, or a guessed FFI symbol.

## Bridge crate exports

`crates/opentui-bridge/src/lib.rs:1-20` declares the Rust modules and
`lib.rs:23-31` re-exports `CellBuffer`, `Editor`, `Rgba`, image types,
`MarkdownDoc`, `RendererSpec`, `Renderer`, and `ScrollBox`. The module list is
the authoritative inventory of this checkout:

| API surface | Rust source | Actual API observed | Live CLI use | Evidence status |
| --- | --- | --- | --- | --- |
| Markdown document identity | `crates/opentui-bridge/src/markdown.rs:149-237` | `MarkdownDoc::new`, `blocks`, `source`, `revision`, `unstable_count`, `set_source`; `Block::id/kind/raw/is_unstable/display_text/rows` at `:48-117` | `crates/cli/src/native_markdown_flow.rs:71-75,612-745,1031-1081` | **present + consumed** |
| Diff parser | `crates/opentui-bridge/src/diff.rs` | `FilePatch::parse` and row model | `crates/cli/src/native_diff_view.rs:419-474` | **present + consumed** |
| Terminal input | `crates/opentui-bridge/src/terminal_input.rs` | `TerminalInput` event decoder | `crates/cli/src/native_input_decoder.rs:1482-1508` | **present + parity-tested** |
| Renderer | `crates/opentui-bridge/src/renderer.rs`, `safe_renderer.rs` | `Renderer::render_once`, safe renderer boundary | `crates/cli/src/tui_entry.rs:19433-19438` | **present + consumed** |
| Frame capture | `crates/cli/src/native_capture.rs:136-170` | `Frame::new`, `Frame::paint`, cursor/hit-target fields | capture adapters consume the canonical frame schema | **present; consumer traced in capture code** |
| Buffer/layout/scroll | `buffer.rs`, `layout.rs`, `scroll.rs` | module APIs are declared by `lib.rs:1-20` | no Markdown-specific consumer found in the inspected CLI path | **present; no Markdown consumer claimed** |
| Highlight/image/editor/scene/shapes/world | corresponding modules listed at `lib.rs:1-20` | modules exist in this checkout | no claim of a live parity consumer without a call-site trace | **present; consumer not established here** |

## Markdown identity mapping

The upstream TUI configures the Markdown component in
`packages/tui/src/routes/session/message-parts.tsx:183-196`: `content`,
`streaming`, `internalBlockMode="top-level"`, grid table options, concealment,
and theme foreground. Its plugin renderer memo is
`packages/tui/src/plugin/markdown.ts:5-17`; the effective handler map is
last-wins and shallow-equal.

The Rust mapping is concrete: `MarkdownFlow::set_content` and
`push_chunk` are at `native_markdown_flow.rs:736-755`; `rebuild` calls the
bridge producer at `:1031-1039`; `match_doc_id` maps parsed flow tokens to
bridge block IDs at `:1218-1236`; and the live chat Markdown path constructs,
configures, and parses the flow at `native_view_chat.rs:3047-3079`. The bridge
contract itself says unchanged kind+raw prefixes retain IDs
(`markdown.rs:191-227`).

## Explicit non-claims

The bridge module inventory does **not** establish an OpenTUI native Markdown
FFI entry point. `markdown.rs:15-20` records that the shipped native library
has no Markdown symbol; the Rust document model is therefore the producer for
identity/display projection, while inline styling and fence consumers remain
in the CLI flow. A future capability-table update must preserve this distinction.
