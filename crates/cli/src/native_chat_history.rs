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
    /// Monotonic fallback identity for locally-created rows. Backend ids are
    /// authoritative when present; this prevents keyed reconciliation from
    /// silently falling back to a moving vector index for notices/streams.
    next_local_id: u64,
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

    #[test]
    fn retained_backend_id_survives_into_session_rows() {
        use crate::native_message_rows::{SessionEntry, SessionRow, Verbosity};
        let mut history = NativeChatHistory::default();
        let mut message = ChatMessage::text(ChatRole::User, "hi");
        message.message_id = Some("msg_42".to_owned());
        history.append("user".into(), message);
        let rows = history.rows(Verbosity::Medium);
        match &rows[0] {
            SessionRow::Entry(SessionEntry::Message { message_id }) => {
                assert_eq!(message_id, "msg_42", "backend id reaches the row model");
            }
            other => panic!("expected a message row: {other:?}"),
        }
    }

    #[test]
    fn retained_messages_project_into_the_shared_grouped_rows() {
        use crate::native_message_rows::{GroupKind, SessionRow, Verbosity};
        let mut history = NativeChatHistory::default();
        history.append("user".into(), ChatMessage::text(ChatRole::User, "hi"));
        history.append(
            "thinking".into(),
            ChatMessage {
                role: ChatRole::Assistant,
                parts: vec![
                    ChatPart::Thinking("first".into()),
                    ChatPart::Thinking("second".into()),
                ],
                agent: None,
                model: None,
                assistant_facts: None,
                message_id: None,
                files: Vec::new(),
                skills: Vec::new(),
            },
        );
        history.notice("note".into());
        let rows = history.rows(Verbosity::Medium);
        assert_eq!(rows.len(), 3);
        assert!(matches!(
            &rows[0],
            SessionRow::Entry(crate::native_message_rows::SessionEntry::Message { .. })
        ));
        match &rows[1] {
            SessionRow::Group(group) => {
                assert_eq!(group.kind, GroupKind::Reasoning);
                assert_eq!(group.size, 2, "consecutive thoughts group together");
                assert!(group.completed, "the following notice closes the group");
            }
            other => panic!("expected a reasoning group: {other:?}"),
        }
        assert!(matches!(
            &rows[2],
            SessionRow::Entry(crate::native_message_rows::SessionEntry::Message { .. })
        ));
    }

    #[test]
    fn par11_stream_replacement_keeps_the_same_logical_row_id() {
        use crate::native_message_rows::{row_key, Verbosity};
        let mut history = NativeChatHistory::default();
        history.replace_partial("first");
        let before = history.rows(Verbosity::Medium);
        let key_before = row_key(&before[0]).expect("stream row key");
        history.replace_partial("final content");
        let after = history.rows(Verbosity::Medium);
        let key_after = row_key(&after[0]).expect("updated row key");
        assert_eq!(key_before, key_after, "streaming->complete is an in-place update");
        assert_eq!(after.len(), 1, "row remains renderable");
    }

    #[test]
    fn par10_complete_then_final_content_does_not_duplicate_the_stream_row() {
        let mut history = NativeChatHistory::default();
        history.replace_partial("partial");
        history.remove_partial();
        history.append("assistant complete".into(), ChatMessage::text(ChatRole::Assistant, "complete"));
        let count = history.messages.iter().filter(|message| message.role == ChatRole::Assistant).count();
        assert_eq!(count, 1);
        history.replace_partial("final");
        assert_eq!(history.messages.iter().filter(|message| message.role == ChatRole::Assistant).count(), 2);
        assert!(history.messages.iter().any(|message| message.parts.iter().any(|part| matches!(part, ChatPart::Text(text) if text == "final"))));
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
            ChatPart::File { filename, mime, uri, data } => {
                filename.len() + mime.len() + uri.as_deref().map_or(0, str::len) + data.as_deref().map_or(0, str::len)
            }
            ChatPart::Subtask {
                agent, description, ..
            } => agent.len() + description.len(),
            ChatPart::Retry { message, .. } => message.len(),
            ChatPart::Step { kind, detail } => kind.len() + detail.len(),
            ChatPart::Compaction { reason, status, .. } => reason.len() + status.len(),
            ChatPart::Patch { path, diff, diagnostics } => path.len() + diff.len() + diagnostics.iter().map(|d| d.message.len()).sum::<usize>(),
            ChatPart::PatchToolRows { rows } => rows.iter().map(|row| row.label.len() + row.path.len() + row.deleted_summary.as_deref().map_or(0, str::len)).sum::<usize>(),
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
        self.append("reasoning".into(),ChatMessage{role:ChatRole::Assistant,parts:vec![ChatPart::Thinking(self.reasoning.clone())],agent:None,model:None,assistant_facts:None,message_id:None,files:Vec::new(),skills:Vec::new()});
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
        self.next_local_id = 0;
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
        if message.message_id.is_none() {
            message.message_id = Some(format!("local:{}", self.next_local_id));
            self.next_local_id = self.next_local_id.saturating_add(1);
        }
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
                ChatPart::File { filename, mime, uri, data } => {
                    *filename = bounded(filename);
                    *mime = bounded(mime);
                    *uri = uri.as_deref().map(bounded);
                    *data = data.as_deref().map(bounded);
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
                ChatPart::Compaction { reason, .. } => *reason = bounded(reason),
                ChatPart::Patch { path, diff, diagnostics } => {
                    *path = bounded(path);
                    *diff = bounded(diff);
                    for diagnostic in diagnostics.iter_mut() {
                        diagnostic.message = bounded(&diagnostic.message);
                    }
                }
                ChatPart::PatchToolRows { rows } => {
                    for row in rows {
                        row.label = bounded(&row.label);
                        row.path = bounded(&row.path);
                        row.deleted_summary = row.deleted_summary.as_deref().map(bounded);
                    }
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
        // A streaming update is an in-place update from the transcript's
        // point of view. Preserve the old logical key across replacement so a
        // keyed remount cannot reset expansion or scroll anchors.
        let message_id = self
            .partial
            .and_then(|(_, index)| self.messages.get(index))
            .and_then(|message| message.message_id.clone());
        self.remove_partial();
        let mut message = ChatMessage::text(ChatRole::Assistant, bounded(text));
        message.message_id = message_id;
        self.append(
            format!("assistant (streaming): {}", bounded(text)),
            message,
        );
        if !self.lines.is_empty() && !self.messages.is_empty() {
            self.partial = Some((self.lines.len() - 1, self.messages.len() - 1));
        }
    }

    /// Grouped transcript rows for the retained messages, using the shared
    /// row model (`native_message_rows`) so live history, replayed history and
    /// the daemon projection group identically.
    ///
    /// Identities use backend ids where available and monotonic local ids for
    /// locally-created rows. They therefore remain stable across in-place
    /// stream updates and keyed session remounts.
    #[must_use]
    pub fn rows(&self, verbosity: crate::native_message_rows::Verbosity) -> Vec<crate::native_message_rows::SessionRow> {
        let entries: Vec<(String, ChatMessage)> = self
            .messages
            .iter()
            .map(|message| {
                let id = message
                    .message_id
                    .clone()
                    .unwrap_or_else(|| "local:missing".to_owned());
                (id, message.clone())
            })
            .collect();
        crate::native_message_rows::rows_from_chat_entries(&entries, verbosity)
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
