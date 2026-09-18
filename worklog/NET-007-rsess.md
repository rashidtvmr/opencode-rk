# NET-007-rsess — remote_sessions.rs

Claim: multi-session routing types in owned file only.
Commit: 5af7884. File did not exist (verified via read miss); created.
Sibling evidence: crates/server/src/remote_turns.rs:1-10 RED scaffold pattern; NET-007 card (completion_plan.py): DeviceId/WorkspaceId/SessionId routing, daemon-owned history, versioned converge, tab-close vs kill, revocation.

Boundary: crates/server/src/remote_sessions.rs only. No lib.rs wiring (owned-file-only lease). #![forbid(unsafe_code)], std only (HashMap/HashSet).

Tests (5, map to T01..T05): cross_routing_rejected, drafts_client_local, rename_converges, close_tab_preserves_background_session, revocation_blocks_queued.
Decisions: CrossWorkspace error carries expected/got; converge = later-version-wins, stale Err; kill_session(session, explicit=false) no-op; revoke drops subs + queue_action/open_tab → Revoked; Fork clones history into child v0.
Unknowns: integration with crates/control-plane/src/session_routes + lib.rs wiring left to integrator.

Evidence: `rustc --edition 2021 --test crates/server/src/remote_sessions.rs -o /tmp/opencode/rs && /tmp/opencode/rs` → 5 passed, 0 failed. sha256 3ab5337769ebe916442795485228bfa59e2aa05fec4d8406bcc8a92fd5e0637a.
