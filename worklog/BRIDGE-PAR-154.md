# BRIDGE-PAR-154
Claim: ses_par154. Owned file: crates/opentui-bridge/src/helper_ctx.rs.
Source: packages/tui/src/context/helper.tsx:1-26 (createSimpleContext generic show-gate, no fixed state).
Target: HelperCtx {visible, topic cap 64} + show/hide/toggle/label.
Tests: 6 in-file (default_hidden, show, hide, toggle_shows_then_switches, toggle_hides_same_topic, trunc).
Decisions: char-count truncation; hide keeps topic; toggle compares capped; forbid(unsafe_code); std-only.
Unknowns: wiring into lib.rs left to integrator (out of scope).
