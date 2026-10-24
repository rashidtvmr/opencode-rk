//! Ordered, bounded todo collections in the active server storage ledger.
use crate::{Storage, StorageError};
use opencode_rk_contracts::SessionId;
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
}
