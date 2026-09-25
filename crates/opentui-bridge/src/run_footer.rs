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
