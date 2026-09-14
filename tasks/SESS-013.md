# SESS-013: Session Snapshot Module

## Scope
ONLY modify `crates/sessions/src/snapshot.rs`

## Goal
Implement a snapshot module for session state capture, restoration, listing, deletion, and statistics.

## Deliverable
`crates/sessions/src/snapshot.rs` with:

### Types
1. `Snapshot` struct: id (SessionId), created_at (Timestamp), sessions_snapshot (Vec<SessionSummary>), metadata (SnapshotMetadata)
2. `SnapshotMetadata` struct: session_count (u32), message_count (u64), compressed (bool), compression_ratio (f64)

### Methods
- `create(manager: &SessionManager) -> Result<Snapshot, SnapshotError>` - create snapshot from manager state
- `restore(snapshot_id: SessionId) -> Result<SessionManager, SnapshotError>` - restore manager from snapshot
- `list() -> Vec<Snapshot>` - list all snapshots
- `delete(id: SessionId) -> bool` - delete snapshot by id, returns true if deleted
- `stats(snapshot_id: SessionId) -> Option<SnapshotMetadata>` - get snapshot stats by id

### Storage Backend
- Snapshots stored in-memory in a static/global registry
- Use lazy_static or once_cell for global snapshot registry
- Each snapshot identified by its SessionId

## Test Obligations
1. `create_snapshot` - create_snapshot correctly captures session state from manager
2. `restore_preserves` - restore_preserves correctly reconstructs manager from snapshot
3. `list_returns_all` - list_returns_all returns all existing snapshots
4. `delete_removes` - delete_removes correctly removes snapshot by id
5. `stats_correct` - stats_correct returns correct metadata for snapshot

## Verification
```bash
cargo test -p opencode-rk-sessions
cargo check --workspace
```