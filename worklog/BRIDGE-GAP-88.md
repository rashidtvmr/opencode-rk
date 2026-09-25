# BRIDGE-GAP-88 scratchpad

claim: BRIDGE-GAP-88 ses_gap88 worklog/BRIDGE-GAP-88.md in-progress
source: TS truth packages/tui/src/prompt/traits.ts:16-29 computePromptTraits (capture/status, owner/role prompt)
observed: TS has no slash/mention/paste flags; lane spec defines new bridge-local classification
target: ONE file crates/opentui-bridge/src/run_prompt_traits.rs only; no lib.rs/Cargo.toml touch; no cargo; no commit
tests: slash_detect, mention_detect, paste_threshold, empty_none, combined_flags, slash_only_single_char (6 in-file)
decisions: assoc fns + free fns both (spec `detect`/`has_any` ambiguous); chars().count threshold >1024; forbid(unsafe_code); std-only
unknowns: none
