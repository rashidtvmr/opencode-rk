#![forbid(unsafe_code)]
//! Native session-tab navigation state (TUI-007, slice: app types).
//!
//! Pure state only: independent per-tab drafts/scroll/model, daemon event
//! convergence (rename/archive/fork), bounded tab lists. Closing a tab never
//! terminates daemon-owned work: [`NavigationState::close`] returns a
//! [`CloseReceipt`] echoing the retained daemon work entry, which stays
//! queryable via [`NavigationState::work_state`] after the tab is gone.
//! No rendering, no IO, no clock, no threads.
//!
//! Convergence: rename/archive events carry a daemon-issued monotonic `rev`.
//! Out-of-order renames converge on the highest `rev` (last-writer-wins per
//! revision, so concurrent renames settle deterministically); an archive
//! tombstone discards renames at or below its `rev`. Fork/resume never touch
//! the service from here: [`NavigationState::fork_request`] and
//! [`NavigationState::resume_request`] build intent values the caller must
//! pass to the real session service, so routing is explicit and testable.
//!
//! Commit: base 5af7884. Evidence: card TUI-007 (T01 independent tab state,
//! T02 child provenance, T03 fork/resume invokes the real service, T04
//! concurrent rename/archive converge, T05 bounded lists and closing a tab
//! does not kill daemon-owned work); prior art
//! `crates/cli/src/native_app.rs` (focus/view enums, bounded history).

/// Upper bound on open tabs, retained titles, work entries and tombstones.
pub const MAX_TABS: usize = 32;

/// Opaque caller-supplied session identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SessionRef(pub String);

/// A daemon-owned session is always considered live work regardless of any
/// tab being closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkState {
    /// The session has work the daemon is executing.
    Running,
    /// The session is settled.
    Idle,
}

/// One open tab with client-local state (draft, scroll and model are per-tab).
#[derive(Clone, Debug)]
pub struct Tab {
    pub session: SessionRef,
    /// Parent session when this tab was opened by fork/branch navigation.
    pub parent: Option<SessionRef>,
    pub draft: String,
    pub scroll: usize,
    pub selected_model: Option<String>,
}

impl Tab {
    fn new(session: SessionRef) -> Self {
        Self {
            session,
            parent: None,
            draft: String::new(),
            scroll: 0,
            selected_model: None,
        }
    }
}

/// Why a tab action did not happen. [`NavigationState::open`] never refuses:
/// a full tab list evicts the oldest tab instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavRefusal {
    NoTabOpen,
    NoSuchTab,
}

/// Proof that closing a tab left daemon-owned work untouched. `daemon_work`
/// equals [`NavigationState::work_state`] for the session after the close.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseReceipt {
    pub session: SessionRef,
    pub daemon_work: Option<WorkState>,
}

impl CloseReceipt {
    /// Retained daemon work entry; `Some(Running)` means the daemon is still
    /// executing the closed tab's session.
    #[must_use]
    pub fn retained_work(&self) -> Option<WorkState> {
        self.daemon_work
    }
}

/// Intent to fork an open session. The caller must invoke the real session
/// service with [`ForkRequest::parent`]; applying the resulting child happens
/// via [`DaemonEvent::Forked`] or [`NavigationState::open_forked`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkRequest {
    pub parent: SessionRef,
}

/// Intent to resume an open session via the real session service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumeRequest {
    pub session: SessionRef,
}

/// Navigation events sourced from the daemon (versioned state, never local
/// guesses). `rev` is a daemon-issued monotonic revision per session.
#[derive(Clone, Debug)]
pub enum DaemonEvent {
    Renamed {
        session: SessionRef,
        title: String,
        rev: u64,
    },
    Archived {
        session: SessionRef,
        rev: u64,
    },
    Forked {
        child: SessionRef,
        parent: SessionRef,
    },
    WorkState {
        session: SessionRef,
        state: WorkState,
    },
}

/// The full navigation state: open tabs, active tab, daemon work states.
#[derive(Debug, Default)]
pub struct NavigationState {
    tabs: Vec<Tab>,
    active: usize,
    /// Daemon-reported work state per session, retained even with no tab.
    work: Vec<(SessionRef, WorkState)>,
    /// Display titles with the revision that set them.
    titles: Vec<(SessionRef, String, u64)>,
    /// Archive tombstones so stale renames cannot resurrect dead sessions.
    archived: Vec<(SessionRef, u64)>,
}

impl NavigationState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a tab for a session (or focus the existing one). When the list is
    /// full the oldest tab is evicted; focus stays on the previously active
    /// session unless it was the evicted one.
    pub fn open(&mut self, session: SessionRef) -> usize {
        if let Some(idx) = self.tabs.iter().position(|t| t.session == session) {
            self.active = idx;
            return idx;
        }
        if self.tabs.len() >= MAX_TABS {
            self.tabs.remove(0);
            self.active = self.active.saturating_sub(1);
        }
        self.tabs.push(Tab::new(session));
        self.active = self.tabs.len() - 1;
        self.active
    }

    /// Open a tab for a forked child, preserving provenance (T02).
    pub fn open_forked(&mut self, child: SessionRef, parent: SessionRef) -> usize {
        let idx = self.open(child);
        self.tabs[idx].parent = Some(parent);
        idx
    }

    /// Close a tab by session. Daemon-owned work keeps running (T05): the
    /// work entry is retained and echoed in the returned receipt.
    pub fn close(&mut self, session: &SessionRef) -> Result<CloseReceipt, NavRefusal> {
        if !self.remove_tab(session) {
            return Err(NavRefusal::NoSuchTab);
        }
        Ok(CloseReceipt {
            session: session.clone(),
            daemon_work: self.work_state(session),
        })
    }

    /// Switch focus to a tab by session.
    pub fn switch(&mut self, session: &SessionRef) -> Result<usize, NavRefusal> {
        if self.tabs.is_empty() {
            return Err(NavRefusal::NoTabOpen);
        }
        let idx = self
            .tabs
            .iter()
            .position(|t| t.session == *session)
            .ok_or(NavRefusal::NoSuchTab)?;
        self.active = idx;
        Ok(idx)
    }

    /// The focused tab.
    #[must_use]
    pub fn active(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    /// Mutable focused tab (draft/scroll/model edits are client-local).
    pub fn active_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    /// Open tab count.
    #[must_use]
    pub fn tab_len(&self) -> usize {
        self.tabs.len()
    }

    /// All tabs, oldest first, for the tab bar.
    #[must_use]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// One page of the bounded tab list, oldest first. Empty when `page` is
    /// out of range or `per_page` is zero (T05 pagination).
    #[must_use]
    pub fn tab_page(&self, page: usize, per_page: usize) -> &[Tab] {
        if per_page == 0 {
            return &[];
        }
        let start = page.saturating_mul(per_page);
        if start >= self.tabs.len() {
            return &[];
        }
        let end = (start + per_page).min(self.tabs.len());
        &self.tabs[start..end]
    }

    /// Build the intent the caller must hand to the real session service to
    /// fork an open tab. Refuses unknown sessions so forks route correctly.
    pub fn fork_request(&self, parent: &SessionRef) -> Result<ForkRequest, NavRefusal> {
        if self.tabs.is_empty() {
            return Err(NavRefusal::NoTabOpen);
        }
        if !self.tabs.iter().any(|t| &t.session == parent) {
            return Err(NavRefusal::NoSuchTab);
        }
        Ok(ForkRequest {
            parent: parent.clone(),
        })
    }

    /// Build the intent the caller must hand to the real session service to
    /// resume an open tab.
    pub fn resume_request(&self, session: &SessionRef) -> Result<ResumeRequest, NavRefusal> {
        if self.tabs.is_empty() {
            return Err(NavRefusal::NoTabOpen);
        }
        if !self.tabs.iter().any(|t| &t.session == session) {
            return Err(NavRefusal::NoSuchTab);
        }
        Ok(ResumeRequest {
            session: session.clone(),
        })
    }

    /// Apply a daemon event. Rename/archive/fork converge all views; the
    /// focus moves off an archived session without switching anywhere else
    /// (T04). Stale renames (older `rev`, or at/below an archive tombstone)
    /// are dropped.
    pub fn apply(&mut self, event: DaemonEvent) {
        match event {
            DaemonEvent::Renamed { session, title, rev } => {
                if self.tombstone_at_or_above(&session, rev) {
                    return;
                }
                match self.titles.iter_mut().find(|(s, _, _)| *s == session) {
                    Some(entry) => {
                        if entry.2 <= rev {
                            entry.1 = title;
                            entry.2 = rev;
                        }
                    }
                    None => {
                        if self.titles.len() < MAX_TABS {
                            self.titles.push((session, title, rev));
                        }
                    }
                }
            }
            DaemonEvent::Archived { session, rev } => {
                if self.tombstone_at_or_above(&session, rev) {
                    return;
                }
                self.remove_tab(&session);
                self.titles.retain(|(s, _, _)| *s != session);
                self.work.retain(|(s, _)| *s != session);
                if self.archived.len() >= MAX_TABS {
                    self.archived.remove(0);
                }
                self.archived.push((session, rev));
            }
            DaemonEvent::Forked { child, parent } => {
                self.open_forked(child, parent);
            }
            DaemonEvent::WorkState { session, state } => {
                match self.work.iter_mut().find(|(s, _)| *s == session) {
                    Some(entry) => entry.1 = state,
                    None => {
                        if self.work.len() < MAX_TABS {
                            self.work.push((session, state));
                        }
                    }
                }
            }
        }
    }

    /// Daemon-reported work state for a session, independent of tab presence.
    /// `None` means the daemon reported nothing (or the session archived).
    #[must_use]
    pub fn work_state(&self, session: &SessionRef) -> Option<WorkState> {
        self.work
            .iter()
            .find(|(s, _)| s == session)
            .map(|(_, state)| *state)
    }

    /// Display title recorded from daemon rename events.
    #[must_use]
    pub fn title(&self, session: &SessionRef) -> Option<&str> {
        self.titles
            .iter()
            .find(|(s, _, _)| s == session)
            .map(|(_, t, _)| t.as_str())
    }

    fn tombstone_at_or_above(&self, session: &SessionRef, rev: u64) -> bool {
        match self.archived.iter().find(|(s, _)| s == session) {
            Some((_, tomb_rev)) => *tomb_rev >= rev,
            None => false,
        }
    }

    /// Remove a tab, keeping focus on the previously focused session.
    /// Returns false when no such tab exists.
    fn remove_tab(&mut self, session: &SessionRef) -> bool {
        let idx = match self.tabs.iter().position(|t| &t.session == session) {
            Some(idx) => idx,
            None => return false,
        };
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.active = 0;
        } else if idx < self.active {
            self.active -= 1;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: &str) -> SessionRef {
        SessionRef(id.to_string())
    }

    #[test]
    fn tabs_keep_independent_drafts_scroll_and_models() {
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.open(s("b"));
        {
            let tab = nav.active_mut().unwrap();
            tab.selected_model = Some("model-b".into());
            tab.scroll = 42;
        }
        nav.switch(&s("a")).unwrap();
        {
            let tab = nav.active_mut().unwrap();
            tab.draft = "draft a".into();
            tab.selected_model = Some("model-a".into());
            tab.scroll = 7;
        }
        assert_eq!(nav.active().unwrap().draft, "draft a");
        assert_eq!(nav.active().unwrap().scroll, 7);
        assert_eq!(
            nav.active().unwrap().selected_model.as_deref(),
            Some("model-a")
        );
        nav.switch(&s("b")).unwrap();
        assert_eq!(nav.active().unwrap().draft, "");
        assert_eq!(nav.active().unwrap().scroll, 42);
        assert_eq!(
            nav.active().unwrap().selected_model.as_deref(),
            Some("model-b")
        );
    }

    #[test]
    fn closing_a_tab_returns_receipt_and_keeps_daemon_owned_work() {
        let mut nav = NavigationState::new();
        nav.open(s("work"));
        nav.apply(DaemonEvent::WorkState {
            session: s("work"),
            state: WorkState::Running,
        });
        let receipt = nav.close(&s("work")).unwrap();
        assert_eq!(receipt.session, s("work"));
        assert_eq!(receipt.retained_work(), Some(WorkState::Running));
        assert_eq!(nav.tab_len(), 0);
        // The daemon-owned session is still running with no tab open, and the
        // receipt echoes exactly what the state retains.
        assert_eq!(nav.work_state(&s("work")), receipt.retained_work());
        // Closing an unknown tab refuses and leaves daemon state untouched.
        assert_eq!(nav.close(&s("ghost")), Err(NavRefusal::NoSuchTab));
        assert_eq!(nav.work_state(&s("work")), Some(WorkState::Running));
    }

    #[test]
    fn concurrent_renames_converge_on_highest_revision() {
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.apply(DaemonEvent::Renamed {
            session: s("a"),
            title: "rev five".into(),
            rev: 5,
        });
        // Stale concurrent rename arrives late: dropped.
        nav.apply(DaemonEvent::Renamed {
            session: s("a"),
            title: "rev three".into(),
            rev: 3,
        });
        assert_eq!(nav.title(&s("a")), Some("rev five"));
        nav.apply(DaemonEvent::Renamed {
            session: s("a"),
            title: "rev seven".into(),
            rev: 7,
        });
        assert_eq!(nav.title(&s("a")), Some("rev seven"));
        // Archive tombstone wins over older renames, even late arrivals.
        nav.apply(DaemonEvent::Archived {
            session: s("a"),
            rev: 9,
        });
        nav.apply(DaemonEvent::Renamed {
            session: s("a"),
            title: "zombie".into(),
            rev: 8,
        });
        assert_eq!(nav.title(&s("a")), None);
        assert_eq!(nav.tab_len(), 0);
    }

    #[test]
    fn fork_tab_preserves_parent_provenance_and_routes_requests() {
        let mut nav = NavigationState::new();
        nav.open(s("parent"));
        nav.apply(DaemonEvent::Forked {
            child: s("child"),
            parent: s("parent"),
        });
        nav.switch(&s("child")).unwrap();
        assert_eq!(nav.active().unwrap().parent.as_ref(), Some(&s("parent")));
        // Intents route to the real service with the exact session ids.
        assert_eq!(
            nav.fork_request(&s("parent")).unwrap(),
            ForkRequest { parent: s("parent") }
        );
        assert_eq!(
            nav.resume_request(&s("child")).unwrap(),
            ResumeRequest { session: s("child") }
        );
        assert_eq!(nav.fork_request(&s("ghost")), Err(NavRefusal::NoSuchTab));
        assert_eq!(
            nav.resume_request(&s("ghost")),
            Err(NavRefusal::NoSuchTab)
        );
    }

    #[test]
    fn archive_event_switches_away_and_drops_stale_state() {
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.open(s("b"));
        nav.apply(DaemonEvent::WorkState {
            session: s("b"),
            state: WorkState::Running,
        });
        nav.apply(DaemonEvent::Archived {
            session: s("b"),
            rev: 1,
        });
        assert_eq!(nav.tab_len(), 1);
        assert_eq!(nav.active().unwrap().session, s("a"));
        assert_eq!(nav.work_state(&s("b")), None);
    }

    #[test]
    fn tab_list_is_bounded_and_paginates() {
        let mut nav = NavigationState::new();
        for i in 0..(MAX_TABS + 8) {
            nav.open(s(&format!("s{i}")));
        }
        assert_eq!(MAX_TABS, 32);
        assert_eq!(nav.tab_len(), MAX_TABS);
        // The oldest tabs were evicted, the newest is focused.
        assert_eq!(
            nav.active().unwrap().session,
            s(&format!("s{}", MAX_TABS + 7))
        );
        assert_eq!(nav.tabs().first().unwrap().session, s("s8"));
        let page0 = nav.tab_page(0, 16);
        assert_eq!(page0.len(), 16);
        assert_eq!(page0[0].session, s("s8"));
        assert_eq!(nav.tab_page(1, 16).len(), 16);
        assert!(nav.tab_page(2, 16).is_empty());
        assert!(nav.tab_page(0, 0).is_empty());
    }

    #[test]
    fn eviction_keeps_focus_on_previously_active_session() {
        let mut nav = NavigationState::new();
        for i in 0..MAX_TABS {
            nav.open(s(&format!("s{i}")));
        }
        nav.switch(&s("s10")).unwrap();
        nav.open(s("new"));
        assert_eq!(nav.tab_len(), MAX_TABS);
        assert_eq!(nav.active().unwrap().session, s("new"));
        nav.switch(&s("s10")).unwrap();
        assert_eq!(nav.active().unwrap().session, s("s10"));
        // Closing a tab before the focused one keeps focus on the session.
        nav.close(&s("s5")).unwrap();
        assert_eq!(nav.active().unwrap().session, s("s10"));
    }

    #[test]
    fn switch_refuses_unknown_or_closed_tabs() {
        let mut nav = NavigationState::new();
        assert_eq!(nav.switch(&s("x")), Err(NavRefusal::NoTabOpen));
        nav.open(s("a"));
        assert_eq!(nav.switch(&s("x")), Err(NavRefusal::NoSuchTab));
        nav.close(&s("a")).unwrap();
        assert_eq!(nav.switch(&s("a")), Err(NavRefusal::NoTabOpen));
    }
}
