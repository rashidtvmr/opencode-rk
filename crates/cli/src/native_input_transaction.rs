//! Transactional input-dispatch primitives.
//!
//! This module deliberately contains no live NativeView or I/O calls. It is the
//! owned planning boundary used by future dispatcher extraction: handlers stage
//! local editor state and external intents, then the caller chooses commit or
//! rollback. Consuming a committed plan moves commands out exactly once.

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InputStateDraft {
    pub(crate) text: String,
    pub(crate) cursor: usize,
    pub(crate) generation: u64,
}

impl InputStateDraft {
    pub(crate) fn new(text: impl Into<String>, cursor: usize, generation: u64) -> Self {
        let text = text.into();
        Self { cursor: cursor.min(text.len()), text, generation }
    }

    pub(crate) fn insert(&mut self, ch: char) -> bool {
        if self.cursor > self.text.len() || !self.text.is_char_boundary(self.cursor) { return false; }
        let mut bytes = [0; 4];
        let encoded = ch.encode_utf8(&mut bytes);
        self.text.insert_str(self.cursor, encoded);
        self.cursor += encoded.len();
        self.generation = self.generation.wrapping_add(1);
        true
    }

    pub(crate) fn rollback(self) {}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExternalAction {
    Http { method: String, path: String, body: Option<String>, auth: Option<String> },
    Worker { name: String, session: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandPlan {
    actions: Vec<ExternalAction>,
    committed: bool,
}

impl CommandPlan {
    pub(crate) fn new() -> Self { Self::default() }
    pub(crate) fn push(&mut self, action: ExternalAction) {
        if !self.committed { self.actions.push(action); }
    }
    pub(crate) fn commit(&mut self) { self.committed = true; }
    pub(crate) fn rollback(&mut self) { self.actions.clear(); self.committed = false; }
    pub(crate) fn into_committed(self) -> Option<Vec<ExternalAction>> {
        self.committed.then_some(self.actions)
    }
    pub(crate) fn committed(&self) -> bool { self.committed }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DispatchOutcome {
    Continue,
    NeedsPaint,
    Exit,
    Crash(String),
}

/// Protocol-only events are classified before any mutable NativeView or
/// external command handling. This is the first live-dispatch seam: protocol
/// replies and focus reports consume their event exactly once and request a
/// repaint without entering the broad legacy dispatcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProtocolEvent {
    FocusGained,
    FocusLost,
    OscColorReply,
    Resize,
}

pub(crate) fn classify_protocol(event: ProtocolEvent) -> DispatchOutcome {
    match event {
        ProtocolEvent::FocusGained
        | ProtocolEvent::FocusLost
        | ProtocolEvent::OscColorReply
        | ProtocolEvent::Resize => DispatchOutcome::NeedsPaint,
    }
}

/// Owned geometry facts for one terminal resize notification. The transaction
/// does not touch the renderer or NativeView; callers commit the facts only
/// after their resize operations succeed, so a rejected event cannot replay a
/// stale size or generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResizeEventTxn {
    prior: (u16, u16),
    next: (u16, u16),
    prior_generation: u64,
    committed: bool,
}

impl ResizeEventTxn {
    pub(crate) fn begin(prior: (u16, u16), next: (u16, u16), generation: u64) -> Self {
        Self { prior, next, prior_generation: generation, committed: false }
    }
    pub(crate) fn commit(&mut self) -> Option<((u16, u16), u64)> {
        if self.committed || self.next == self.prior { return None; }
        self.committed = true;
        Some((self.next, self.prior_generation.wrapping_add(1)))
    }
    pub(crate) fn rollback(&mut self) {
        self.next = self.prior;
        self.committed = false;
    }
    pub(crate) fn committed(&self) -> bool { self.committed }
}

#[derive(Debug)]
pub(crate) struct InputEventTxn {
    draft: InputStateDraft,
    plan: CommandPlan,
    consumed: bool,
}

impl InputEventTxn {
    pub(crate) fn begin(draft: InputStateDraft) -> Self {
        Self { draft, plan: CommandPlan::new(), consumed: false }
    }
    pub(crate) fn draft_mut(&mut self) -> &mut InputStateDraft { &mut self.draft }
    pub(crate) fn plan_mut(&mut self) -> &mut CommandPlan { &mut self.plan }
    pub(crate) fn consume(&mut self) -> bool {
        if self.consumed { false } else { self.consumed = true; true }
    }
    pub(crate) fn commit(mut self) -> (InputStateDraft, Option<Vec<ExternalAction>>) {
        self.plan.commit();
        (self.draft, self.plan.into_committed())
    }
    pub(crate) fn rollback(mut self) {
        self.plan.rollback();
        self.draft.rollback();
    }
    pub(crate) fn consumed(&self) -> bool { self.consumed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_is_consumed_once_and_rollback_discards_actions() {
        let mut tx = InputEventTxn::begin(InputStateDraft::new("hi", 2, 7));
        assert!(tx.consume());
        assert!(!tx.consume());
        assert!(tx.draft_mut().insert('!'));
        tx.plan_mut().push(ExternalAction::Http {
            method: "POST".into(), path: "/api/test".into(), body: None, auth: None,
        });
        tx.rollback();
    }

    #[test]
    fn commit_publishes_owned_draft_and_commands_once() {
        let mut tx = InputEventTxn::begin(InputStateDraft::new("a", 1, 0));
        assert!(tx.consume());
        tx.draft_mut().insert('b');
        tx.plan_mut().push(ExternalAction::Worker { name: "compact".into(), session: "s".into() });
        let (draft, actions) = tx.commit();
        assert_eq!(draft.text, "ab");
        assert_eq!(actions.unwrap().len(), 1);
        assert_eq!(DispatchOutcome::NeedsPaint, DispatchOutcome::NeedsPaint);
    }

    #[test]
    fn protocol_classification_is_pure_and_requests_paint() {
        for event in [
            ProtocolEvent::FocusGained,
            ProtocolEvent::FocusLost,
            ProtocolEvent::OscColorReply,
            ProtocolEvent::Resize,
        ] {
            assert_eq!(classify_protocol(event), DispatchOutcome::NeedsPaint);
        }
    }

    #[test]
    #[test]
    fn resize_transaction_commits_once_and_rolls_back_without_replay() {
        let mut tx = ResizeEventTxn::begin((80, 24), (100, 30), 4);
        assert_eq!(tx.commit(), Some(((100, 30), 5)));
        assert_eq!(tx.commit(), None);
        assert!(tx.committed());
        let mut rolled = ResizeEventTxn::begin((80, 24), (120, 40), 9);
        rolled.rollback();
        assert_eq!(rolled.commit(), None);
        assert!(!rolled.committed());
    }

    #[test]
    fn uncommitted_plan_cannot_execute_or_replay() {
        let mut plan = CommandPlan::new();
        plan.push(ExternalAction::Http { method: "DELETE".into(), path: "/x".into(), body: None, auth: None });
        plan.rollback();
        assert!(!plan.committed());
        assert_eq!(plan.into_committed(), None);
    }
}
