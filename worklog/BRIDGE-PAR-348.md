# BRIDGE-PAR-348 scratchpad (UNCLAIMED: ledger overflow, orchestrator owns claims.json)

- Claim: skipped per spawn note. No ledger touch. File-only lane.
- Source evidence: packages/tui/src/audio.ts:1-40 (Audio.create autoStart false, error listener, loadSoundFile cache, play null when no context); crates/opentui-bridge/src/audio.rs:1-30 (SoundName registry, MAX_SOUNDS cap); style model crates/opentui-bridge/src/toast_single.rs (forbid unsafe, TS-truth docs, ponytail note, caller-clock testability).
- Observed scenario: no AudioStub exists; audio.rs already covers registry+guard, so new file must not duplicate it.
- Target boundary: ONE new file crates/opentui-bridge/src/audio_stub_full.rs. No lib.rs, no Cargo.toml, no cargo, no commit.
- Tests: 3 in-module (unmuted counts, muted noop, set_muted toggles). Frozen on write.
- Decisions: pub fields (spec shape {muted, plays}); plays() method coexists with field (separate namespaces); saturating_add ceiling; Default derive, new() unmuted.
- Unknowns: none. Awaiting orchestrator wiring + rustfmt gate.
