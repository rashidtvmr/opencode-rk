# BRIDGE-071 context_stores

Claim: plain-state mirrors of TUI Solid contexts, std only, no reactivity.
Evidence: tui/src/context/permission.tsx:5,11-23; clipboard.tsx:5-16;
theme.tsx:32-61,209-220,293-298; thinking.ts:4,12-27; prompt.tsx:9-16;
event.ts:12-30 (subscribe/on documented only, no state to mirror);
exit.tsx:3; project.tsx:22-36,89-112; directory.ts:7-16.
Reuses context_ui::{PermissionGate,PermissionState,ExitCode},
context_kv::{tildefy,Project} (Project struct already in context_kv;
ProjectStore here is the workspace list/current slice).
Target: crates/opentui-bridge/src/context_stores.rs created, lib.rs untouched.
Tests: 9 tests (permission, clipboard, theme, thinking, summary, prompt,
exit, project, directory). RED skipped (no cargo per scope); logically green
by inspection against existing crate test style.
Decisions: event.ts has no holdable state (subscribe/on are SDK wrappers) so
no struct; documented in header. Clipboard injectable in-memory only, no OS.
Unknowns: none blocking.
