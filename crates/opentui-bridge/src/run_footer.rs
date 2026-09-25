//! Run footer queue (TS `run/footer.ts` read-only ref).
//!
//! Microtask-coalesced footer view: callers [`RunFooter::append`] stream
//! commits, [`RunFooter::flush`] drains the queue in one batch. Fail-closed:
//! appends after destroy are rejected, the queue is capped at [`FOOTER_CAP`]
//! (oldest evicted), and destroy is idempotent.

use crate::run_stream::StreamCommit;

/// Maximum queued commits; oldest is evicted past the cap.
pub const FOOTER_CAP: usize = 128;

/// Visible footer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FooterView {
    /// No run active.
    #[default]
    Idle,
    /// Run streaming output.
    Streaming,
    /// Run finished.
    Done,
}

/// Event kinds accepted by [`RunFooter::event`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterEventKind {
    /// Run started.
    Start,
    /// A commit landed.
    Commit,
    /// Run finished.
    Finish,
    /// Footer destroyed (destroy hook).
    Destroy,
}

/// Coalesced footer queue with a destroy latch.
#[derive(Debug, Default)]
pub struct RunFooter {
    view: FooterView,
    queue: Vec<StreamCommit>,
    destroyed: bool,
}

impl RunFooter {
    /// Empty idle footer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Current view.
    #[must_use]
    pub fn view(&self) -> FooterView {
        self.view
    }

    /// Queued commit count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// True when nothing is queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// True after [`RunFooter::destroy`] or a destroy event.
    #[must_use]
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    /// Queue a commit; false when destroyed. Evicts oldest past the cap.
    pub fn append(&mut self, commit: StreamCommit) -> bool {
        if self.destroyed {
            return false;
        }
        if self.queue.len() >= FOOTER_CAP {
            self.queue.remove(0);
        }
        self.queue.push(commit);
        true
    }

    /// Drain the queue (microtask coalesce point).
    pub fn flush(&mut self) -> Vec<StreamCommit> {
        std::mem::take(&mut self.queue)
    }

    /// Apply an event kind, returning the new view.
    /// `Destroy` latches destroyed, clears the queue, views `Done`.
    pub fn event(&mut self, kind: FooterEventKind) -> FooterView {
        match kind {
            FooterEventKind::Start | FooterEventKind::Commit => {
                if !self.destroyed {
                    self.view = FooterView::Streaming;
                }
            }
            FooterEventKind::Finish => {
                if !self.destroyed {
                    self.view = FooterView::Done;
                }
            }
            FooterEventKind::Destroy => {
                self.destroy();
            }
        }
        self.view
    }

    /// Latch destroyed and clear the queue; idempotent.
    pub fn destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;
        self.queue.clear();
        self.view = FooterView::Done;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str) -> StreamCommit {
        StreamCommit {
            id: id.into(),
            text: "t".into(),
        }
    }

    #[test]
    fn append_caps_at_128_evicting_oldest() {
        let mut f = RunFooter::new();
        for i in 0..FOOTER_CAP + 5 {
            assert!(f.append(commit(&format!("c-{i}"))));
        }
        assert_eq!(f.len(), FOOTER_CAP);
        let out = f.flush();
        assert_eq!(out.len(), FOOTER_CAP);
        assert_eq!(out[0].id, "c-5");
        assert_eq!(out[FOOTER_CAP - 1].id, format!("c-{}", FOOTER_CAP + 4));
    }

    #[test]
    fn flush_drains_leaving_empty() {
        let mut f = RunFooter::new();
        f.append(commit("c-1"));
        f.append(commit("c-2"));
        let out = f.flush();
        assert_eq!(out.len(), 2);
        assert!(f.is_empty());
        assert!(f.flush().is_empty());
    }

    #[test]
    fn destroy_blocks_appends_and_clears() {
        let mut f = RunFooter::new();
        f.append(commit("c-1"));
        f.destroy();
        assert!(f.is_destroyed());
        assert!(!f.append(commit("c-2")));
        assert!(f.is_empty());
        assert!(f.flush().is_empty());
    }

    #[test]
    fn event_switches_view() {
        let mut f = RunFooter::new();
        assert_eq!(f.view(), FooterView::Idle);
        assert_eq!(f.event(FooterEventKind::Start), FooterView::Streaming);
        assert_eq!(f.event(FooterEventKind::Commit), FooterView::Streaming);
        assert_eq!(f.event(FooterEventKind::Finish), FooterView::Done);
    }

    #[test]
    fn destroy_event_latches_and_freezes_view() {
        let mut f = RunFooter::new();
        f.append(commit("c-1"));
        assert_eq!(f.event(FooterEventKind::Destroy), FooterView::Done);
        assert!(f.is_destroyed());
        assert!(f.is_empty());
        assert_eq!(f.event(FooterEventKind::Start), FooterView::Done);
    }

    #[test]
    fn double_destroy_safe() {
        let mut f = RunFooter::new();
        f.destroy();
        f.destroy();
        assert!(f.is_destroyed());
        assert_eq!(f.view(), FooterView::Done);
    }
}

/// Second-generation footer view (TS `FooterView` in `run/types.ts`: prompt,
/// permission, question; plus status, subagent, menu surfaces).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FooterView2 {
    /// Idle status line.
    #[default]
    Status,
    /// Composer prompt.
    Prompt,
    /// Permission request.
    Permission,
    /// Question request.
    Question,
    /// Subagent panel.
    Subagent,
    /// Menu / picker panel.
    Menu,
}

/// Minimal footer view machine with a busy flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FooterMachine {
    view: FooterView2,
    busy: bool,
}

impl FooterMachine {
    /// Idle status machine, not busy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Current view.
    #[must_use]
    pub fn view(&self) -> FooterView2 {
        self.view
    }

    /// Busy flag.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    /// Switch the active view.
    pub fn show(&mut self, view: FooterView2) {
        self.view = view;
    }

    /// Set the busy flag.
    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    /// True when the prompt view is active.
    #[must_use]
    pub fn is_prompt(&self) -> bool {
        self.view == FooterView2::Prompt
    }

    /// Stable label for the active view.
    #[must_use]
    pub fn view_label(&self) -> &'static str {
        match self.view {
            FooterView2::Status => "status",
            FooterView2::Prompt => "prompt",
            FooterView2::Permission => "permission",
            FooterView2::Question => "question",
            FooterView2::Subagent => "subagent",
            FooterView2::Menu => "menu",
        }
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    fn default_is_status_idle() {
        let m = FooterMachine::new();
        assert_eq!(m.view(), FooterView2::Status);
        assert!(!m.is_busy());
        assert!(!m.is_prompt());
    }

    #[test]
    fn show_switches_view() {
        let mut m = FooterMachine::new();
        m.show(FooterView2::Permission);
        assert_eq!(m.view(), FooterView2::Permission);
        m.show(FooterView2::Question);
        assert_eq!(m.view(), FooterView2::Question);
        m.show(FooterView2::Subagent);
        assert_eq!(m.view(), FooterView2::Subagent);
        m.show(FooterView2::Menu);
        assert_eq!(m.view(), FooterView2::Menu);
    }

    #[test]
    fn busy_flag_toggles() {
        let mut m = FooterMachine::new();
        m.set_busy(true);
        assert!(m.is_busy());
        m.set_busy(false);
        assert!(!m.is_busy());
    }

    #[test]
    fn prompt_detect_only_on_prompt() {
        let mut m = FooterMachine::new();
        assert!(!m.is_prompt());
        m.show(FooterView2::Prompt);
        assert!(m.is_prompt());
        m.show(FooterView2::Status);
        assert!(!m.is_prompt());
    }

    #[test]
    fn labels_non_empty_and_stable() {
        let views = [
            FooterView2::Status,
            FooterView2::Prompt,
            FooterView2::Permission,
            FooterView2::Question,
            FooterView2::Subagent,
            FooterView2::Menu,
        ];
        for v in views {
            let mut m = FooterMachine::new();
            m.show(v);
            assert!(!m.view_label().is_empty(), "label empty for {v:?}");
        }
        let mut m = FooterMachine::new();
        m.show(FooterView2::Prompt);
        assert_eq!(m.view_label(), "prompt");
        m.show(FooterView2::Permission);
        assert_eq!(m.view_label(), "permission");
    }
}
