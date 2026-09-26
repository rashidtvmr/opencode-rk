# BRIDGE-GAP-94 scratchpad

Claim: BRIDGE-GAP-94 via cc.claim, session ses_gap94.
Source: packages/tui/src/routes/session/index.tsx:186 route=useRouteData("session"), :517-530 DialogTimeline, :540-552 DialogForkFromTimeline, chat default view.
Naming: crates/opentui-bridge/src/routes_state.rs SessionPage/SessionDialog/SidebarMode/KvToggles, not edited.
Target: crates/opentui-bridge/src/route_session.rs only, std-only, forbid unsafe, <140 lines.
Tests: empty errs, switch ok, title truncates, default chat, view roundtrip.
Decisions: MAX_SESSION_ID=64 per task cap; title "session <id8> [view]" with lowercase view label.
