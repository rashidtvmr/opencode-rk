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
    reasoning_partial: Option<(usize, usize)>,
    reasoning: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thinking(history:&NativeChatHistory)->Vec<&str> {
        history.messages.iter().flat_map(|message|message.parts.iter()).filter_map(|part|if let ChatPart::Thinking(text)=part{Some(text.as_str())}else{None}).collect()
    }

    #[test]
    fn reasoning_replay_coalesces_and_replaces_in_place_across_interleaved_rows() {
        let mut history=NativeChatHistory::default();history.begin_reasoning_turn();
        history.reasoning_event("a ",false);history.reasoning_event("b",false);
        assert_eq!(thinking(&history),vec!["a b"]);
        history.notice("permission notice".into());
        history.append("tool result".into(),ChatMessage::text(ChatRole::Tool,"tool result"));
        history.replace_partial("assistant text");
        history.reasoning_event("authoritative summary",true);
        history.reasoning_event("authoritative summary",true);
        assert_eq!(thinking(&history),vec!["authoritative summary"]);
        assert!(matches!(&history.messages[0].parts[0],ChatPart::Thinking(text) if text=="authoritative summary"));
        assert_eq!(history.messages[1].role,ChatRole::System);assert_eq!(history.messages[2].role,ChatRole::Tool);
        assert!(matches!(&history.messages[3].parts[0],ChatPart::Text(text) if text=="assistant text"));
        history.begin_reasoning_turn();history.reasoning_event("next",false);history.reasoning_event("next final",true);
        assert_eq!(thinking(&history),vec!["authoritative summary","next final"]);
    }

    #[test]
    fn reasoning_identity_survives_partial_removal_and_pruning_and_reset() {
        let mut history=NativeChatHistory::default();
        history.notice("old".into());history.replace_partial("partial before thinking");
        history.reasoning_event("first",false);history.notice("tail".into());
        history.remove_partial();history.reasoning_event(" first final",true);
        assert_eq!(thinking(&history),vec![" first final"]);
        for _ in 0..MAX_MESSAGES-2 {history.notice("padding".into());}
        history.reasoning_event("retained",true);assert_eq!(thinking(&history),vec!["retained"]);
        for _ in 0..MAX_MESSAGES+1 {history.notice("prune".into());}
        history.reasoning_event(&"界".repeat(MAX_CHARS*2),true);
        assert_eq!(thinking(&history).len(),1);assert_eq!(thinking(&history)[0].chars().count(),MAX_CHARS);
        assert!(history.messages.len()<=MAX_MESSAGES);assert!(history.messages.iter().map(message_bytes).sum::<usize>()<=MAX_BYTES);
        history.reset(vec![],vec![]);history.reasoning_event("after reset",false);
        assert_eq!(thinking(&history),vec!["after reset"]);
    }

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
        assert!(
            history
                .messages
                .iter()
                .all(|message| message.role == ChatRole::System)
        );
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
            ChatPart::ToolState {
                name,
                id,
                status,
                args,
                output,
                ..
            } => name.len() + id.len() + status.len() + args.len() + output.len(),
            ChatPart::File { filename, mime } => filename.len() + mime.len(),
            ChatPart::Subtask {
                agent, description, ..
            } => agent.len() + description.len(),
            ChatPart::Retry { message, .. } => message.len(),
            ChatPart::Step { kind, detail } => kind.len() + detail.len(),
            ChatPart::Compaction { reason } => reason.len(),
            ChatPart::Patch { path, diff } => path.len() + diff.len(),
            ChatPart::ToolCall { name, id } => name.len() + id.len(),
            ChatPart::ToolResult { name, output, .. } => name.len() + output.len(),
        })
        .sum::<usize>()
        + message.agent.as_ref().map_or(0, String::len)
        + message.model.as_ref().map_or(0, String::len)
}

impl NativeChatHistory {
    pub fn reasoning_event(&mut self, text: &str, is_final: bool) {
        if is_final {
            self.reasoning = bounded(text);
        } else {
            let remaining = MAX_CHARS.saturating_sub(self.reasoning.chars().count());
            self.reasoning.extend(text.chars().take(remaining));
        }
        if self.reasoning.is_empty() && self.reasoning_partial.is_none() {return;}
        if let Some((line, message)) = self.reasoning_partial {
            if line < self.lines.len() && message < self.messages.len() {
                self.messages[message].parts = vec![ChatPart::Thinking(self.reasoning.clone())];
                self.prune_messages();
                self.prune_lines();
                return;
            }
        }
        self.append("reasoning".into(),ChatMessage{role:ChatRole::Assistant,parts:vec![ChatPart::Thinking(self.reasoning.clone())],agent:None,model:None,assistant_facts:None});
        self.reasoning_partial=Some((self.lines.len()-1,self.messages.len()-1));
    }

    pub fn begin_reasoning_turn(&mut self) {
        self.reasoning.clear();
        self.reasoning_partial=None;
    }

    pub fn reset(&mut self, lines: Vec<String>, messages: Vec<ChatMessage>) {
        self.lines = lines;
        self.messages.clear();
        self.partial = None;
        self.begin_reasoning_turn();
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
        if let Some(facts) = message.assistant_facts.as_mut() {
            facts.stop_reason = facts.stop_reason.as_deref().map(bounded);
            facts.error_message = facts.error_message.as_deref().map(bounded);
        }
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
                ChatPart::ToolState {
                    name,
                    id,
                    status,
                    args,
                    output,
                    ..
                } => {
                    *name = bounded(name);
                    *id = bounded(id);
                    *status = bounded(status);
                    *args = bounded(args);
                    *output = bounded(output);
                }
                ChatPart::File { filename, mime } => {
                    *filename = bounded(filename);
                    *mime = bounded(mime);
                }
                ChatPart::Subtask {
                    agent, description, ..
                } => {
                    *agent = bounded(agent);
                    *description = bounded(description);
                }
                ChatPart::Retry { message, .. } => *message = bounded(message),
                ChatPart::Step { kind, detail } => {
                    *kind = bounded(kind);
                    *detail = bounded(detail);
                }
                ChatPart::Compaction { reason } => *reason = bounded(reason),
                ChatPart::Patch { path, diff } => {
                    *path = bounded(path);
                    *diff = bounded(diff);
                }
            }
        }
        self.messages.push(message);
        self.prune_messages();
    }

    fn prune_messages(&mut self) {
        let mut bytes: usize = self.messages.iter().map(message_bytes).sum();
        while bytes > MAX_BYTES || self.messages.len() > MAX_MESSAGES {
            bytes = bytes.saturating_sub(message_bytes(&self.messages.remove(0)));
            self.partial = self
                .partial
                .and_then(|(line, index)| index.checked_sub(1).map(|index| (line, index)));
            shift_reasoning_identity(&mut self.reasoning_partial,0,true);
        }
    }

    fn prune_lines(&mut self) {
        let mut bytes: usize = self.lines.iter().map(String::len).sum();
        while bytes > MAX_BYTES || self.lines.len() > MAX_MESSAGES {
            bytes = bytes.saturating_sub(self.lines.remove(0).len());
            self.partial = self
                .partial
                .and_then(|(index, message)| index.checked_sub(1).map(|index| (index, message)));
            shift_reasoning_identity(&mut self.reasoning_partial,0,false);
        }
    }

    pub fn remove_partial(&mut self) {
        if let Some((line, message)) = self.partial.take() {
            if line < self.lines.len() {
                self.lines.remove(line);
                shift_reasoning_identity(&mut self.reasoning_partial,line,false);
            }
            if message < self.messages.len() {
                self.messages.remove(message);
                shift_reasoning_identity(&mut self.reasoning_partial,message,true);
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

fn shift_reasoning_identity(identity:&mut Option<(usize,usize)>,removed:usize,message_axis:bool) {
    *identity=identity.and_then(|(line,message)| {
        let index=if message_axis {message}else{line};
        if index==removed {None}
        else if index>removed {Some(if message_axis{(line,message-1)}else{(line-1,message)})}
        else{Some((line,message))}
    });
}
