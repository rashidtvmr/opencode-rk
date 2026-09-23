# BRIDGE-029 context_ui

Claim: context_ui.rs created, NOT wired (lib.rs untouched per scope).
Evidence (TS @ a0d9b6c):
- theme.tsx:84-98,289-291; clipboard.tsx:4-8; permission.tsx:5 (+replies types.gen.ts:1400, config ask|allow|deny :1657, permission.tsx route:168-181)
- prompt.tsx:4-18 + placeholder prompt/index.tsx:1316; epilogue.tsx:3-6 + presentation.ts:29-37 + app.tsx:361
- exit.tsx:3 + error.ts:11-12 + agent.ts:134,216; thinking.ts:4,24-27,36; event.ts:12-20 + sync.tsx:170-439, app.tsx:985-1031
RED: not run (cargo forbidden by task).
Tests: 6 inline (theme roundtrip, permission replies, exit codes, thinking toggle, ui events, epilogue bounds).
Unknowns: full UiEvent list vs never-subscribed SDK names; exit>1 codes beyond passthrough; permission expiry absent.
