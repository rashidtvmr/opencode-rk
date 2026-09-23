# BRIDGE-035 small_widgets

Claim: small widget contracts ported from TS checkout a0d9b6c.
Evidence: link.tsx:5-12,28; plugin-route-missing.tsx:8; startup-loading.tsx:6; todo-item.tsx:19; use-connected.tsx:6-10; workspace-label.tsx:16; register-spinner.ts:5.
Tests: 6 unit tests in-file (link, route, startup, todo, connection, workspace). RED: n/a (no cargo run per scope); logically green by inspection.
Decisions: http/https only fail-closed; in_progress marker render-only, TodoItem bool only; StartupPhase labels + StartupSequence cursor; WorkspaceLabel basename handles / and \.
Unknowns: lib.rs mod registration left to integrator (scope: single file only).
