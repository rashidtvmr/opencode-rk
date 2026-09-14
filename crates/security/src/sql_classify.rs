//! Advanced SQL classification for destructive patterns (SEC-012).
//! Complements `PermissionBroker::authorize_sql` with a standalone,
//! configurable classifier returning review/deny verdicts.

/// Strictness tunes how `NeedsReview` findings are reported.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum Strictness {
    /// Review findings are downgraded to `Safe`.
    Permissive,
    /// Review findings stay `NeedsReview` (default).
    #[default]
    Standard,
    /// Review findings are escalated to `Denied`.
    Strict,
}

/// Verdict for a single SQL input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SqlRisk {
    Safe,
    NeedsReview(String),
    Denied(String),
}

impl SqlRisk {
    #[must_use]
    pub fn is_safe(&self) -> bool {
        matches!(self, SqlRisk::Safe)
    }
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self, SqlRisk::Denied(_))
    }
    #[must_use]
    pub fn is_review(&self) -> bool {
        matches!(self, SqlRisk::NeedsReview(_))
    }
}

/// Configurable classifier. Case-insensitive. Pure function of input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct SqlClassifier {
    pub strictness: Strictness,
}

impl SqlClassifier {
    #[must_use]
    pub fn new(strictness: Strictness) -> Self {
        Self { strictness }
    }
    #[must_use]
    pub fn standard() -> Self {
        Self::new(Strictness::Standard)
    }
    #[must_use]
    pub fn strict() -> Self {
        Self::new(Strictness::Strict)
    }
    #[must_use]
    pub fn permissive() -> Self {
        Self::new(Strictness::Permissive)
    }

    /// Classify one SQL string (possibly multi-statement or dot-command).
    #[must_use]
    pub fn classify(&self, sql: &str) -> SqlRisk {
        let trimmed = sql.trim();
        let upper = trimmed.to_ascii_uppercase();
        let words: Vec<&str> = upper
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|w| !w.is_empty())
            .collect();
        let first = words.first().copied().unwrap_or("");
        let has_where = words.iter().any(|w| *w == "WHERE");

        // Dot-commands (sqlite3 shell): file read/write outside SQL grammar.
        if upper.starts_with(".IMPORT") || upper.starts_with(".LOAD") {
            return SqlRisk::Denied("sqlite dot-command .import/.load is never executed by an agent".to_owned());
        }
        // Privilege changes: never agent-executed.
        if matches!(first, "GRANT" | "REVOKE") {
            return SqlRisk::Denied(format!("{first} privilege changes are never executed by an agent"));
        }
        // Cross-database file attach: never agent-executed.
        if words.iter().any(|w| *w == "ATTACH") {
            return SqlRisk::Denied("ATTACH DATABASE is never executed by an agent".to_owned());
        }
        // Stacked statements: each extra statement hides intent.
        let stmts = upper.split(';').filter(|s| !s.trim().is_empty()).count();
        if stmts > 1 {
            return self.gate("multiple SQL statements in one input require human approval".to_owned());
        }
        // Index DDL changes query plans and locks writers.
        if matches!(
            words.as_slice(),
            ["CREATE", "INDEX", ..] | ["CREATE", "UNIQUE", "INDEX", ..] | ["DROP", "INDEX", ..]
        ) {
            return self.gate("CREATE/DROP INDEX requires human approval".to_owned());
        }
        // UPDATE without WHERE rewrites the whole table.
        if first == "UPDATE" && !has_where {
            return self.gate("UPDATE without an explicit WHERE clause requires human approval".to_owned());
        }
        // Bulk copy inside the engine, bypasses row-level review.
        if first == "INSERT" && words.iter().any(|w| *w == "SELECT") {
            return self.gate("INSERT INTO ... SELECT bulk copy requires human approval".to_owned());
        }
        // SQLite pragmas: reads are inert, writes reconfigure the engine.
        if first == "PRAGMA" {
            if is_write_pragma(&upper) {
                return self.gate("write PRAGMA (SQLite engine config) requires human approval".to_owned());
            }
            return SqlRisk::Safe;
        }
        SqlRisk::Safe
    }

    fn gate(&self, reason: String) -> SqlRisk {
        match self.strictness {
            Strictness::Permissive => SqlRisk::Safe,
            Strictness::Standard => SqlRisk::NeedsReview(reason),
            Strictness::Strict => SqlRisk::Denied(reason),
        }
    }
}

/// Standard-classifier shortcut.
#[must_use]
pub fn classify_sql(sql: &str) -> SqlRisk {
    SqlClassifier::standard().classify(sql)
}

/// A PRAGMA is a write when it assigns (`=`) or invokes a mutating
/// maintenance pragma callable without `=`.
fn is_write_pragma(upper: &str) -> bool {
    if upper.contains('=') {
        return true;
    }
    // Note: `upper` keeps underscores; `words` splits on them.
    upper.contains("WAL_CHECKPOINT") || upper.contains("SHRINK_MEMORY") || upper.contains("OPTIMIZE")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn std() -> SqlClassifier {
        SqlClassifier::standard()
    }

    #[test]
    fn safe_select_passes() {
        assert_eq!(std().classify("SELECT * FROM users WHERE id = 7"), SqlRisk::Safe);
        assert_eq!(std().classify("select id from users"), SqlRisk::Safe);
        assert_eq!(std().classify("SELECT * FROM users;"), SqlRisk::Safe);
        assert_eq!(std().classify("UPDATE users SET active = 0 WHERE id = 7"), SqlRisk::Safe);
        assert_eq!(std().classify("PRAGMA table_info(users)"), SqlRisk::Safe);
    }

    #[test]
    fn grant_revoke_denied() {
        assert!(std().classify("GRANT SELECT ON users TO app").is_denied());
        assert!(std().classify("grant all on db.* to 'app'@'%'").is_denied());
        assert!(std().classify("REVOKE DELETE ON users FROM role").is_denied());
        assert!(std().classify("revoke all on users from app").is_denied());
        assert!(matches!(SqlClassifier::permissive().classify("GRANT SELECT ON t TO r"), SqlRisk::Denied(_)));
    }

    #[test]
    fn update_no_where_review() {
        assert!(matches!(std().classify("UPDATE users SET active = 0"), SqlRisk::NeedsReview(_)));
        assert!(matches!(std().classify("update users set active=0"), SqlRisk::NeedsReview(_)));
        assert_eq!(std().classify("UPDATE users SET active = 0 WHERE id = 7"), SqlRisk::Safe);
    }

    #[test]
    fn pragma_read_safe_write_review() {
        assert_eq!(std().classify("PRAGMA table_info(users)"), SqlRisk::Safe);
        assert_eq!(std().classify("pragma journal_mode"), SqlRisk::Safe);
        assert!(matches!(std().classify("PRAGMA journal_mode=WAL"), SqlRisk::NeedsReview(_)));
        assert!(matches!(std().classify("pragma foreign_keys = ON"), SqlRisk::NeedsReview(_)));
        assert!(matches!(std().classify("PRAGMA wal_checkpoint(TRUNCATE)"), SqlRisk::NeedsReview(_)));
    }

    #[test]
    fn attach_database_denied() {
        assert!(std().classify("ATTACH DATABASE 'aux.db' AS aux").is_denied());
        assert!(std().classify("attach database '/tmp/x.db' as x").is_denied());
        assert!(std().classify("ATTACH 'aux.db' AS aux").is_denied());
        assert!(matches!(SqlClassifier::permissive().classify("ATTACH DATABASE 'a' AS a"), SqlRisk::Denied(_)));
    }

    #[test]
    fn index_ddl_review() {
        assert!(matches!(std().classify("CREATE INDEX ix ON users (email)"), SqlRisk::NeedsReview(_)));
        assert!(matches!(std().classify("CREATE UNIQUE INDEX ix ON users (email)"), SqlRisk::NeedsReview(_)));
        assert!(matches!(std().classify("DROP INDEX ix"), SqlRisk::NeedsReview(_)));
    }

    #[test]
    fn insert_select_and_multi_statement_review() {
        assert!(matches!(std().classify("INSERT INTO archive SELECT * FROM users"), SqlRisk::NeedsReview(_)));
        assert_eq!(std().classify("INSERT INTO users (id) VALUES (1)"), SqlRisk::Safe);
        assert!(matches!(std().classify("SELECT 1; SELECT 2"), SqlRisk::NeedsReview(_)));
        assert!(matches!(std().classify("SELECT 1; ATTACH DATABASE 'a' AS a"), SqlRisk::Denied(_)));
    }

    #[test]
    fn dot_command_denied() {
        assert!(std().classify(".import /tmp/users.csv users").is_denied());
        assert!(std().classify(".load /tmp/ext").is_denied());
        assert!(std().classify("  .IMPORT x y").is_denied());
    }

    #[test]
    fn strictness_gates_review() {
        let review = "UPDATE users SET active = 0";
        assert!(matches!(SqlClassifier::standard().classify(review), SqlRisk::NeedsReview(_)));
        assert!(matches!(SqlClassifier::strict().classify(review), SqlRisk::Denied(_)));
        assert_eq!(SqlClassifier::permissive().classify(review), SqlRisk::Safe);
    }
}
