# BRIDGE-GAP-29

Claim: `BRIDGE-GAP-29`, session `ses_gap29`.

Source evidence:
- `crates/opentui-bridge/src/prompt_composer.rs:84-88` defines submit gate: non-empty buffer and not busy.
- `crates/opentui-bridge/src/keymap_resolve.rs:76-93` resolves a binding to an optional action string; no composer key classification exists.
- User-cited upstream `packages/tui/src/component/prompt/index.tsx:405-417,826-857,961-964` is unavailable in this checkout; no `packages/` source tree exists. Action names are therefore the explicit task contract: `submit`, `newline`, `interrupt`, `shell`, `quit`, `q`.

Boundary: std-only pure classifier. No composer mutation, I/O, I/O lifetimes, or caller wiring. Unknown actions fail closed. `quit` and `q` produce `Quit` only for an empty buffer; non-empty `q`/`quit` remain ordinary input.

Tests: standalone `rustc --edition 2021 --test`; no Cargo, `lib.rs`, commit, or external test edits.
