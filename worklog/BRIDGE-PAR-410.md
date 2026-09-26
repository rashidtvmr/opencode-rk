# Task BRIDGE-PAR-410 - scratchpad (UNCLAIMED ledger status)
# Orchestrator owns tasks/completion/claims.json; proceeding file-only per delegation.

- Claim: NOT claimed (orchestrator-owned ledger, instructed not to touch).
- Source evidence: `packages/tui/src/ui/link.tsx:18-19` displayText = children ?? href; `crates/opentui-bridge/src/link_ui_full.rs:6-18` strip-scheme label + is_url prefix check.
- Observed: existing `link_ui_full::link_label(url)` single-arg strips scheme; new lane needs two-arg `link_label(url, text)` mirroring children ?? href fallback.
- Target boundary: ONE new file `crates/opentui-bridge/src/ui_link_full.rs`; no edits to lib.rs, Cargo.toml, link_ui_full.rs; no cargo, no commit.
- Tests: 4 unit tests in-file (trim+cap, is_external prefix, label fallback/cap).
- Decisions: `link_text` = trim + 256-char cap; `is_external` = http/https prefix; `link_label` = trim text, fallback trimmed url when empty, 256-char cap; std-only, forbid unsafe.
- Unknowns: none; verification rustfmt --check only.
