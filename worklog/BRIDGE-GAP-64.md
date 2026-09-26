# BRIDGE-GAP-64: run prompt editor handoff

Claim: ses_gap64, in-progress (ledger).
Source evidence:
- TS truth: /home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/prompt.editor.ts (resolveEditorSlashValue, realignEditorPromptParts; no spawn, host-owned)
- Style ref: crates/opentui-bridge/src/editor_spawn.rs (forbid unsafe, plan-types-only, byte caps)
- Style ref: crates/opentui-bridge/src/prompt_composer.rs (MAX_VALUE truncate pattern)
Target boundary: ONE file crates/opentui-bridge/src/run_prompt_editor.rs, std-only, forbid(unsafe_code), <150 lines. No lib.rs/Cargo.toml edits, no cargo, no commit.
Tests: empty_errs, overlong_errs, key_format, truncate_cap, line_zero_ok, roundtrip (6, in-file #[cfg(test)]).
Decisions: file cap 512 chars (spec); edit cap 64KiB chars; line u32 any ok incl 0; editor_key "file:line"; finish_edit truncates by chars.
Verification: rustfmt --check <file> PASS.
Unknowns: none.
