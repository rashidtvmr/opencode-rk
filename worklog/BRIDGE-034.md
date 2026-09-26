# BRIDGE-034 routes_state

Claim: routes_state.rs page/dialog state.
Evidence: home.tsx:22-71; session-destination.tsx:13; session/index.tsx:186,339-345,517-552,1258-1294; dialog-subagent.tsx:4-25; dialog-timeline.tsx:10-14; dialog-fork-from-timeline.tsx:12; dialog-message.tsx:10-14; permission.tsx:111; question.tsx:14; route.tsx:6-15; prompt/history.tsx:9; footer.tsx session; sidebar.tsx:12; subagent-footer.tsx:11.
Target: SessionDialog(6) + SessionPage + HomePage, bounded ids/draft. No cargo run per scope.
Tests: 6 (destination, dialog empty, permission dir bound, session lifecycle, session bounds, home draft).
Unknowns: exact tab/draft fields beyond PromptInfo input; footer/sidebar carry no page state (display only).
