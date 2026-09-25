# BRIDGE-PAR-347 scratchpad (UNCLAIMED - ledger overflow, orchestrator owns ledger)

Claim: skipped per task override. No touch of tasks/completion/claims.json.
Source evidence:
- TS truth: packages/tui/src/editor-zed.ts:197-199 (isZedTerminal env probe), editor.ts:26-35 (openEditor argv from $VISUAL||$EDITOR).
- Existing Rust: crates/opentui-bridge/src/editor_zed.rs:78-83 (is_zed_terminal), crates/opentui-bridge/src/editor_spawn.rs (plan-only, no spawn).
Observed: sibling editor_zed.rs owns terminal probe + ZedRequest; editor_spawn.rs owns spawn plans. New file must not duplicate; owns only argv builder + label + availability stub.
Target boundary: ONE file crates/opentui-bridge/src/editor_zed_full.rs. No lib.rs, no Cargo.toml edits. No cargo test/commit.
Tests: 3 (cmd_shape, cmd_line_passthrough, label_and_stub).
Decisions:
- zed_cmd returns Vec<String> ["zed", path, line], cap 3 (not path:line form; noted as ponytail upgrade).
- is_zed_available false stub documented (host probes PATH; lib does no exec).
- zed_label static str "zed".
Remaining: rustfmt --check result pending; integration wiring is orchestrator job.
