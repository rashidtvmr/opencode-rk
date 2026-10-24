//! Durable per-session instruction exclusions. Source contents stay on disk.
use crate::{Storage, StorageError};
use opencode_rk_contracts::SessionId;
use rusqlite::{params, OptionalExtension, TransactionBehavior};

impl Storage {
    pub fn memory_exclusions(&self, session: SessionId) -> Result<Vec<String>, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        if connection
            .query_row(
                "SELECT 1 FROM sessions WHERE id=?1 LIMIT 1",
                [session.to_string()],
                |_| Ok(()),
            )
            .optional()?
            .is_none()
        {
            return Err(StorageError::SessionNotFound(session));
        }
        let mut statement = connection.prepare(
            "SELECT path FROM session_memory_exclusions WHERE session_id=?1 ORDER BY path LIMIT 64",
        )?;
        let paths = statement
            .query_map([session.to_string()], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(paths)
    }

    pub fn set_memory_included(
        &self,
        session: SessionId,
        path: &str,
        included: bool,
    ) -> Result<(), StorageError> {
        if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx
            .query_row(
                "SELECT 1 FROM sessions WHERE id=?1 LIMIT 1",
                [session.to_string()],
                |_| Ok(()),
            )
            .optional()?
            .is_none()
        {
            return Err(StorageError::SessionNotFound(session));
        }
        if included {
            tx.execute(
                "DELETE FROM session_memory_exclusions WHERE session_id=?1 AND path=?2",
                params![session.to_string(), path],
            )?;
        } else {
            let count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM session_memory_exclusions WHERE session_id=?1",
                [session.to_string()],
                |row| row.get(0),
            )?;
            let present = tx.query_row("SELECT 1 FROM session_memory_exclusions WHERE session_id=?1 AND path=?2 LIMIT 1", params![session.to_string(), path], |_| Ok(())).optional()?.is_some();
            if !present && count >= 64 {
                return Err(StorageError::InlinePayloadTooLarge);
            }
            tx.execute(
                "INSERT OR IGNORE INTO session_memory_exclusions(session_id,path) VALUES(?1,?2)",
                params![session.to_string(), path],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
