# BRIDGE-PAR-338 (unclaimed: ledger overflow, orchestrator owns claims)

Claim: none (per instruction, no claims.json touch).
Source: opencode/packages/tui/src/component/command-palette.tsx:48-60 (options mapping); local pattern crates/opentui-bridge/src/dialog_select_full.rs:1-60; existing crates/opentui-bridge/src/command_palette.rs (richer PaletteState, no conflict: new CmdPalette type).
Target: crates/opentui-bridge/src/command_palette_full.rs, CmdPalette + set_query/filtered/move_cursor/selected_filtered, std-only, forbid(unsafe_code), 127 lines.
Tests: 6 in-file (empty, caps, query cap/reset, case-insensitive filter, 32-cap, cursor wrap).
Decisions: free push() helper added (needed to fill items); filtered() recomputes (simple, bounded 64x128).
Unknowns: lib.rs wiring left to orchestrator (no edit per scope).
Verification: rustfmt --check PASS, FMT_OK.
