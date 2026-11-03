//! Ordered, bounded todo collections in the active server storage ledger.
use crate::{Storage, StorageError};
use opencode_rk_contracts::{MessageId, SessionId};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_TODOS: usize = 100;
/// Leave envelope space below the 64 KiB todo publication budget.
pub const MAX_TODO_COLLECTION_BYTES: usize = 60 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoPriority {
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Todo {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: TodoPriority,
}

pub fn validate(todos: &[Todo]) -> Result<(), StorageError> {
    let mut ids = HashSet::new();
    if todos.len() > MAX_TODOS {
        return Err(StorageError::InvalidTodo);
    }
    for todo in todos {
        if todo.id.is_empty()
            || todo.id.len() > 128
            || todo.id.chars().any(char::is_control)
            || todo.content.trim().is_empty()
            || todo.content.len() > 4096
            || !ids.insert(&todo.id)
        {
            return Err(StorageError::InvalidTodo);
        }
        // JSON escaping can increase byte length: bound the stored encoding,
        // rather than assuming the source string alone bounds the SQL value.
        if serde_json::to_vec(todo)
            .map_err(|_| StorageError::InvalidTodo)?
            .len()
            > 8192
        {
            return Err(StorageError::InvalidTodo);
        }
    }
    if serde_json::to_vec(todos)
        .map_err(|_| StorageError::InvalidTodo)?
        .len()
        > MAX_TODO_COLLECTION_BYTES
    {
        return Err(StorageError::InvalidTodo);
    }
    Ok(())
}

impl Storage {
    pub fn session_todos(&self, session: SessionId) -> Result<Vec<Todo>, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let exists = connection
            .query_row(
                "SELECT 1 FROM sessions WHERE id=?1 LIMIT 1",
                [session.to_string()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(StorageError::SessionNotFound(session));
        }
        let mut statement = connection.prepare(
            "SELECT payload FROM session_todos WHERE session_id=?1 ORDER BY ordinal LIMIT 100",
        )?;
        let rows = statement.query_map([session.to_string()], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let payload = row?;
            serde_json::from_str(&payload).map_err(|_| StorageError::InvalidTodo)
        })
        .collect()
    }

    pub fn replace_session_todos(
        &self,
        session: SessionId,
        todos: &[Todo],
    ) -> Result<(), StorageError> {
        validate(todos)?;
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists = tx
            .query_row(
                "SELECT 1 FROM sessions WHERE id=?1 LIMIT 1",
                [session.to_string()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(StorageError::SessionNotFound(session));
        }
        tx.execute(
            "DELETE FROM session_todos WHERE session_id=?1",
            [session.to_string()],
        )?;
        for (ordinal, todo) in todos.iter().enumerate() {
            let payload = serde_json::to_string(todo).map_err(|_| StorageError::InvalidTodo)?;
            tx.execute(
                "INSERT INTO session_todos(session_id,ordinal,payload) VALUES(?1,?2,?3)",
                params![session.to_string(), ordinal as i64, payload],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Derive the todo list from typed tool history, mirroring upstream
    /// `sessionTodos` (`packages/tui/src/util/todo.ts:32-43`): the input of the
    /// latest COMPLETED `todowrite` call strictly before the revert boundary.
    /// `boundary = None` or an unknown boundary message means the whole history.
    /// Returns `Ok(vec![])` when no completed todowrite call exists.
    pub fn session_todos_at(
        &self,
        session: SessionId,
        boundary: Option<MessageId>,
    ) -> Result<Vec<Todo>, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let exists = connection
            .query_row(
                "SELECT 1 FROM sessions WHERE id=?1 LIMIT 1",
                [session.to_string()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(StorageError::SessionNotFound(session));
        }
        // Upstream uses a not-found boundary as "include every message".
        let boundary_rowid: Option<i64> = match boundary {
            Some(id) => connection
                .query_row(
                    "SELECT rowid FROM messages WHERE id=?1 AND session_id=?2 LIMIT 1",
                    params![id.to_string(), session.to_string()],
                    |row| row.get(0),
                )
                .optional()?,
            None => None,
        };
        let (has_boundary, bound) = match boundary_rowid {
            Some(rowid) => (1i64, rowid),
            None => (0i64, 0i64),
        };
        let payload: Option<String> = connection
            .query_row(
                "SELECT t.payload FROM typed_tool_records t
                 JOIN messages m ON m.id=t.message_id
                 WHERE m.session_id=?1 AND t.kind='call' AND t.name='todowrite'
                   AND EXISTS (SELECT 1 FROM typed_tool_records o
                               WHERE o.round_id=t.round_id AND o.pair_index=t.pair_index AND o.kind='output')
                   AND (?2=0 OR m.rowid < ?3)
                 ORDER BY m.rowid DESC, t.pair_index DESC LIMIT 1",
                params![session.to_string(), has_boundary, bound],
                |row| row.get(0),
            )
            .optional()?;
        let Some(payload) = payload else {
            return Ok(Vec::new());
        };
        // Malformed history degrades to an empty list, matching upstream
        // `parseTodos` (`util/todo.ts:19-24`).
        let todos = serde_json::from_str::<serde_json::Value>(&payload)
            .ok()
            .and_then(|value| value.get("todos").cloned())
            .and_then(|todos| serde_json::from_value::<Vec<Todo>>(todos).ok())
            .unwrap_or_default();
        Ok(todos)
    }
}
