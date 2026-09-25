# BRIDGE-GAP-35 scratchpad

Claim: ses_gap35. Source: crates/opentui-bridge/src/input.rs:60-133 FocusRing, render_focus.rs FocusedStyle. Target: crates/opentui-bridge/src/focus_wire.rs. Tests: register_and_cycle, empty_is_none_fail_closed, wrap_around_both_directions, unknown_id_kept_as_none, duplicate_register_errs, dialog_target_reachable. Decision: Vec map (bounded MAX_FOCUS), unknown id kept, lookup None fail-closed. rustfmt: see FMT_OK above.
