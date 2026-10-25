use crate::{Storage, StorageError};
use opencode_rk_contracts::SessionId;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

const MAX_POLICY_BYTES: i64 = 1024;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompactionStrategy {
    #[default]
    Auto,
    Native,
    Fallback,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionCompactionSettings {
    pub enabled: bool,
    pub threshold_tokens: u64,
    pub keep_recent_turns: usize,
    pub strategy: CompactionStrategy,
}

impl Default for SessionCompactionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_tokens: 100_000,
            keep_recent_turns: 20,
            strategy: CompactionStrategy::Auto,
        }
    }
}

impl SessionCompactionSettings {
    pub fn validate(&self) -> Result<(), StorageError> {
        if !(1..=10_000_000).contains(&self.threshold_tokens)
            || !(1..=1000).contains(&self.keep_recent_turns)
        {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        Ok(())
    }
}

impl Storage {
    pub fn session_compaction_settings(
        &self,
        id: SessionId,
    ) -> Result<Option<SessionCompactionSettings>, StorageError> {
        let conn = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM sessions WHERE id=?1 LIMIT 1",
                params![id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StorageError::SessionNotFound(id));
        }
        let length: Option<i64> = conn.query_row("SELECT length(CAST(policy_json AS BLOB)) FROM session_compaction_settings WHERE session_id=?1 LIMIT 1", params![id.to_string()], |r| r.get(0)).optional()?;
        let Some(length) = length else {
            return Ok(None);
        };
        if !(1..=MAX_POLICY_BYTES).contains(&length) {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        conn.query_row("SELECT policy_json FROM session_compaction_settings WHERE session_id=?1 AND length(CAST(policy_json AS BLOB)) BETWEEN 1 AND 1024 LIMIT 1", params![id.to_string()], |r| {
            let json: String = r.get(0)?;
            let settings: SessionCompactionSettings = serde_json::from_str(&json).map_err(|_| rusqlite::Error::InvalidQuery)?;
            settings.validate().map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok(settings)
        }).optional().map_err(StorageError::from)
    }

    pub fn set_session_compaction_settings(
        &self,
        id: SessionId,
        settings: &SessionCompactionSettings,
    ) -> Result<(), StorageError> {
        settings.validate()?;
        let json = serde_json::to_string(settings).map_err(|_| rusqlite::Error::InvalidQuery)?;
        if json.len() > MAX_POLICY_BYTES as usize {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        let mut conn = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM sessions WHERE id=?1 LIMIT 1",
                params![id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StorageError::SessionNotFound(id));
        }
        tx.execute("INSERT INTO session_compaction_settings(session_id,policy_json) VALUES(?1,?2) ON CONFLICT(session_id) DO UPDATE SET policy_json=excluded.policy_json", params![id.to_string(), json])?;
        tx.commit()?;
        Ok(())
    }
}
