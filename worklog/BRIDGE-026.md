# BRIDGE-026 scratchpad

Claim: TuiConfig resolved defaults + validate.
Evidence: index.tsx:21 (LeaderTimeoutDefault 2000), :22-24 (leader>0),
:55 (theme string), :62-63 (scroll), :65,:115 (mouse default true),
:88-117 resolve(); context/theme.tsx:121 (theme default "opencode");
routes/session/index.tsx:260, app.tsx:898, spinner.tsx:17 (animations_enabled true).
Reused: crate::keymap::LEADER_TIMEOUT_DEFAULT_MS, crate::scroll_accel::{ScrollConfig, is_valid_speed}.
Target: crates/opentui-bridge/src/tui_config.rs only. lib.rs NOT touched (owner must add `pub mod tui_config;`).
Tests: 6 in-file (defaults, validate-ok, empty/long/control theme, zero timeout, bad speed). Not run (no cargo per scope).
Unknowns: none blocking.
