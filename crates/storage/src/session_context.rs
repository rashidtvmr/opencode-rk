use crate::{HistoryItem, Storage, StorageError};
use opencode_rk_contracts::{MessageId, SessionId};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContextSelection {
    pub messages: Vec<MessageId>,
    pub checkpoints: Vec<MessageId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextToolPreview {
    pub kind: String,
    pub name: String,
    pub call_id: String,
    pub pair_index: u64,
    pub payload: String,
    pub bytes: u64,
    pub truncated: bool,
}

impl Storage {
    pub fn context_tool_preview(
        &self,
        session: SessionId,
        message: MessageId,
    ) -> Result<Vec<ContextToolPreview>, StorageError> {
        let conn = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let role: Option<String> = conn
            .query_row(
                "SELECT role FROM messages WHERE session_id=?1 AND id=?2 LIMIT 1",
                params![session.to_string(), message.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        match role.as_deref() {
            None => return Err(StorageError::MessageNotFound(message)),
            Some("tool") => {}
            _ => return Err(StorageError::TypedHistoryIdentity),
        }
        let mut statement = conn.prepare(
            "SELECT kind,name,call_id,pair_index,substr(CAST(payload AS BLOB),1,4096),byte_len
             FROM typed_tool_records WHERE message_id=?1
               AND length(CAST(name AS BLOB)) BETWEEN 1 AND 8192
               AND length(CAST(call_id AS BLOB)) BETWEEN 1 AND 8192
               AND byte_len=length(CAST(payload AS BLOB))
             ORDER BY CASE kind WHEN 'call' THEN 0 ELSE 1 END LIMIT 3",
        )?;
        let rows = statement.query_map(params![message.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;
        let mut output = Vec::new();
        for row in rows {
            let (kind, name, call_id, pair_index, payload, bytes) = row?;
            let bytes = u64::try_from(bytes).map_err(|_| StorageError::TypedHistoryIdentity)?;
            output.push(ContextToolPreview {
                kind,
                name,
                call_id,
                pair_index: u64::try_from(pair_index)
                    .map_err(|_| StorageError::TypedHistoryIdentity)?,
                truncated: bytes > payload.len() as u64,
                payload: String::from_utf8_lossy(&payload).into_owned(),
                bytes,
            });
        }
        if output.len() != 2
            || output[0].kind != "call"
            || output[1].kind != "output"
            || output[0].name != output[1].name
            || output[0].call_id != output[1].call_id
            || output[0].pair_index != output[1].pair_index
        {
            return Err(StorageError::TypedHistoryIncomplete);
        }
        Ok(output)
    }
    pub fn context_selection(&self, id: SessionId) -> Result<ContextSelection, StorageError> {
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
        let mut out = ContextSelection::default();
        let mut q = conn.prepare("SELECT item_kind,item_id FROM session_context_exclusions WHERE session_id=?1 ORDER BY item_kind,item_id LIMIT 257")?;
        let mut count = 0usize;
        for row in q.query_map(params![id.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            count += 1;
            if count > 256 {
                return Err(StorageError::TypedHistoryLimit);
            }
            let (kind, raw) = row?;
            let parsed: MessageId = raw
                .parse()
                .map_err(|_| StorageError::TypedHistoryIdentity)?;
            if raw != parsed.to_string() {
                return Err(StorageError::TypedHistoryIdentity);
            }
            match kind.as_str() {
                "message" => {
                    let role: Option<String> = conn
                        .query_row(
                            "SELECT role FROM messages WHERE session_id=?1 AND id=?2 LIMIT 1",
                            params![id.to_string(), raw],
                            |r| r.get(0),
                        )
                        .optional()?;
                    match role {
                        Some(r) if r != "system" => out.messages.push(parsed),
                        Some(_) => return Err(StorageError::TypedHistoryIdentity),
                        None => return Err(StorageError::TypedHistoryIdentity),
                    }
                }
                "checkpoint" => {
                    let valid: Option<i64> = conn.query_row("SELECT 1 FROM session_compaction_checkpoints c JOIN messages m ON m.id=c.boundary_message_id AND m.session_id=c.session_id WHERE c.session_id=?1 AND c.boundary_message_id=?2 LIMIT 1", params![id.to_string(), raw], |r| r.get(0)).optional()?;
                    if valid.is_none() {
                        return Err(StorageError::TypedHistoryIdentity);
                    }
                    out.checkpoints.push(parsed)
                }
                _ => return Err(StorageError::TypedHistoryIdentity),
            }
        }
        Ok(out)
    }

    pub fn set_context_included(
        &self,
        session: SessionId,
        item: MessageId,
        checkpoint: bool,
        included: bool,
    ) -> Result<(), StorageError> {
        let mut conn = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let present: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM sessions WHERE id=?1 LIMIT 1",
                params![session.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if present.is_none() {
            return Err(StorageError::SessionNotFound(session));
        }
        let kind = if checkpoint { "checkpoint" } else { "message" };
        if checkpoint {
            let valid: Option<i64> = tx.query_row("SELECT 1 FROM session_compaction_checkpoints c JOIN messages m ON m.id=c.boundary_message_id WHERE c.session_id=?1 AND m.session_id=?1 AND m.id=?2 LIMIT 1", params![session.to_string(), item.to_string()], |r| r.get(0)).optional()?;
            if valid.is_none() {
                return Err(StorageError::MessageNotFound(item));
            }
        } else {
            let role: Option<String> = tx
                .query_row(
                    "SELECT role FROM messages WHERE session_id=?1 AND id=?2 LIMIT 1",
                    params![session.to_string(), item.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            match role {
                None => return Err(StorageError::MessageNotFound(item)),
                Some(r) if r == "system" => return Err(StorageError::TypedHistoryIdentity),
                _ => {}
            }
        }
        if included {
            tx.execute("DELETE FROM session_context_exclusions WHERE session_id=?1 AND item_kind=?2 AND item_id=?3", params![session.to_string(), kind, item.to_string()])?;
        } else {
            if checkpoint {
                tx.execute("DELETE FROM session_context_exclusions WHERE session_id=?1 AND item_kind='checkpoint' AND item_id<>?2", params![session.to_string(), item.to_string()])?;
            }
            let already: Option<i64> = tx.query_row("SELECT 1 FROM session_context_exclusions WHERE session_id=?1 AND item_kind=?2 AND item_id=?3 LIMIT 1", params![session.to_string(), kind, item.to_string()], |r| r.get(0)).optional()?;
            if already.is_some() {
                tx.commit()?;
                return Ok(());
            }
            let n: i64 = tx.query_row(
                "SELECT count(*) FROM session_context_exclusions WHERE session_id=?1",
                params![session.to_string()],
                |r| r.get(0),
            )?;
            if n >= 256 {
                return Err(StorageError::TypedHistoryLimit);
            }
            tx.execute("INSERT OR IGNORE INTO session_context_exclusions(session_id,item_kind,item_id) VALUES(?1,?2,?3)", params![session.to_string(), kind, item.to_string()])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn bounded_context_history(
        &self,
        id: SessionId,
        boundary: Option<MessageId>,
        items: usize,
        bytes: usize,
    ) -> Result<Vec<HistoryItem>, StorageError> {
        self.bounded_history_from(id, boundary, items, bytes, true)
    }

    pub fn context_message_compacted(
        &self,
        session: SessionId,
        message: MessageId,
        boundary: MessageId,
    ) -> Result<bool, StorageError> {
        let conn = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let m: i64 = conn
            .query_row(
                "SELECT rowid FROM messages WHERE session_id=?1 AND id=?2 LIMIT 1",
                params![session.to_string(), message.to_string()],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StorageError::MessageNotFound(message))?;
        let b: i64 = conn
            .query_row(
                "SELECT rowid FROM messages WHERE session_id=?1 AND id=?2 LIMIT 1",
                params![session.to_string(), boundary.to_string()],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StorageError::MessageNotFound(boundary))?;
        Ok(m <= b)
    }
}
