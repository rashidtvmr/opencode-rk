# BRIDGE-PAR-388 scratchpad (UNCLAIMED - orchestrator owns ledger, no claim made)

Claim: new file crates/opentui-bridge/src/editor_ts_full.rs.
Source: /home/rashid/projects/opencode/packages/tui/src/editor.ts:26-54 openEditor, :56-96 discoverEditorConnection; local reuse boundary: editor.rs OpenEditorRequest, editor_spawn.rs EditorSpawn/pick_best, editor_zed.rs ZedRequest.
Observed: grep remote/label in editor.ts = no match; remote prefixes chosen as ssh/remote/vscode-remote probe.
Target: editor_arg(path,line)->String cap 512 + is_remote prefix + editor_label static. std-only, forbid unsafe, <70 lines, >=3 tests.
Tests: 4 tests in-file (arg_line_suffix, arg_caps, remote_prefix, label_static). NOT run via cargo (scope forbids cargo). rustfmt --check: FMT_OK. Lines: 66 (under 70).
Decisions: line 0 = no suffix; truncate on char boundary; remote = 3 scheme prefixes.
Unknowns: wiring into lib.rs left to orchestrator (scope forbids lib.rs edit).
