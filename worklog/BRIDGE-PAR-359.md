# BRIDGE-PAR-359 (unclaimed, file-only per orchestrator)

Lease: ONE new file crates/opentui-bridge/src/route_sess_footer_full.rs.
No lib.rs / Cargo.toml edits. No cargo. No commit. No claims.json touch.

Claim: skipped per orchestrator override (proceed file-only, unclaimed).
Source evidence:
- TS truth: packages/tui/src/routes/session/footer.tsx:1-60 (Footer, theme/sync/route, welcome tick, box layout)
- Existing sibling: crates/opentui-bridge/src/session_footer.rs:31-76 (SessionFooter view+busy, render cap 64)
Target boundary: SessFooter {mode: String cap 32, busy: bool} + set_mode(&str) + set_busy + status()->String cap 128. std-only, forbid(unsafe_code), under 80 lines, >=3 tests.
Tests: 4 tests in-file (default_idle_empty, mode_capped_at_32, busy_status, status_capped_at_128).
Decisions: chars().take caps (unicode-safe); status "<mode> <busy|idle>" cap 128; new() reuses set_mode for cap.
Verification: `rtk rustfmt --check crates/opentui-bridge/src/route_sess_footer_full.rs` PASS (exit 0, no output).
Remaining: orchestrator to wire into lib.rs + run cargo tests.
