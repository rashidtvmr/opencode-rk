# BRIDGE-PAR-432

Status: unclaimed (orchestrator owns claims.json; file-only lane, no claim attempted).
Task: ONE new file `crates/opentui-bridge/src/dialog_prompt_tsx_full.rs`.
Scope: no edits to lib.rs, Cargo.toml, dialog_prompt_full.rs. No cargo, no commit.
Truth: `packages/tui/src/ui/dialog-prompt.tsx:9` DialogPromptProps, value :13, confirm :28-31.
Sibling pattern: `dialog_tsx_full.rs` trim+take(128); `dialog_prompt_full.rs` char-count caps.
Deliverable: PromptTsx {label cap 128, value cap 512} + set_value + value_of + is_empty.
Decision: truncating setters (ponytail: no Result errs; add when caller needs rejection).
Tests: 3 (label trim/cap, value cap 512, empty default).
Verify: rustfmt --check only.
