//! Migration functions for session store.
//!
//! Provides table creation and migration tracking for the session store.

use rusqlite::Connection;

/// Create the sessions table if it doesn't exist.
pub fn create_session_table(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            id BLOB NOT NULL UNIQUE CHECK(length(id) = 16),
            title TEXT NOT NULL,
            state INTEGER NOT NULL DEFAULT 0 CHECK(state IN (0, 1)),
            created_at_us INTEGER NOT NULL,
            updated_at_us INTEGER NOT NULL,
            archived_at_us INTEGER
        )",
        [],
    )?;
    Ok(())
}

/// Ensure all migration tables and versions exist.
pub fn ensure_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        )",
        [],
    )?;
    create_session_table(conn)?;

    // Record migration version if not already present
    let version: Option<i64> = conn
        .query_row(
            "SELECT version FROM migrations WHERE version = 1",
            [],
            |row| row.get(0),
        )
        .ok();

    if version.is_none() {
        conn.execute(
            "INSERT INTO migrations (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![1i64, chrono::Utc::now().to_rfc3339()],
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_session_table_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        // Should not fail when called multiple times
        create_session_table(&conn).unwrap();
        create_session_table(&conn).unwrap();
    }

    #[test]
    fn ensure_migrations_creates_tables() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_migrations(&conn).unwrap();

        // Verify sessions table exists
        let table_exists: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='sessions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_exists, 1);
    }

    #[test]
    fn ensure_migrations_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_migrations(&conn).unwrap();
        ensure_migrations(&conn).unwrap();
    }

    #[test]
    fn ensure_migrations_tracks_version() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_migrations(&conn).unwrap();

        let version: i64 = conn
            .query_row(
                "SELECT version FROM migrations WHERE version = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);
    }
}
