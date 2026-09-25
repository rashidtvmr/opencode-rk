# BRIDGE-PAR-420 scratchpad (unclaimed)

Status: unclaimed (orchestrator owns tasks/completion/claims.json; proceeded file-only per task order, no claim attempted).
Claim: none. Source evidence: TS truth `/home/rashid/projects/opencode/packages/tui/src/audio.d.ts:1-9` (only `*.mp3 -> string path` decls, no codec info); sibling style `crates/opentui-bridge/src/audio_stub_full.rs:1-79`, `audio.rs:1-11`.
Observed: no existing AudioFmt; new isolated file only.
Target boundary: ONE file `crates/opentui-bridge/src/audio_dts_full.rs`, std-only, forbid(unsafe_code), <60 lines, >=3 tests. No edits to lib.rs/Cargo.toml/audio.rs/audio_stub_full.rs. No cargo, no commit.
Tests: empty_by_default, set_roundtrip, cap_64 (all in-file `mod tests`).
Decisions: `set` truncates to 64 chars (grapheme-blind, std `chars().take(64)`); `name_of -> &str`; `is_set = !empty`. `ponytail:` capped String only.
Unknowns: none; rustfmt --check pending.
