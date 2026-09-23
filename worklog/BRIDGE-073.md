# BRIDGE-073 editor_spawn

Claim: spawn-plan types, IDE socket pick, utf8 offset map.
Evidence: editor.ts:26-54 openEditor, :56-96 discoverEditorConnection, :98-101 facade; editor-zed.ts:41-87 resolveZedSelection, :223-243 offsetToPosition, :245-273 offsetsToSelection/position.
Boundary: plan types only, no spawn/exec/sqlite; host executes. Reuse: crate::editor (buffer ops), crate::editor_zed (probe/to_open_request).
Tests: 8 in-file (spawn ok/bounds, best wins/tie-first, ascii lines, multibyte snap-up + surrogate UTF-16, range validate, socket validate).
Gaps: lib.rs wiring left to parent (scope: one file only). Past-end offset returns None (stricter than TS clamp). character in UTF-16 units (exact TS rule).
