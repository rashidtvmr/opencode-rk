# BRIDGE-021 command_palette

Claim: bounded palette state + prefix/substring rank.
Evidence: opencode(a0d9b6c) command-palette.tsx:15-17 filter, :48-61 mapping, :64-76 suggested; keymap.tsx:22 self-command; prompt/autocomplete.tsx:502-525 fuzzysort+prefix boost.
Target: crates/opentui-bridge/src/command_palette.rs only (lib.rs wiring left to owner).
Tests: 7 in-file (order, prefix>substring, alias/desc, empty-rank noop, wrap/clamp, bounds, const). Not run (no cargo per scope).
Decisions: std-only substring/prefix scoring, stable tiers; no frecency; query truncate 256, entries cap 512.
Unknowns: DialogSelect fuzzy algorithm exact semantics; suggested-hoist mirrored by caller, not state.
