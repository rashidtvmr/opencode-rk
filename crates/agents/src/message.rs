//! Message struct, role enum, and in-memory message store for agent conversations.
#![forbid(unsafe_code)]

use chrono::Utc;
use opencode_rk_contracts::{MessageId, Timestamp};
use std::collections::HashMap;

/// The role of a message in a conversation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A single message in a conversation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: Timestamp,
    pub metadata: HashMap<String, String>,
}

impl Message {
    /// Create a new message with the given role and content.
    /// Metadata defaults to empty.
    #[must_use]
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role,
            content: content.into(),
            timestamp: Timestamp::now(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new message with metadata.
    #[must_use]
    pub fn with_metadata(
        role: MessageRole,
        content: impl Into<String>,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self {
            id: MessageId::new(),
            role,
            content: content.into(),
            timestamp: Timestamp::now(),
            metadata,
        }
    }
}

/// In-memory ordered collection of messages with indexing support.
#[derive(Debug, Default, Clone)]
pub struct MessageStore {
    messages: Vec<Message>,
    index: HashMap<String, usize>,
}

impl MessageStore {
    /// Create an empty message store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a message to the store.
    pub fn append(&mut self, msg: Message) {
        let id_str = msg.id.to_string();
        if let Some(pos) = self.index.get(&id_str) {
            self.messages[*pos] = msg;
        } else {
            self.index.insert(id_str, self.messages.len());
            self.messages.push(msg);
        }
    }

    /// Retrieve a message by its ID.
    #[must_use]
    pub fn get(&self, id: MessageId) -> Option<&Message> {
        self.index
            .get(&id.to_string())
            .and_then(|&idx| self.messages.get(idx))
    }

    /// Find all messages with the given role.
    #[must_use]
    pub fn find_by_role(&self, role: MessageRole) -> Vec<&Message> {
        self.messages
            .iter()
            .filter(|m| m.role == role)
            .collect()
    }

    /// Return the last `n` messages (or fewer if store has less).
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<&Message> {
        let start = self.messages.len().saturating_sub(n);
        self.messages[start..].iter().collect()
    }

    /// Total number of messages in the store.
    #[must_use]
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Returns true if the store contains no messages.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn append_and_get() {
        let mut store = MessageStore::new();
        let msg = Message::new(MessageRole::User, "hello world");
        let id = msg.id;
        store.append(msg);
        let fetched = store.get(id).expect("message should exist");
        assert_eq!(fetched.content, "hello world");
        assert_eq!(fetched.role, MessageRole::User);
        assert!(fetched.metadata.is_empty());
    }

    #[test]
    fn find_by_role() {
        let mut store = MessageStore::new();
        store.append(Message::new(MessageRole::System, "system prompt"));
        store.append(Message::new(MessageRole::User, "user question"));
        store.append(Message::new(MessageRole::Assistant, "assistant reply"));
        store.append(Message::new(MessageRole::User, "follow-up"));

        let users: Vec<&Message> = store.find_by_role(MessageRole::User);
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].content, "user question");
        assert_eq!(users[1].content, "follow-up");

        let system: Vec<&Message> = store.find_by_role(MessageRole::System);
        assert_eq!(system.len(), 1);
        assert_eq!(system[0].content, "system prompt");

        let tool: Vec<&Message> = store.find_by_role(MessageRole::Tool);
        assert!(tool.is_empty());
    }

    #[test]
    fn recent_returns_last_n() {
        let mut store = MessageStore::new();
        for i in 0..5 {
            store.append(Message::new(MessageRole::Assistant, format!("msg{i}")));
        }

        let recent = store.recent(3);
        assert_eq!(recent.len(), 3);
        let contents: Vec<&str> = recent.iter().map(|m| m.content.as_str()).collect();
        assert_eq!(contents, vec!["msg2", "msg3", "msg4"]);

        // Requesting more than available returns all
        let all = store.recent(10);
        assert_eq!(all.len(), 5);

        // Requesting zero returns nothing
        let empty = store.recent(0);
        assert!(empty.is_empty());
    }

    #[test]
    fn empty_store() {
        let store = MessageStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert!(store.recent(5).is_empty());

        // get on empty store returns None
        let id = MessageId::new();
        assert!(store.get(id).is_none());
    }

    #[test]
    fn message_timestamp() {
        let msg = Message::new(MessageRole::User, "timed message");
        let before = Utc::now();
        // Timestamp::now() is set during construction; verify it's close to "now"
        let ts_after = msg.timestamp.as_datetime();
        let ts_before = before;
        assert!(ts_after >= ts_before - chrono::Duration::milliseconds(100));
        assert!(ts_after <= Utc::now() + chrono::Duration::milliseconds(100));
    }
}