# BRIDGE-033 scratchpad

Claim: system builtin plugin surface models.
Evidence:
- opencode a0d9b6c packages/tui/src/feature-plugins/system/notifications.ts:9-27 (notify + sessionErrorMessage)
- .../system/which-key.tsx:10-22 (commands), :126-142 (activeKeyEntry key/label)
- .../home/tips-view.tsx:71 (NO_MODELS_TIP), :164-283 (TIPS)
- .../home/footer.tsx:64-82 (View row), 8 (id internal:home-footer)
- .../home/tips.tsx:7 (id internal:home-tips), .../system/plugins.tsx:9 (id internal:plugin-manager)
- .../builtins.ts:21-35 (builtin list); plugin_slots.rs reused, not redefined.
Target: crates/opentui-bridge/src/system_plugins.rs only. lib.rs wiring left to owner.
Tests: 7 (notification_bounds, session_error_mapping, queue_bounded_fail_closed,
whichkey_add_lookup_bound, tip_rotates_and_wraps, footer_bound_render_slot, system_ids_known).
Decisions: queue fail-closed Full (TS live-Set dedup); tips static plain-string subset,
rotating index (TS random offset); footer render "left | right".
Unknowns: diff-viewer builtin has no stable id const (grouped file rows) - excluded.
Status: completed, cargo not run per scope (logically green by inspection).
