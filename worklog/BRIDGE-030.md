# BRIDGE-030 editor_zed

Claim: Zed external-editor contract.
Evidence: opencode/editor-zed.ts:187-199, context/editor.ts:121, editor.ts:26-27 @ a0d9b6c.
Scenario: open file[:line] in Zed; fall back VISUAL||EDITOR; wait flag.
Boundary: pure/std only, no spawn; caller spawns `zed --wait <loc>`.
Tests: zed_path_carry, bad_path_errs, line_zero_errs, env_fallback.
Decisions: line u32>=1; ZED_TERM/TERM_PROGRAM probe; wait_flag Zed only.
Unknowns: none.
