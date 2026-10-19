//! Bounded presentation history. Roles come from backend/event facts; local
//! notices are explicitly system messages, never parsed from text prefixes.
#![cfg(feature = "native")]

use crate::native_view_chat::{ChatMessage, ChatPart, ChatRole};
use std::ops::{Deref, DerefMut};

const MAX_BYTES: usize = 64 * 1024;
const MAX_MESSAGES: usize = 200;
const MAX_CHARS: usize = 1024;

#[derive(Default)]
pub struct NativeChatHistory {
    pub lines: Vec<String>,
    pub messages: Vec<ChatMessage>,
    partial: Option<(usize, usize)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_removal_uses_identity_not_provider_text() {
        let mut history = NativeChatHistory::default();
        history.append(
            "assistant (streaming): literal user text".into(),
            ChatMessage::text(ChatRole::User, "assistant (streaming): literal user text"),
        );
        history.replace_partial("first");
        history.notice("permission: awaiting acknowledgement".into());
        history.replace_partial("second");
        history.remove_partial();
        assert_eq!(
            history.lines,
            [
                "assistant (streaming): literal user text",
                "permission: awaiting acknowledgement"
            ]
        );
        assert_eq!(history.messages[0].role, ChatRole::User);
        assert_eq!(history.messages[1].role, ChatRole::System);
    }

    #[test]
    fn notices_and_streams_share_finite_history_budget() {
        let mut history = NativeChatHistory::default();
        for _ in 0..1000 {
            history.notice("界".repeat(2048));
        }
        history.replace_partial("tail");
        assert!(history.lines.len() <= MAX_MESSAGES);
        assert!(history.messages.len() <= MAX_MESSAGES);
        assert!(history.lines.iter().map(String::len).sum::<usize>() <= MAX_BYTES);
        assert!(history.messages.iter().map(message_bytes).sum::<usize>() <= MAX_BYTES);
        history.remove_partial();
        assert!(history
            .messages
            .iter()
            .all(|message| message.role == ChatRole::System));
    }
}

impl Deref for NativeChatHistory {
    type Target = Vec<String>;
    fn deref(&self) -> &Self::Target {
        &self.lines
    }
}
impl DerefMut for NativeChatHistory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lines
    }
}

fn bounded(text: &str) -> String {
    text.chars().take(MAX_CHARS).collect()
}
fn message_bytes(message: &ChatMessage) -> usize {
    message
        .parts
        .iter()
        .map(|part| match part {
            ChatPart::Text(text) | ChatPart::Thinking(text) => text.len(),
            ChatPart::ToolCall { name, id } => name.len() + id.len(),
            ChatPart::ToolResult { name, output, .. } => name.len() + output.len(),
        })
        .sum::<usize>()
        + message.agent.as_ref().map_or(0, String::len)
        + message.model.as_ref().map_or(0, String::len)
}

impl NativeChatHistory {
    pub fn reset(&mut self, lines: Vec<String>, messages: Vec<ChatMessage>) {
        self.lines = lines;
        self.messages.clear();
        self.partial = None;
        for message in messages {
            self.push_message(message);
        }
        self.prune_lines();
    }

    pub fn notice(&mut self, line: String) {
        let message = ChatMessage::text(ChatRole::System, bounded(&line));
        self.append(line, message);
    }

    pub fn append(&mut self, line: String, message: ChatMessage) {
        self.lines.push(bounded(&line));
        self.push_message(message);
        self.prune_lines();
    }

    fn push_message(&mut self, mut message: ChatMessage) {
        message.parts.truncate(128);
        message.agent = message.agent.map(|v| bounded(&v));
        message.model = message.model.map(|v| bounded(&v));
        for part in &mut message.parts {
            match part {
                ChatPart::Text(text) | ChatPart::Thinking(text) => *text = bounded(text),
                ChatPart::ToolCall { name, id } => {
                    *name = bounded(name);
                    *id = bounded(id);
                }
                ChatPart::ToolResult { name, output, .. } => {
                    *name = bounded(name);
                    *output = bounded(output);
                }
            }
        }
        self.messages.push(message);
        let mut bytes: usize = self.messages.iter().map(message_bytes).sum();
        while bytes > MAX_BYTES || self.messages.len() > MAX_MESSAGES {
            bytes = bytes.saturating_sub(message_bytes(&self.messages.remove(0)));
            self.partial = self
                .partial
                .and_then(|(line, index)| index.checked_sub(1).map(|index| (line, index)));
        }
    }

    fn prune_lines(&mut self) {
        let mut bytes: usize = self.lines.iter().map(String::len).sum();
        while bytes > MAX_BYTES || self.lines.len() > MAX_MESSAGES {
            bytes = bytes.saturating_sub(self.lines.remove(0).len());
            self.partial = self
                .partial
                .and_then(|(index, message)| index.checked_sub(1).map(|index| (index, message)));
        }
    }

    pub fn remove_partial(&mut self) {
        if let Some((line, message)) = self.partial.take() {
            if line < self.lines.len() {
                self.lines.remove(line);
            }
            if message < self.messages.len() {
                self.messages.remove(message);
            }
        }
    }

    pub fn replace_partial(&mut self, text: &str) {
        self.remove_partial();
        self.append(
            format!("assistant (streaming): {}", bounded(text)),
            ChatMessage::text(ChatRole::Assistant, bounded(text)),
        );
        if !self.lines.is_empty() && !self.messages.is_empty() {
            self.partial = Some((self.lines.len() - 1, self.messages.len() - 1));
        }
    }
}
