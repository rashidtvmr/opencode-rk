# BRIDGE-PAR-237 session_destination_full

- Claim: BRIDGE-PAR-237 via completion_claims, session ses_par237, scratchpad worklog/BRIDGE-PAR-237.md.
- Source evidence:
  - TS truth `packages/tui/src/routes/home/session-destination.tsx:13,28` - `HomeSessionDestination` directory|new, default `sync.path.directory || paths.cwd`.
  - Sibling pattern `crates/opentui-bridge/src/session_index.rs:16-47` - caps (32/64), dedup-false, char-safe truncate; read only, not edited.
  - Style pattern `crates/opentui-bridge/src/session_header.rs:1,37-39` - forbid(unsafe_code), std-only, char-safe clip.
- Target boundary: ONE new file `crates/opentui-bridge/src/session_destination_full.rs`. No lib.rs/Cargo.toml/session_index.rs edits. No cargo, no commit.
- Tests: 6 in-file (new-clip, set-replace, push-add, push-dup, push-empty, push-evict-16). Written before verification.
- Decisions: push_recent rejects empty/dup (false, sibling register_action semantics), evicts oldest at 16 (recent-list LRU bound) then true. `ponytail:` no subdirectory/new-type enum; add when home route needs it.
- Remaining: rustfmt --check only per scope.
