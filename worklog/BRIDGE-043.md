# BRIDGE-043 scratchpad

Claim: session_msg_dialogs.rs inner contracts for DialogMessage/Timeline/ForkFromTimeline.
Source: packages/tui/src/routes/session/dialog-message.tsx:24-105, dialog-timeline.tsx:22-44, dialog-fork-from-timeline.tsx:24-73 (TS a0d9b6c).
Boundary: actions/list/fork-request only. Open/close state stays routes_state.rs SessionDialog. Do NOT edit lib.rs (orchestrator wires).
Tests: 6, written pre-impl (RED assumed, cargo NOT run per scope ban).
Decisions: exactly 3 MessageActions (no edit/retry in source); None = full-session fork mode (no mode param evidenced); reverse list; clamp jump_to.
Unknowns: prompt-rebuild (non-synthetic text+file parts) left to caller; Footer Locale.time not ported.
