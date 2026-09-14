# SESS-012: Migration System for Session Store

## Overview

Implement a migration system in `crates/sessions/src/migration.rs` that provides schema version tracking, forward migrations, and rollback capabilities for the session store.

## Scope

- **Owned File**: `crates/sessions/src/migration.rs`
- **Dependencies**: `crates/contracts/src/lib.rs` (Timestamp, SessionId)

## Deliverable

`sesson.Migration` struct with:
- `from_version: u32` - starting schema version
- `to_version: u32` - target schema version  
- `applied_at: Timestamp` - when migration was applied
- `status: MigrationStatus` - enum: Pending, Applied, RolledBack

`MigrationStatus` enum with variants: Pending, Applied, RolledBack

`MigrationManager` struct with:
- `migrations: Vec<Migration>` - registered migrations
- `applied: HashMap<String, Timestamp>` - applied migrations by version key

Methods:
- `run_migrations()` - apply all pending migrations in order
- `rollback(id: &str)` - rollback a specific migration by version id
- `status() -> Vec<MigrationStatus>` - return status of all migrations
- `validate()` - verify all applied migrations match checksums

## Test Obligations

1. `applies_migrations` - migrations are applied in order, status updates to Applied
2. `rollback_works` - rollback changes status to RolledBack, allows re-apply
3. `status_tracks` - status() correctly reflects migration states
4. `validate_passes` - validate() returns Ok when all migrations match
5. `idempotent` - running run_migrations multiple times only applies once

## Verification

```bash
cargo test -p opencode-rk-sessions && cargo check --workspace
```

Both commands must pass with no errors.