# BRIDGE-068 routes_state extension

Claim: additive-only extension of `crates/opentui-bridge/src/routes_state.rs`.
Existing items preserved (SessionDestination, SessionDialog 6 variants, SessionPage/HomePage shapes).

Evidence (TS checkout a0d9b6c):
- `context/route.tsx:17-21` PluginRoute{id,data?}; `:6-15` Home/SessionRoute.
- `routes/home/session-destination.tsx:13` HomeSessionDestination picker.
- `prompt/history.tsx:9-12` PromptInfo{input,mode,parts}; `component/prompt/history.tsx:1` re-export.
- `routes/session/index.tsx:249-269` sidebar kv auto/hide + sidebarOpen + wide>120 + visible memo; `:255-261` kv toggles; `:364` retry show; `:431,2303` alert; `:481,1197` confirm; `:507` rename; `:520,527,542,1262` onMove/setPrompt; `:958` export options.
- `routes/session/dialog-timeline.tsx:10-14`, `dialog-fork-from-timeline.tsx:12`, `dialog-message.tsx:10-14` callback shapes.
- `component/dialog-session-rename.tsx:7-9`, `ui/dialog-alert.tsx:6-10,59`, `ui/dialog-confirm.tsx:9-15,93`, `ui/dialog-export-options.tsx:8-22`, `component/dialog-retry-action.tsx:15-21,150`.

Added: PluginRoute{MAX_DATA 4k}, HomeDestination{phase,directory MAX_DIR 1024,subdirectory}, PromptDraft{input 16k, mode whitelist normal|shell} + draft_info/set_draft_info, SessionDialog Rename/Alert/Confirm/Export/RetryAction (evidenced props only), SidebarMode{Auto,Shown,Hidden}+SidebarOpts{wide}+sidebar_mode, KvToggles+defaults(), TIMELINE_ACTIONS/MESSAGE_ACTIONS consts, MAX_DIR/MAX_TEXT/MAX_MODE consts.
Destination correction documented in module docs: TS picker (pre-submit dir) vs kept SessionDestination (post-submit target) via phase pin.

Tests: 6 pre-existing kept + 7 new (plugin_route_bounded, home_destination_picker, prompt_draft_mode_whitelist, dialog_new_variants, sidebar_mode_visibility, kv_toggles_defaults, action_id_consts) + extended session_page_bounds_draft_info. Total 13.
RED: new tests fail pre-impl by construction (reference missing items); impl then standalone `rustc --test routes_state.rs` GREEN 13/13.
Full `cargo test` BLOCKED by pre-existing `prompt_composer.rs:310-312` E0753 inner-doc error (not my file, untouched per scope).

Unknowns: plugin data serialization format (chose JSON string, bounded); confirm label bound (used MAX_MODE 32, no TS len given); Alert/Confirm title/message bound (MAX_TEXT 1024, no TS len given).
