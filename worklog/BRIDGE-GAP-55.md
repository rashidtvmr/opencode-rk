# BRIDGE-GAP-55 scratchpad

claim: BRIDGE-GAP-55 via ses_gap55 (tools/completion_claims.py claim).
source evidence: crates/opentui-bridge/src/editor.rs:1-242 (EditorState, EditorKind, EditorError, OpenEditorRequest, is_managed_textarea; forbid unsafe, bounded, doc-comment style with TS refs).
observed: no context_editor.rs exists; editor.ts WS schemas (selection + mention) missing.
target boundary: ONE new file crates/opentui-bridge/src/context_editor.rs only. No lib.rs/Cargo.toml edits. No cargo. No commit/push.
tests: in-file #[cfg(test)] >=5 (valid selection, inverted errs, key format, mention parse, mention bad none).
decisions: rsplit_once(':') for "file:line" (supports paths with colons, e.g. drive letters); file cap 512 enforced in parse_mention; validate_selection checks lines>0 + start<=end only per spec.
remaining: rustfmt --check + ledger completed flip.
