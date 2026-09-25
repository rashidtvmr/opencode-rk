# BRIDGE-GAP-68 scratchpad

claim: BRIDGE-GAP-68 via completion_claims, session ses_gap68. ok.
source: TS truth packages/opencode/src/cli/cmd/run/session.shared.ts (createSession/resolveSession keyed by session id; history/variant readers). style model crates/opentui-bridge/src/session_title.rs (forbid unsafe, doc evidence header).
target: ONE new file crates/opentui-bridge/src/run_session_shared.rs. no lib.rs/Cargo.toml edits, no cargo, no commit.
tests: in-file 6 (upsert_adds, upsert_updates_title, remove_missing_false, get_none, cap_256, latest_is_last).
decisions: upsert bool = true on insert/update, false only on cap reject. truncate at char boundary via chars().take. latest = Vec::last (insertion order = recency).
unknowns: none. line count target <170.
