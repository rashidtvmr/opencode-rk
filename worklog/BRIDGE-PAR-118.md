# BRIDGE-PAR-118 scratchpad

- claim: BRIDGE-PAR-118 ses_par118 in-progress
- source: packages/tui/src/plugin/adapters.tsx:41-73 routeNavigate, routeCurrent; mapOption 75-80 passthrough
- target: crates/opentui-bridge/src/plugin_routes.rs only
- tests: 6 in-file (home nav, session ok, session needs id, plugin fallback, names, trunc)
- decisions: char-count caps (64 id, 128 label); empty plugin name falls back? No - any non-home/session name -> Plugin incl empty truncated
