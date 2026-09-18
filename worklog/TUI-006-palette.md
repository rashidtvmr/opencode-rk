# TUI-006 palette — worklog

## Claim
`crates/cli/src/native_palette.rs`: searchable command registry executing
real `CommandAction`s, model/effort selection with text encode/decode for
restart survival, independent main/child effort, theme enum + no-color
fallback + WCAG contrast helper, gated-command explainer. Std only,
`#![forbid(unsafe_code)]`.

## Source evidence
- HEAD 5af7884 (`git rev-parse HEAD`).
- Card: `python3 tools/completion_plan.py --card TUI-006` → journey
  "Choose models/providers/effort, invoke commands and change
  themes/settings from keyboard", tests T01–T05, path
  `crates/cli/src/native_palette.rs`.
- Prior file (303 lines) at same path: registry/query/execute/gate/selection
  existed, but persistence test used `serde_json` (external crate), no
  `Theme` contrast helper, no `CommandGate::explain`, id match was
  case-sensitive, `register` had no bound signal.
- Prior art pattern: `crates/cli/src/native_composer.rs:1-80` (pure state,
  bounded buffers, `Display` explainers, std only, `rustc --test` clean).
- Wiring: `crates/cli/src/main.rs:27` declares `mod native_palette;` (owned
  file is new/untracked; no other files touched).

## Observed scenario
- Old file under required verifier
  `rustc --edition 2021 --test crates/cli/src/native_palette.rs`:
  `error[E0433]: cannot find ... serde_json` ×2 → missing-behavior
  evidence (persistence untestable std-only; RED-equivalent: encode/decode,
  explainer, contrast helper absent).
- New file: `rustc --edition 2021 --test ... -o /tmp/opencode/np` clean,
  `/tmp/opencode/np` → 9 passed, 0 failed.

## Target boundary
- Owned file only: `crates/cli/src/native_palette.rs` (787 lines).
- No edits to `main.rs`, other crates, docs, or controller state.
- No rendering/IO/threads; permission checks stay with broker (palette only
  refuses gated ids).

## Tests (in-file `mod tests`, frozen as written)
1. `search_filters_by_title_and_id` — title/id case-insensitive filter,
   empty query lists all, no-match empty.
2. `execute_returns_registered_action_not_a_label` — real action dispatch,
   unknown id refused.
3. `disabled_commands_explain_constraint_and_cannot_bypass` — gate equality,
   explainer carries constraint, gated entries still listed for explanation.
4. `model_selection_persists_marker_through_encode` — marker round trip incl.
   separator/newline/backslash escapes, empty/overlong rejected, clear→none.
5. `main_and_child_effort_stay_independent` — incl. across encode/decode.
6. `no_color_fallback_stays_readable` — requires_palette + is_readable +
   fallback marker.
7. `contrast_helper_accepts_readable_pairs_rejects_flat` — black/white >20,
   identical/low-delta rejected.
8. `registry_and_results_stay_bounded` — MAX_COMMANDS cap with bool signal,
   MAX_RESULTS cap, query truncation.
9. `decode_rejects_garbage_without_touching_state` — bad version/values/
   missing/unknown keys rejected.

## Decisions
- Line-based `encode`/`decode` (version marker + key=value + `\`/`\n`
  escapes) instead of serde: std-only requirement + `rustc --test`
  verifier forbid external crates. Strict decode: any malformed input is
  `Err`, caller keeps current state.
- `register` returns `bool` (false = over cap, entry dropped, order stable)
  instead of silent void.
- `CommandGate::explain` + `Display` mirrors `ComposerError` pattern.
- WCAG luminance formula with `MIN_CONTRAST = 4.5`; `NoColor::is_readable`
  always true (user terminal contrast by construction).
- ponytail: NoColor fg marker grey; upgrade to terminal-query when lane owns
  host detection.

## Remaining unknowns
- None in-slice. Integration (engine dispatch of `CommandAction`,
  persistence file location, theme rendering) belongs to integrator/other
  lanes — not claimed here.
