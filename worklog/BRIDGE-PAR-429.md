# BRIDGE-PAR-429 (unclaimed, file-only per orchestrator)

Claim: SessDestRoute {dest cap 512} + set + dest_of + is_set.
Source: packages/tui/src/routes/home/session-destination.tsx:13-29 (HomeSessionDestination defaults sync.path.directory || paths.cwd; provider set/clear); prior art crates/opentui-bridge/src/session_destination_full.rs (clip pattern), home_destination.rs.
Boundary: new file sess_dest_route_full.rs only. No lib.rs/Cargo.toml edits. No cargo/commit.
Tests: set_stores_dest, unset_is_empty, set_clips_to_512.
Status: file written, rustfmt pending.
