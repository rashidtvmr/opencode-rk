# BRIDGE-GAP-54 scratchpad

Claim: BRIDGE-GAP-54 via ses_gap54, in-progress (pre-existing fence).
Source: crates/opentui-bridge/src/theme_utils.rs (pack_options cap 256, border/fallback), theme_resolve.rs (forbid unsafe, TS theme compat). No theme.tsx in repo; TS ref per task spec.
Target: crates/opentui-bridge/src/context_theme.rs only. No lib.rs/Cargo.toml edits, no cargo run, no commit.
Tests: 6 in-file (apply unlocked, locked blocks, palette default, discover cap, lock cycle, palette truncate 64).
Decisions: std-only, forbid(unsafe_code), 169 lines, ponytail: no registry integration, add when wired.
Evidence: rustfmt --check PASS, 169 lines.
