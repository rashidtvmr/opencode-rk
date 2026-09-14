//! Schema migration runner with checksum verification.
use blake3::hash as blake3_hash;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

#[derive(Clone)]
pub struct Migration {
    pub version: u32,
    pub name: String,
    pub sql: String,
    pub checksum: String,
}
impl Migration {
    pub fn new(version: u32, name: impl Into<String>, sql: impl Into<String>) -> Self {
        let sql = sql.into();
        let checksum = blake3_hash(sql.as_bytes()).to_hex().to_string();
        Self {
            version,
            name: name.into(),
            sql,
            checksum,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppliedMigration {
    pub version: u32,
    pub applied_at: DateTime<Utc>,
    pub checksum: String,
}

pub struct MigrationRunner<'a> {
    conn: &'a Connection,
}
impl<'a> MigrationRunner<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
    pub fn init_migration_table(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch("CREATE TABLE IF NOT EXISTS _migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at TEXT NOT NULL)")
    }
    pub fn run_pending(&self, migrations: &[Migration]) -> rusqlite::Result<Vec<u32>> {
        self.init_migration_table()?;
        let applied = self.applied()?;
        let applied_set: std::collections::HashSet<u32> =
            applied.iter().map(|m| m.version).collect();
        let mut pending: Vec<&Migration> = migrations
            .iter()
            .filter(|m| !applied_set.contains(&m.version))
            .collect();
        pending.sort_by_key(|m| m.version);
        let mut result = Vec::new();
        for migration in pending {
            self.conn.execute_batch(&migration.sql)?;
            self.conn.execute("INSERT INTO _migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)", params![migration.version as i64, &migration.name, &migration.checksum, Utc::now().to_rfc3339()])?;
            result.push(migration.version);
        }
        Ok(result)
    }
    pub fn applied(&self) -> rusqlite::Result<Vec<AppliedMigration>> {
        self.init_migration_table()?;
        let mut stmt = self
            .conn
            .prepare("SELECT version, applied_at, checksum FROM _migrations ORDER BY version")?;
        let rows = stmt.query_map([], |row| {
            let s: String = row.get(1)?;
            let at = DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok(AppliedMigration {
                version: row.get::<_, i64>(0)? as u32,
                applied_at: at,
                checksum: row.get(2)?,
            })
        })?;
        rows.collect()
    }
    pub fn verify_checksums(
        &self,
        migrations: &[Migration],
    ) -> rusqlite::Result<Vec<(u32, String, String)>> {
        self.init_migration_table()?;
        let applied = self.applied()?;
        Ok(applied
            .iter()
            .filter_map(|am| {
                migrations
                    .iter()
                    .find(|m| m.version == am.version)
                    .and_then(|m| {
                        if m.checksum != am.checksum {
                            Some((am.version, am.checksum.clone(), m.checksum.clone()))
                        } else {
                            None
                        }
                    })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_migrations() -> Vec<Migration> {
        vec![
            Migration::new(
                1,
                "create_users",
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
            ),
            Migration::new(
                2,
                "create_posts",
                "CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER, title TEXT)",
            ),
            Migration::new(3, "add_email", "ALTER TABLE users ADD COLUMN email TEXT"),
        ]
    }
    #[test]
    fn empty_db_runs_all() {
        let conn = Connection::open_in_memory().unwrap();
        let r = MigrationRunner::new(&conn);
        let v = r.run_pending(&test_migrations()).unwrap();
        assert_eq!(v, vec![1, 2, 3]);
        assert_eq!(r.applied().unwrap().len(), 3);
    }
    #[test]
    fn skip_already_applied() {
        let conn = Connection::open_in_memory().unwrap();
        let r = MigrationRunner::new(&conn);
        let m = test_migrations();
        r.run_pending(&m).unwrap();
        assert!(r.run_pending(&m).unwrap().is_empty());
    }
    #[test]
    fn checksum_mismatch_fails() {
        let conn = Connection::open_in_memory().unwrap();
        let r = MigrationRunner::new(&conn);
        let m = test_migrations();
        r.run_pending(&m).unwrap();
        conn.execute(
            "UPDATE _migrations SET checksum = 'tampered' WHERE version = 2",
            [],
        )
        .unwrap();
        let bad = r.verify_checksums(&m).unwrap();
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].0, 2);
    }
    #[test]
    fn ordered_execution() {
        let conn = Connection::open_in_memory().unwrap();
        let r = MigrationRunner::new(&conn);
        let m = vec![
            Migration::new(3, "third", "CREATE TABLE third (id INTEGER)"),
            Migration::new(1, "first", "CREATE TABLE first (id INTEGER)"),
            Migration::new(2, "second", "CREATE TABLE second (id INTEGER)"),
        ];
        let v = r.run_pending(&m).unwrap();
        assert_eq!(v, vec![1, 2, 3]);
        let mut s = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name IN ('first','second','third') ORDER BY name").unwrap();
        let names: Vec<String> = s
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(names, vec!["first", "second", "third"]);
    }
    #[test]
    fn partial_failure_rolls_back() {
        let conn = Connection::open_in_memory().unwrap();
        let r = MigrationRunner::new(&conn);
        let m = vec![
            Migration::new(1, "good", "CREATE TABLE t1 (id INTEGER)"),
            Migration::new(2, "bad", "INVALID SQL STATEMENT"),
        ];
        assert!(r.run_pending(&m).is_err());
    }
}
