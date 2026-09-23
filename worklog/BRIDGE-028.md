# BRIDGE-028 context_session

Claim: session snapshot + route + sync contract.
Source: TS checkout a0d9b6c route.tsx:6-23 sync.tsx:65,578-587
 session-status-event.ts:9-32 sdk.tsx:86-116 location.tsx:1.
Target: crates/opentui-bridge/src/context_session.rs only.
Tests: 6 (route roundtrip, stale, kind roundtrip, empty id, title bound, status+location).
Decisions: offline=>stale on construct+set_kind; reconnecting not stale;
 title bound 512 chars (display trunc 50 out of scope); Location workspace only.
Unknowns: none blocking; compacting derived state (`sync.tsx:581`) folded to Busy.
