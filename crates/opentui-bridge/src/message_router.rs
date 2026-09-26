#![forbid(unsafe_code)]
//! Message action router (mirrors `dialog-message.tsx` Revert/Copy/Fork menu).
//!
//! TS: `DialogMessage` offers per-message actions via `DialogSelect`;
//! this type queues one pending `MessageAction` per message id.

/// Max message id chars (mirrors `session_message_dialog::MAX_MESSAGE_ID`).
pub const MAX_MESSAGE_ID: usize = 64;

/// Per-message actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageAction {
    Reply,
    Edit,
    Delete,
    Copy,
}

/// Static label per action.
pub fn action_label(a: MessageAction) -> &'static str {
    match a {
        MessageAction::Reply => "Reply",
        MessageAction::Edit => "Edit",
        MessageAction::Delete => "Delete",
        MessageAction::Copy => "Copy",
    }
}

/// Routes one pending action for a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRoute {
    pub message_id: String,
    pub pending: Option<MessageAction>,
}

impl MessageRoute {
    /// New route; id truncated to 64 chars.
    pub fn new(id: &str) -> Self {
        Self {
            message_id: id.chars().take(MAX_MESSAGE_ID).collect(),
            pending: None,
        }
    }

    /// Queue (overwrite) a pending action.
    pub fn request(&mut self, a: MessageAction) {
        self.pending = Some(a);
    }

    /// Take the pending action, leaving none.
    pub fn take(&mut self) -> Option<MessageAction> {
        self.pending.take()
    }

    /// True when an action is queued.
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_non_empty() {
        for a in [
            MessageAction::Reply,
            MessageAction::Edit,
            MessageAction::Delete,
            MessageAction::Copy,
        ] {
            assert!(!action_label(a).is_empty());
        }
    }

    #[test]
    fn request_take_roundtrip() {
        let mut r = MessageRoute::new("m1");
        assert!(!r.has_pending());
        r.request(MessageAction::Reply);
        assert!(r.has_pending());
        assert_eq!(r.take(), Some(MessageAction::Reply));
        assert!(!r.has_pending());
    }

    #[test]
    fn request_overwrites() {
        let mut r = MessageRoute::new("m1");
        r.request(MessageAction::Edit);
        r.request(MessageAction::Delete);
        assert_eq!(r.take(), Some(MessageAction::Delete));
    }

    #[test]
    fn empty_take_is_none() {
        let mut r = MessageRoute::new("m1");
        assert_eq!(r.take(), None);
    }

    #[test]
    fn id_truncates_64() {
        let long = "x".repeat(MAX_MESSAGE_ID + 8);
        let r = MessageRoute::new(&long);
        assert_eq!(r.message_id.chars().count(), MAX_MESSAGE_ID);
    }

    #[test]
    fn labels_distinct() {
        assert_ne!(
            action_label(MessageAction::Reply),
            action_label(MessageAction::Copy)
        );
    }
}
