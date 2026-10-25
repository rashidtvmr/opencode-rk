//! Durable, provider-opaque native compaction checkpoints.
use crate::{Storage, StorageError};
use opencode_rk_contracts::{MessageId, SessionId, Timestamp};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

pub const MAX_WINDOW_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactionCheckpoint {
    pub boundary_message_id: MessageId,
    pub provider_id: String,
    pub model_id: String,
    pub window_json: String,
}

impl Storage {
    pub fn compaction_checkpoint(&self, session_id: SessionId) -> Result<Option<CompactionCheckpoint>, StorageError> {
        let conn = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let sizes: Option<(i64, i64, i64, i64, i64)> = conn.query_row(
            "SELECT length(CAST(boundary_message_id AS BLOB)),length(CAST(provider_id AS BLOB)),length(CAST(model_id AS BLOB)),length(CAST(window_json AS BLOB)),window_bytes FROM session_compaction_checkpoints WHERE session_id=?1 LIMIT 1",
            params![session_id.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        ).optional()?;
        let Some((boundary, provider, model, window, declared)) = sizes else { return Ok(None); };
        if boundary != 36 || !(1..=256).contains(&provider) || !(1..=256).contains(&model)
            || window <= 0 || window > MAX_WINDOW_BYTES as i64 || window != declared {
            return Err(StorageError::TypedHistoryLimit);
        }
        conn.query_row("SELECT boundary_message_id,provider_id,model_id,window_json FROM session_compaction_checkpoints WHERE session_id=?1 AND length(CAST(boundary_message_id AS BLOB))=36 AND length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 256 AND length(CAST(model_id AS BLOB)) BETWEEN 1 AND 256 AND length(CAST(window_json AS BLOB)) BETWEEN 1 AND 1048576 AND window_bytes=length(CAST(window_json AS BLOB)) LIMIT 1", params![session_id.to_string()], |r| {
            let boundary: String = r.get(0)?;
            Ok(CompactionCheckpoint {
                boundary_message_id: boundary.parse().map_err(|_| rusqlite::Error::InvalidQuery)?,
                provider_id: r.get(1)?, model_id: r.get(2)?, window_json: r.get(3)?
            })
        }).map(Some).map_err(StorageError::from)
    }

    /// Replace the one checkpoint only if the captured boundary is still the
    /// newest durable message. The check and write share one SQLite transaction.
    pub fn save_compaction_checkpoint(&self, session_id: SessionId, checkpoint: &CompactionCheckpoint) -> Result<(), StorageError> {
        if checkpoint.window_json.is_empty() || checkpoint.window_json.len() > MAX_WINDOW_BYTES
            || checkpoint.provider_id.is_empty() || checkpoint.provider_id.len() > 256
            || checkpoint.model_id.is_empty() || checkpoint.model_id.len() > 256 {
            return Err(StorageError::TypedHistoryLimit);
        }
        let mut conn = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let belongs: Option<i64> = tx.query_row("SELECT rowid FROM messages WHERE session_id=?1 AND id=?2", params![session_id.to_string(), checkpoint.boundary_message_id.to_string()], |r| r.get(0)).optional()?;
        let boundary_rowid = belongs.ok_or(StorageError::MessageNotFound(checkpoint.boundary_message_id))?;
        let head: Option<i64> = tx.query_row("SELECT rowid FROM messages WHERE session_id=?1 ORDER BY rowid DESC LIMIT 1", params![session_id.to_string()], |r| r.get(0)).optional()?;
        if head != Some(boundary_rowid) { return Err(StorageError::TypedHistoryIdentity); }
        tx.execute("INSERT INTO session_compaction_checkpoints(session_id,boundary_message_id,provider_id,model_id,window_json,window_bytes,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(session_id) DO UPDATE SET boundary_message_id=excluded.boundary_message_id,provider_id=excluded.provider_id,model_id=excluded.model_id,window_json=excluded.window_json,window_bytes=excluded.window_bytes,created_at=excluded.created_at", params![session_id.to_string(), checkpoint.boundary_message_id.to_string(), checkpoint.provider_id, checkpoint.model_id, checkpoint.window_json, checkpoint.window_json.len() as i64, Timestamp::now().to_string()])?;
        // An exclusion identifies the snapshot that the user inspected, not
        // every future checkpoint for this session. Reconcile it atomically
        // with successful replacement; message exclusions remain untouched.
        tx.execute("DELETE FROM session_context_exclusions WHERE session_id=?1 AND item_kind='checkpoint'", params![session_id.to_string()])?;
        tx.commit()?;
        Ok(())
    }
}
