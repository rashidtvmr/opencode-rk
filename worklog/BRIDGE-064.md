# BRIDGE-064 worklog

Claim: additive specs to small_widgets.rs only.
Source: checkout a0d9b6c; TS files read fully with line evidence.
- startup-loading.tsx:6 text, :22 MIN_SHOW 3000, :42 SHOW_DELAY 500.
- todo-item.tsx:4-5 status/content, :19 marker map.
- use-connected.tsx:7-9 Provider[] predicate.
- workspace-label.tsx:5 status/icon, :16 display.
- plugin-route-missing.tsx:3 onHome callback, :8 message.
- spinner.tsx:10 SPINNER_FRAMES, :17 animations_enabled fallback.
- register-spinner.ts:4 register iff catalogue.spinner missing.

Added: SHOW_DELAY_MS, MIN_SHOW_MS, MAX_TODO_STATUS, IN_PROGRESS_MARKER,
MAX_WS_STATUS, MAX_FRAMES, should_show, hold_remaining_ms (helper),
TodoItem status/content + with_status + in-progress marker branch,
providers_present, WorkspaceLabel status/icon + with_status/with_icon,
RouteMissing home_action + with_home_action, SpinnerDef + fallback +
register_opencode_spinner.
Existing items untouched (signatures preserved, only struct field additions).
Tests: 6 existing kept, 6 new added. No cargo run per scope ban; green by inspection.
Unknowns: none blocking; Provider[] full model out of scope by design.
