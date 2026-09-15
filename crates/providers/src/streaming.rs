//! Streaming session module.
//!
//! Buffers per-session streamed events from LLM providers so a session can
//! emit chunks incrementally and later drain the full event history.

use std::collections::HashMap;

/// A single event in a streaming session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// A chunk of streamed output text.
    Data(String),
    /// The stream completed successfully.
    Done,
    /// The stream failed with an error message.
    Error(String),
}

/// Tracks buffered streaming events per session id.
#[derive(Debug, Default)]
pub struct StreamingHandler {
    /// Buffered events keyed by session id.
    sessions: HashMap<String, Vec<StreamEvent>>,
    /// Session ids that have been started but not yet ended.
    pending_tokens: Vec<String>,
}

impl StreamingHandler {
    /// Creates an empty streaming handler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a new streaming session, creating an empty event buffer.
    pub fn start_session(&mut self, id: &str) {
        let id = id.to_string();
        if !self.sessions.contains_key(&id) {
            self.sessions.insert(id.clone(), Vec::new());
            self.pending_tokens.push(id);
        }
    }

    /// Appends an event to the session's buffer.
    ///
    /// Sessions not started are created on demand so `stream` still returns
    /// the emitted events.
    pub fn emit(&mut self, id: &str, event: StreamEvent) {
        let id = id.to_string();
        if !self.sessions.contains_key(&id) {
            self.sessions.insert(id.clone(), Vec::new());
            self.pending_tokens.push(id.clone());
        }
        self.sessions
            .get_mut(&id)
            .expect("session inserted above")
            .push(event);
    }

    /// Ends a session: appends [`StreamEvent::Done`] if not already completed
    /// and removes the session from pending tracking.
    pub fn end_session(&mut self, id: &str) {
        if let Some(events) = self.sessions.get_mut(id) {
            match events.last() {
                Some(StreamEvent::Done) | Some(StreamEvent::Error(_)) => {}
                _ => events.push(StreamEvent::Done),
            }
        }
        self.pending_tokens.retain(|token| token != id);
    }

    /// Returns a copy of all buffered events for a session.
    ///
    /// The buffer is retained so repeated calls return the full history.
    pub fn stream(&self, id: &str) -> Vec<StreamEvent> {
        self.sessions.get(id).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_stream() {
        let mut handler = StreamingHandler::new();
        handler.start_session("session-1");
        assert_eq!(handler.stream("session-1"), Vec::new());
        assert_eq!(handler.pending_tokens, vec!["session-1".to_string()]);
    }

    #[test]
    fn emit_data() {
        let mut handler = StreamingHandler::new();
        handler.start_session("session-2");
        handler.emit("session-2", StreamEvent::Data("hello ".to_string()));
        handler.emit("session-2", StreamEvent::Data("world".to_string()));

        let events = handler.stream("session-2");
        assert_eq!(
            events,
            vec![
                StreamEvent::Data("hello ".to_string()),
                StreamEvent::Data("world".to_string()),
            ]
        );
    }

    #[test]
    fn end_completes() {
        let mut handler = StreamingHandler::new();
        handler.start_session("session-3");
        handler.emit("session-3", StreamEvent::Data("chunk".to_string()));
        handler.end_session("session-3");

        let events = handler.stream("session-3");
        assert_eq!(
            events,
            vec![StreamEvent::Data("chunk".to_string()), StreamEvent::Done,]
        );
        assert!(handler.pending_tokens.is_empty());
    }

    #[test]
    fn multiple_sessions() {
        let mut handler = StreamingHandler::new();
        handler.start_session("session-a");
        handler.start_session("session-b");
        handler.emit("session-a", StreamEvent::Data("a1".to_string()));
        handler.emit("session-b", StreamEvent::Data("b1".to_string()));

        assert_eq!(
            handler.stream("session-a"),
            vec![StreamEvent::Data("a1".to_string())]
        );
        assert_eq!(
            handler.stream("session-b"),
            vec![StreamEvent::Data("b1".to_string())]
        );
    }

    #[test]
    fn stream_empty_after_end() {
        let mut handler = StreamingHandler::new();
        handler.start_session("session-c");
        handler.end_session("session-c");

        // Empty session ends with a single Done event.
        let events = handler.stream("session-c");
        assert_eq!(events, vec![StreamEvent::Done]);
        assert!(handler.pending_tokens.is_empty());

        // Never-started sessions stream empty and do not become pending.
        handler.emit("ghost", StreamEvent::Error("boom".to_string()));
        handler.end_session("ghost");
        assert_eq!(
            handler.stream("ghost"),
            vec![StreamEvent::Error("boom".to_string())]
        );
        assert!(handler.pending_tokens.is_empty());
    }
}
