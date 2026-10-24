//! Durable, bounded per-session MCP server exclusions.
use crate::{Storage, StorageError};
use opencode_rk_contracts::SessionId;
use rusqlite::{params, OptionalExtension, TransactionBehavior};

pub const MAX_MCP_EXCLUSIONS: usize = 128;
pub const MAX_MCP_SERVER_NAME_BYTES: usize = 64;

fn valid_name(name: &str) -> bool { !name.is_empty() && name.len() <= MAX_MCP_SERVER_NAME_BYTES && !name.chars().any(char::is_control) }

impl Storage {
    pub fn mcp_exclusions(&self, session: SessionId) -> Result<Vec<String>, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        if connection.query_row("SELECT 1 FROM sessions WHERE id=?1", [session.to_string()], |_| Ok(())).optional()?.is_none() { return Err(StorageError::SessionNotFound(session)); }
        let mut statement = connection.prepare("SELECT server_name FROM session_mcp_exclusions WHERE session_id=?1 ORDER BY server_name LIMIT 128")?;
        let exclusions = statement.query_map([session.to_string()], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(exclusions)
    }
    pub fn set_mcp_included(&self, session: SessionId, server: &str, included: bool) -> Result<(), StorageError> {
        if !valid_name(server) { return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery)); }
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.query_row("SELECT 1 FROM sessions WHERE id=?1", [session.to_string()], |_| Ok(())).optional()?.is_none() { return Err(StorageError::SessionNotFound(session)); }
        if included { tx.execute("DELETE FROM session_mcp_exclusions WHERE session_id=?1 AND server_name=?2", params![session.to_string(), server])?; }
        else {
            let exists = tx.query_row("SELECT 1 FROM session_mcp_exclusions WHERE session_id=?1 AND server_name=?2", params![session.to_string(), server], |_| Ok(())).optional()?.is_some();
            let count: i64 = tx.query_row("SELECT COUNT(*) FROM session_mcp_exclusions WHERE session_id=?1", [session.to_string()], |row| row.get(0))?;
            if !exists && count >= MAX_MCP_EXCLUSIONS as i64 { return Err(StorageError::InlinePayloadTooLarge); }
            tx.execute("INSERT OR IGNORE INTO session_mcp_exclusions(session_id,server_name) VALUES(?1,?2)", params![session.to_string(), server])?;
        }
        tx.commit()?; Ok(())
    }
}
