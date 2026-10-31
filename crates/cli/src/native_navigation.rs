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

use crate::native_app::Focus;

/// Upper bound on open tabs, retained titles, work entries and tombstones.
pub const MAX_TABS: usize = 32;

/// Upper bound on retained unversioned snapshot titles (bounded to open tabs).
pub const MAX_SNAPSHOT_TITLES: usize = MAX_TABS;

/// Upper bound on navigable sidebar entries (mirrors the tab bound).
pub const MAX_SIDEBAR: usize = MAX_TABS;

/// Presentation orientation for the same session-tab identity list.  The
/// orientation is deliberately not part of a tab identity: switching between
/// horizontal and vertical strips must not recreate tabs or lose local state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabOrientation {
    #[default]
    Horizontal,
    Vertical,
}

/// Advance keyboard focus, skipping a collapsed/hidden sidebar so focus
/// never strands on a zero-area region. Pure mirror of
/// `crate::native_app::NativeApp::cycle_focus`: the caller passes whether
/// the sidebar rect is empty.
#[must_use]
pub fn focus_next(current: Focus, sidebar_empty: bool) -> Focus {
    let mut next = current.next();
    if next == Focus::Sidebar && sidebar_empty {
        next = Focus::Status;
    }
    next
}

/// Reverse of [`focus_next`]; pure mirror of `NativeApp::cycle_focus_prev`.
#[must_use]
pub fn focus_prev(current: Focus, sidebar_empty: bool) -> Focus {
    let mut prev = current.prev();
    if prev == Focus::Sidebar && sidebar_empty {
        prev = Focus::Transcript;
    }
    prev
}

/// Remap a sidebar focus request onto the transcript when the sidebar rect
/// is empty. Pure mirror of `ShellLayout::resolve_focus`.
#[must_use]
pub fn resolve_sidebar_focus(requested: Focus, sidebar_empty: bool) -> Focus {
    match requested {
        Focus::Sidebar if sidebar_empty => Focus::Transcript,
        other => other,
    }
}

/// A navigable sidebar roster: ordered session entries with a bounded
/// selection cursor. Pure state, no rendering; the caller sizes the visible
/// window from the sidebar rect.
#[derive(Clone, Debug, Default)]
pub struct SidebarNav {
    entries: Vec<SessionRef>,
    selected: usize,
}

impl SidebarNav {
    /// Empty roster.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Roster length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the roster holds no entry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Selected entry, `None` when empty.
    #[must_use]
    pub fn selected(&self) -> Option<&SessionRef> {
        self.entries.get(self.selected)
    }

    /// Selection index (0 while empty).
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Entries oldest first.
    #[must_use]
    pub fn entries(&self) -> &[SessionRef] {
        &self.entries
    }

    /// Replace the roster, keeping the selection on the same session when
    /// it survives and clamping otherwise. Bounded at [`MAX_SIDEBAR`]:
    /// overlong input keeps the first entries and drops the rest.
    pub fn set_entries(&mut self, entries: Vec<SessionRef>) {
        let current = self.entries.get(self.selected).cloned();
        self.entries = entries.into_iter().take(MAX_SIDEBAR).collect();
        self.selected = current
            .as_ref()
            .and_then(|s| self.entries.iter().position(|e| e == s))
            .unwrap_or(0);
        if self.selected >= self.entries.len() {
            self.selected = 0;
        }
    }

    /// Select an entry by session. `false` (selection untouched) when
    /// unknown or empty.
    pub fn select(&mut self, session: &SessionRef) -> bool {
        match self.entries.iter().position(|e| e == session) {
            Some(idx) => {
                self.selected = idx;
                true
            }
            None => false,
        }
    }

    /// Move selection down one; clamps at the last entry. `false` when empty.
    pub fn select_next(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        self.selected = (self.selected + 1).min(self.entries.len() - 1);
        true
    }

    /// Move selection up one; clamps at the first entry. `false` when empty.
    pub fn select_prev(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        self.selected = self.selected.saturating_sub(1);
        true
    }

    /// One page of the bounded roster, oldest first. Empty when `page` is
    /// out of range or `per_page` is zero (mirrors `tab_page` bounds).
    #[must_use]
    pub fn page(&self, page: usize, per_page: usize) -> &[SessionRef] {
        if per_page == 0 {
            return &[];
        }
        let start = page.saturating_mul(per_page);
        if start >= self.entries.len() {
            return &[];
        }
        let end = (start + per_page).min(self.entries.len());
        &self.entries[start..end]
    }
}

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
    /// Display-only directory fallback from the latest authenticated listing.
    /// Named projects and VCS metadata are not inferred from this value.
    project_label: Option<String>,
    pub draft: String,
    /// Composer cursor position, retained independently for every session tab.
    pub cursor: usize,
    pub scroll: usize,
    pub selected_model: Option<String>,
}

impl Tab {
    fn new(session: SessionRef) -> Self {
        Self {
            session,
            parent: None,
            project_label: None,
            draft: String::new(),
            cursor: 0,
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
#[derive(Clone, Debug, Default)]
pub struct NavigationState {
    tabs: Vec<Tab>,
    active: usize,
    home: bool,
    orientation: TabOrientation,
    /// Daemon-reported work state per session, retained even with no tab.
    work: Vec<(SessionRef, WorkState)>,
    /// Display titles with the revision that set them.
    titles: Vec<(SessionRef, String, u64)>,
    /// Unversioned session-list snapshot titles; never fabricate revisions.
    snapshot_titles: Vec<(SessionRef, String)>,
    /// Archive tombstones so stale renames cannot resurrect dead sessions.
    archived: Vec<(SessionRef, u64)>,
    /// Bounded in-memory session navigation history (`session-tabs-model.ts:147-176`).
    history: TabHistory,
    /// User-closed tabs eligible for reopen, in-memory only
    /// (`session-tabs-model.ts:70-101`). Never persisted.
    closed: Vec<ClosedTab>,
}

/// In-memory tab history. Upstream bounds it at 100 entries and keeps the
/// newest at `index` (`session-tabs-model.ts:147-176`).
const SESSION_TAB_HISTORY_LIMIT: usize = 100;
const CLOSED_SESSION_TAB_LIMIT: usize = 25;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TabHistory {
    entries: Vec<SessionRef>,
    index: Option<usize>,
}

/// One recently closed tab and the slot it occupied, so reopening restores the
/// original position (`session-tabs-model.ts:84-101`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosedTab {
    pub session: SessionRef,
    pub index: usize,
}

/// Serialized tab layout. Only order and titles survive a restart; the
/// recently-closed stack and navigation history are deliberately absent.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PersistedTabs {
    pub order: Vec<SessionRef>,
    pub titles: Vec<(SessionRef, String)>,
}

impl NavigationState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Current presentation orientation. Both orientations read the same
    /// bounded `tabs` collection and therefore share identity and state.
    #[must_use]
    pub fn orientation(&self) -> TabOrientation {
        self.orientation
    }

    /// Change only presentation; no tab is opened, closed, reordered, or
    /// reconstructed.
    pub fn set_orientation(&mut self, orientation: TabOrientation) {
        self.orientation = orientation;
    }

    /// Toggle the presentation orientation without changing active identity.
    pub fn toggle_orientation(&mut self) {
        self.orientation = match self.orientation {
            TabOrientation::Horizontal => TabOrientation::Vertical,
            TabOrientation::Vertical => TabOrientation::Horizontal,
        };
    }

    /// Open a tab for a session (or focus the existing one). When the list is
    /// full the oldest tab is evicted; focus stays on the previously active
    /// session unless it was the evicted one.
    pub fn open(&mut self, session: SessionRef) -> usize {
        self.home = false;
        if let Some(idx) = self.tabs.iter().position(|t| t.session == session) {
            self.focus_index(idx);
            return idx;
        }
        if self.tabs.len() >= MAX_TABS {
            self.tabs.remove(0);
            self.active = self.active.saturating_sub(1);
        }
        self.tabs.push(Tab::new(session));
        let idx = self.tabs.len() - 1;
        self.focus_index(idx);
        idx
    }

    /// Focus a tab slot and append it to the bounded navigation history.
    /// `context/session-tabs.tsx:211-220` records history on every routed
    /// session change; opening and switching are the native equivalents.
    fn focus_index(&mut self, index: usize) {
        self.active = index;
        self.home = false;
        let session = self.tabs[index].session.clone();
        self.record_history(&session);
    }

    /// Port of `recordSessionTabHistory` (`session-tabs-model.ts:147-156`):
    /// re-selecting the current entry is a no-op, otherwise the forward tail
    /// is replaced and the list is bounded from the oldest side.
    fn record_history(&mut self, session: &SessionRef) {
        let current = self
            .history
            .index
            .and_then(|index| self.history.entries.get(index));
        if current == Some(session) {
            return;
        }
        let keep = self
            .history
            .index
            .map_or(0, |index| (index + 1).min(self.history.entries.len()));
        let mut entries: Vec<SessionRef> = self.history.entries[..keep].to_vec();
        entries.push(session.clone());
        if entries.len() > SESSION_TAB_HISTORY_LIMIT {
            let excess = entries.len() - SESSION_TAB_HISTORY_LIMIT;
            entries.drain(..excess);
        }
        self.history.entries = entries;
        self.history.index = self.history.entries.len().checked_sub(1);
    }

    /// Port of `moveSessionTabHistory` (`session-tabs-model.ts:158-176`) for
    /// the backward direction used by close: walk history backwards for the
    /// newest entry that is still open and is not the closing tab.
    fn history_previous(&self, open: &[SessionRef], closing: &SessionRef) -> Option<SessionRef> {
        let index = self.history.index?;
        self.history.entries[..index.min(self.history.entries.len())]
            .iter()
            .rev()
            .find(|entry| *entry != closing && open.contains(entry))
            .cloned()
    }

    /// Bounded in-memory history entries, oldest first. History is never
    /// persisted, so it is empty after a restart restore.
    #[must_use]
    pub fn history_entries(&self) -> &[SessionRef] {
        &self.history.entries
    }

    /// User-closed tabs that are still closed, oldest first. Upstream
    /// `recentlyClosed` filters entries already open again
    /// (`context/session-tabs.tsx:381-389`).
    #[must_use]
    pub fn recently_closed(&self) -> Vec<ClosedTab> {
        self.closed
            .iter()
            .filter(|entry| !self.tabs.iter().any(|tab| tab.session == entry.session))
            .cloned()
            .collect()
    }

    /// Reopen a closed tab at its original slot. Without an explicit session
    /// the most recent entry is used. Already-open entries are consumed so
    /// repeated reopens walk the stack (`session-tabs-model.ts:91-111`).
    pub fn reopen(&mut self, session: Option<&SessionRef>) -> Option<SessionRef> {
        loop {
            let index = match session {
                Some(target) => self.closed.iter().position(|entry| &entry.session == target)?,
                None => self.closed.len().checked_sub(1)?,
            };
            let entry = self.closed.remove(index);
            if self.tabs.iter().any(|tab| tab.session == entry.session) {
                if session.is_some() {
                    return None;
                }
                continue;
            }
            let slot = entry.index.min(self.tabs.len());
            self.tabs.insert(slot, Tab::new(entry.session.clone()));
            self.focus_index(slot);
            return Some(entry.session);
        }
    }

    /// Persisted tab layout: order plus display titles. Upstream persists
    /// `{ tabs, unread }` per scope (`context/session-tabs.tsx:72-102`); the
    /// active route, recently-closed stack and navigation history stay
    /// in-memory by design.
    #[must_use]
    pub fn persist_tabs(&self) -> PersistedTabs {
        PersistedTabs {
            order: self.tabs.iter().map(|tab| tab.session.clone()).collect(),
            titles: self
                .tabs
                .iter()
                .filter_map(|tab| {
                    self.title(&tab.session)
                        .map(|title| (tab.session.clone(), title.to_owned()))
                })
                .collect(),
        }
    }

    /// Restore a persisted order. Duplicates are dropped, the list is bounded,
    /// and focus starts at the first restored tab because the active route is
    /// not part of the persisted layout.
    pub fn restore_tabs(&mut self, persisted: &PersistedTabs) {
        self.tabs.clear();
        self.active = 0;
        self.home = false;
        for session in &persisted.order {
            if self.tabs.len() >= MAX_TABS {
                break;
            }
            if self.tabs.iter().any(|tab| &tab.session == session) {
                continue;
            }
            self.tabs.push(Tab::new(session.clone()));
        }
        let titles: Vec<(SessionRef, String)> = persisted.titles.clone();
        for (session, title) in titles {
            if self.tabs.iter().any(|tab| tab.session == session) {
                self.ingest_snapshot_title(session, title);
            }
        }
        self.history = TabHistory::default();
        self.closed.clear();
    }

    /// Explicit tab lifecycle name used by native view callers. This is an
    /// alias over the single identity model, not a second tab framework.
    pub fn open_tab(&mut self, session: SessionRef) -> usize {
        self.open(session)
    }

    /// Open a tab for a forked child, preserving provenance (T02).
    pub fn open_forked(&mut self, child: SessionRef, parent: SessionRef) -> usize {
        let idx = self.open(child);
        self.tabs[idx].parent = Some(parent);
        idx
    }

    /// Close a tab by session. Daemon-owned work keeps running (T05): the
    /// work entry is retained and echoed in the returned receipt. The closed
    /// slot is remembered in memory for reopen; background closes never move
    /// focus (`context/session-tabs.tsx:326-374`).
    pub fn close(&mut self, session: &SessionRef) -> Result<CloseReceipt, NavRefusal> {
        let index = self
            .tabs
            .iter()
            .position(|tab| &tab.session == session)
            .ok_or(NavRefusal::NoSuchTab)?;
        self.closed.retain(|entry| &entry.session != session);
        self.closed.push(ClosedTab {
            session: session.clone(),
            index,
        });
        if self.closed.len() > CLOSED_SESSION_TAB_LIMIT {
            self.closed.remove(0);
        }
        self.remove_tab(session);
        Ok(CloseReceipt {
            session: session.clone(),
            daemon_work: self.work_state(session),
        })
    }

    /// Close the focused tab. The successor is the newest still-open history
    /// entry, falling back to the right neighbor and then the left one
    /// (`context/session-tabs.tsx:326-374` `previous.sessionID ?? closed.next`).
    /// The daemon work map is intentionally untouched.
    pub fn close_active(&mut self) -> Result<CloseReceipt, NavRefusal> {
        let session = self
            .active()
            .map(|tab| tab.session.clone())
            .ok_or(NavRefusal::NoTabOpen)?;
        self.record_history(&session);
        let open: Vec<SessionRef> = self.tabs.iter().map(|tab| tab.session.clone()).collect();
        let successor = self
            .history_previous(&open, &session)
            .or_else(|| self.active_successor());
        let receipt = self.close(&session)?;
        if let Some(next) = successor {
            if self.tabs.iter().any(|tab| tab.session == next) {
                let _ = self.switch(&next);
            }
        }
        Ok(receipt)
    }

    /// Return the upstream close fallback without consulting history: the
    /// right neighbor first, otherwise the left neighbor.
    #[must_use]
    pub fn active_successor(&self) -> Option<SessionRef> {
        let active = self.active()?.session.clone();
        let index = self.tabs.iter().position(|tab| tab.session == active)?;
        self.tabs
            .get(index + 1)
            .or_else(|| index.checked_sub(1).and_then(|i| self.tabs.get(i)))
            .map(|tab| tab.session.clone())
    }

    /// Save the complete client-local state for the active tab. `cursor` is a
    /// UTF-8 byte offset, matching the native composer boundary.
    pub fn save_active_state(
        &mut self,
        draft: String,
        cursor: usize,
        scroll: usize,
        selected_model: Option<String>,
    ) -> Result<(), NavRefusal> {
        let tab = self.active_mut().ok_or(NavRefusal::NoTabOpen)?;
        tab.cursor = cursor.min(draft.len());
        while tab.cursor > 0 && !draft.is_char_boundary(tab.cursor) {
            tab.cursor -= 1;
        }
        tab.draft = draft;
        tab.scroll = scroll;
        tab.selected_model = selected_model;
        Ok(())
    }

    /// Testable generation gate used by live worker/callback consumers.
    #[must_use]
    pub fn accepts_generation(
        current: u64,
        session: &SessionRef,
        event_generation: u64,
        event_session: &SessionRef,
    ) -> bool {
        current == event_generation && session == event_session
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
        self.focus_index(idx);
        Ok(idx)
    }

    /// Select an existing tab by identity.
    pub fn select_tab(&mut self, session: &SessionRef) -> Result<usize, NavRefusal> {
        self.switch(session)
    }

    /// Reorder one open tab to `index`, mirroring upstream `moveSessionTab`
    /// (`session-tabs-model.ts:93-101`): remove, then splice at the target
    /// slot, clamped to the last open tab. Per-tab state (draft, cursor,
    /// scroll, model) travels with the tab. Focus follows the tab it pointed
    /// at; returns whether the order actually changed.
    pub fn move_tab(&mut self, session: &SessionRef, index: usize) -> bool {
        let Some(from) = self.tabs.iter().position(|tab| tab.session == *session) else {
            return false;
        };
        let to = index.min(self.tabs.len().saturating_sub(1));
        if from == to {
            return false;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.active = if self.active == from {
            to
        } else if from < self.active && to >= self.active {
            self.active - 1
        } else if from > self.active && to <= self.active {
            self.active + 1
        } else {
            self.active
        };
        true
    }

    /// Move one session-tab slot, wrapping at either end. This is the native
    /// counterpart of upstream `session.tab.next`/`previous`; it only changes
    /// the local selection and never creates a tab.
    pub fn step_tab(&mut self, forward: bool) -> Option<usize> {
        if self.tabs.is_empty() {
            return None;
        }
        let next = if forward {
            (self.active + 1) % self.tabs.len()
        } else {
            self.active.checked_sub(1).unwrap_or(self.tabs.len() - 1)
        };
        self.focus_index(next);
        Some(next)
    }

    /// Cycle one tab in either direction, skipping slots whose session does
    /// not match. This is the native form of `cycleUnread`
    /// (`context/session-tabs.tsx:484-490`), used by the unread shortcuts.
    pub fn step_tab_where(
        &mut self,
        forward: bool,
        matches: impl Fn(&SessionRef) -> bool,
    ) -> Option<usize> {
        if self.tabs.is_empty() {
            return None;
        }
        for offset in 1..=self.tabs.len() {
            let index = if forward {
                (self.active + offset) % self.tabs.len()
            } else {
                (self.active + self.tabs.len() * 2 - offset) % self.tabs.len()
            };
            if matches(&self.tabs[index].session) {
                self.focus_index(index);
                return Some(index);
            }
        }
        None
    }

    /// Select a one-based quick tab slot. Out-of-range slots are a no-op.
    pub fn select_tab_slot(&mut self, slot: usize) -> Option<usize> {
        let index = slot.checked_sub(1)?;
        if index >= self.tabs.len() {
            return None;
        }
        self.focus_index(index);
        Some(index)
    }

    /// The focused tab.
    #[must_use]
    pub fn active(&self) -> Option<&Tab> {
        if self.home { None } else { self.tabs.get(self.active) }
    }

    /// Mutable focused tab (draft/scroll/model edits are client-local).
    pub fn active_mut(&mut self) -> Option<&mut Tab> {
        if self.home { None } else { self.tabs.get_mut(self.active) }
    }

    /// Enter Home without deleting or mutating retained tab-local state.
    pub fn enter_home(&mut self) {
        self.home = true;
    }

    /// The explicit Home route fact for horizontal tab painting: upstream
    /// `route.data.type === "home"` drives newTab() (context/session-tabs.tsx
    /// newTab createMemo: `if (route.data.type === "home") return true`).
    #[must_use]
    pub fn is_home(&self) -> bool {
        self.home
    }

    /// Source-backed close(None) target while on Home: upstream selects the
    /// history-previous session, falling back to the last remaining tab
    /// (context/session-tabs.tsx close(): `previous.sessionID ??
    /// state().tabs.at(-1)?.sessionID`). The canonical model has no per-strip
    /// history, so the retained focused tab before `enter_home` is the
    /// previous fact, and the final tab is the fallback; without any tab the
    /// route stays Home (upstream navigates only `if (route.data.type ===
    /// "home" && session)`).
    #[must_use]
    pub fn home_dispatch_target(&self) -> Option<SessionRef> {
        if !self.home || self.tabs.is_empty() {
            return None;
        }
        let index = self.active.min(self.tabs.len() - 1);
        Some(self.tabs[index].session.clone())
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

    /// Replace labels only on open tabs, so a missing directory never leaves a
    /// previously observed label visible and memory is bounded by MAX_TABS.
    pub fn replace_project_labels(&mut self, labels: &[(SessionRef, Option<String>)]) {
        for tab in &mut self.tabs {
            tab.project_label = labels.iter()
                .find(|(session, _)| *session == tab.session)
                .and_then(|(_, label)| label.clone());
        }
    }

    #[must_use]
    pub fn project_label(&self, session: &SessionRef) -> Option<&str> {
        self.tabs.iter().find(|tab| &tab.session == session)
            .and_then(|tab| tab.project_label.as_deref())
    }

    #[must_use]
    pub fn tab_at(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
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
            DaemonEvent::Renamed {
                session,
                title,
                rev,
            } => {
                if self.tombstone_at_or_above(&session, rev) {
                    return;
                }
                let accepted = match self.titles.iter().position(|(s, _, _)| s == &session) {
                    Some(index) if self.titles[index].2 <= rev => {
                        let entry = &mut self.titles[index];
                        entry.1 = title;
                        entry.2 = rev;
                        true
                    }
                    Some(_) => false,
                    None if self.titles.len() < MAX_TABS => {
                        self.titles.push((session.clone(), title, rev));
                        true
                    }
                    None => false,
                };
                if accepted {
                    self.snapshot_titles.retain(|(sid, _)| sid != &session);
                }
            }
            DaemonEvent::Archived { session, rev } => {
                if self.tombstone_at_or_above(&session, rev) {
                    return;
                }
                self.remove_tab(&session);
                self.closed.retain(|entry| entry.session != session);
                self.titles.retain(|(s, _, _)| *s != session);
                self.snapshot_titles.retain(|(s, _)| *s != session);
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
            .or_else(|| self.snapshot_titles.iter().find(|(s, _)| s == session).map(|(_, t)| t.as_str()))
    }

    fn bound_snapshot_title(title: String) -> String {
        const MAX_TITLE_BYTES: usize = 512;
        if title.len() <= MAX_TITLE_BYTES { return title; }
        let mut end = MAX_TITLE_BYTES;
        while !title.is_char_boundary(end) { end -= 1; }
        title[..end].to_owned()
    }

    /// Store an authenticated listing title without manufacturing a revision.
    pub fn ingest_snapshot_title(&mut self, session: SessionRef, title: String) {
        if self.archived.iter().any(|(s, _)| s == &session) { return; }
        let title = Self::bound_snapshot_title(title);
        if let Some(index) = self.snapshot_titles.iter().position(|(s, _)| s == &session) {
            // Refresh recency deterministically: the most recently observed
            // session is retained when the bounded cache must evict.
            self.snapshot_titles.remove(index);
            self.snapshot_titles.push((session, title));
        } else {
            if self.snapshot_titles.len() >= MAX_SNAPSHOT_TITLES {
                // FIFO over refresh order is a bounded LRU approximation with
                // no clock or synthetic revision. Callers seed the active
                // target last, so it cannot be evicted by a large listing.
                self.snapshot_titles.remove(0);
            }
            self.snapshot_titles.push((session, title));
        }
    }

    pub fn ingest_snapshot_titles<I>(&mut self, titles: I)
    where I: IntoIterator<Item = (SessionRef, String)> {
        for (session, title) in titles { self.ingest_snapshot_title(session, title); }
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

// ---------------------------------------------------------------------------
// Focus-confirmed view acknowledgement.
//
// Upstream `packages/tui/src/context/session-tabs.tsx:53-55,229-267`: the
// unread watermark is acknowledged only while the terminal is focused and the
// route shows that session; the watermark is recorded before the request, a
// failure clears it, and retries back off 250 ms -> 5 s while a success resets.
// ---------------------------------------------------------------------------

pub const VIEW_RETRY_DELAY_MS: u64 = 250;
pub const VIEW_RETRY_MAX_DELAY_MS: u64 = 5_000;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewAcknowledgement {
    focused: bool,
    idle: Option<u64>,
    viewed: Option<u64>,
    acknowledged: Option<u64>,
    retry_at: Option<u64>,
    attempts: u32,
}

impl ViewAcknowledgement {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed the session's timestamps and focus state. `idle` is the newest
    /// idle transition; `viewed` is the server-global watermark.
    pub fn observe(&mut self, focused: bool, idle: Option<u64>, viewed: Option<u64>) {
        self.focused = focused;
        self.idle = idle;
        self.viewed = viewed;
    }

    /// `context/session-tabs.tsx:120-123` `isUnread`.
    #[must_use]
    pub fn unread(&self) -> bool {
        match self.idle {
            Some(idle) => self.viewed.is_none_or(|viewed| idle > viewed),
            None => false,
        }
    }

    /// Return the watermark to POST, recording it first so event-driven
    /// re-runs do not duplicate the request. Retries are gated by `now`.
    pub fn poll(&mut self, now_ms: u64) -> Option<u64> {
        if !self.focused || !self.unread() {
            return None;
        }
        let idle = self.idle?;
        if self.acknowledged == Some(idle) {
            return None;
        }
        if self.retry_at.is_some_and(|at| now_ms < at) {
            return None;
        }
        self.retry_at = None;
        self.acknowledged = Some(idle);
        Some(idle)
    }

    /// The request failed: forget the watermark for this idle and schedule the
    /// next attempt, capped at 5 s. Returns the delay used.
    pub fn failed(&mut self, now_ms: u64) -> u64 {
        self.acknowledged = None;
        let delay = VIEW_RETRY_DELAY_MS
            .saturating_mul(1u64 << self.attempts.min(8))
            .min(VIEW_RETRY_MAX_DELAY_MS);
        self.attempts = self.attempts.saturating_add(1);
        self.retry_at = Some(now_ms.saturating_add(delay));
        delay
    }

    /// The request succeeded: clear the timer and the backoff.
    pub fn succeeded(&mut self) {
        self.retry_at = None;
        self.attempts = 0;
    }

    #[must_use]
    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}

// ---------------------------------------------------------------------------
// Tab commands and shortcuts.
//
// Upstream `packages/tui/src/app.tsx:854-902` defines the `session.tab.*`
// commands and `packages/tui/src/config/keybind.ts:113-120,152-161` binds
// default keys. A customized binding only replaces the key; it still resolves
// to the same command name and therefore the same behavior.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabCommand {
    Next,
    Previous,
    NextUnread,
    PreviousUnread,
    Close,
    Reopen,
    Select(usize),
}

impl TabCommand {
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "session.tab.next" => Some(Self::Next),
            "session.tab.previous" => Some(Self::Previous),
            "session.tab.next_unread" => Some(Self::NextUnread),
            "session.tab.previous_unread" => Some(Self::PreviousUnread),
            "session.tab.close" => Some(Self::Close),
            "session.tab.reopen" => Some(Self::Reopen),
            _ => {
                let slot = name.strip_prefix("session.tab.select.")?;
                let slot: usize = slot.parse().ok()?;
                (1..=10).contains(&slot).then_some(Self::Select(slot - 1))
            }
        }
    }

    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Next => "session.tab.next".to_owned(),
            Self::Previous => "session.tab.previous".to_owned(),
            Self::NextUnread => "session.tab.next_unread".to_owned(),
            Self::PreviousUnread => "session.tab.previous_unread".to_owned(),
            Self::Close => "session.tab.close".to_owned(),
            Self::Reopen => "session.tab.reopen".to_owned(),
            Self::Select(slot) => format!("session.tab.select.{}", slot + 1),
        }
    }
}

/// Default bindings from the pinned `config/keybind.ts:113-120,152-161`.
pub const DEFAULT_TAB_KEYBINDS: &[(&str, &str)] = &[
    ("ctrl+tab", "session.tab.next"),
    ("alt+down", "session.tab.next"),
    ("ctrl+shift+tab", "session.tab.previous"),
    ("alt+up", "session.tab.previous"),
    ("alt+shift+down", "session.tab.next_unread"),
    ("alt+shift+up", "session.tab.previous_unread"),
    ("<leader>w", "session.tab.close"),
    ("ctrl+shift+t", "session.tab.reopen"),
    ("<leader>1", "session.tab.select.1"),
    ("ctrl+1", "session.tab.select.1"),
    ("<leader>2", "session.tab.select.2"),
    ("ctrl+2", "session.tab.select.2"),
    ("<leader>3", "session.tab.select.3"),
    ("ctrl+3", "session.tab.select.3"),
    ("<leader>4", "session.tab.select.4"),
    ("ctrl+4", "session.tab.select.4"),
    ("<leader>5", "session.tab.select.5"),
    ("ctrl+5", "session.tab.select.5"),
    ("<leader>6", "session.tab.select.6"),
    ("ctrl+6", "session.tab.select.6"),
    ("<leader>7", "session.tab.select.7"),
    ("ctrl+7", "session.tab.select.7"),
    ("<leader>8", "session.tab.select.8"),
    ("ctrl+8", "session.tab.select.8"),
    ("<leader>9", "session.tab.select.9"),
    ("ctrl+9", "session.tab.select.9"),
    ("<leader>0", "session.tab.select.10"),
    ("ctrl+0", "session.tab.select.10"),
];

/// First matching binding. Defaults and user-customized tables share this
/// resolver, so both dispatch the same `TabCommand`.
#[must_use]
pub fn resolve_tab_shortcut(bindings: &[(&str, &str)], key: &str) -> Option<TabCommand> {
    bindings
        .iter()
        .find(|(binding, _)| *binding == key)
        .and_then(|(_, name)| TabCommand::from_name(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: &str) -> SessionRef {
        SessionRef(id.to_string())
    }

    #[test]
    fn home_retains_tabs_without_exposing_active_state() {
        let mut nav = NavigationState::new();
        let id = s("home-retained");
        nav.open(id.clone());
        nav.save_active_state("draft".into(), 5, 3, Some("model".into())).unwrap();
        nav.enter_home();
        assert!(nav.active().is_none());
        assert_eq!(nav.tab_len(), 1);
        nav.open(id);
        assert_eq!(nav.active().unwrap().draft, "draft");
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
            tab.cursor = 3;
            tab.selected_model = Some("model-a".into());
            tab.scroll = 7;
        }
        assert_eq!(nav.active().unwrap().draft, "draft a");
        assert_eq!(nav.active().unwrap().scroll, 7);
        assert_eq!(nav.active().unwrap().cursor, 3);
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
    fn save_active_state_normalizes_cursor_to_utf8_boundary() {
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.save_active_state("世界".into(), 1, 4, None).unwrap();
        let tab = nav.active().unwrap();
        assert_eq!(tab.cursor, 0);
        assert_eq!(tab.scroll, 4);
        nav.save_active_state("世界".into(), 99, 4, None).unwrap();
        assert_eq!(nav.active().unwrap().cursor, "世界".len());
    }

    #[test]
    fn generation_gate_rejects_stale_session_and_accepts_current_successor() {
        let a = s("a");
        let b = s("b");
        assert!(!NavigationState::accepts_generation(2, &b, 1, &b));
        assert!(!NavigationState::accepts_generation(2, &b, 2, &a));
        assert!(NavigationState::accepts_generation(2, &b, 2, &b));
    }

    #[test]
    fn horizontal_and_vertical_views_share_identity_and_preserve_tab_state() {
        let mut nav = NavigationState::new();
        nav.open_tab(s("a"));
        nav.active_mut().unwrap().draft = "unsent".into();
        nav.active_mut().unwrap().cursor = 5;
        nav.set_orientation(TabOrientation::Vertical);
        assert_eq!(nav.orientation(), TabOrientation::Vertical);
        assert_eq!(nav.tabs().len(), 1);
        nav.toggle_orientation();
        assert_eq!(nav.orientation(), TabOrientation::Horizontal);
        assert_eq!(nav.active().unwrap().session, s("a"));
        assert_eq!(nav.active().unwrap().draft, "unsent");
        assert_eq!(nav.active().unwrap().cursor, 5);
    }

    #[test]
    fn close_active_selects_right_then_left_neighbor_without_daemon_side_effects() {
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.open(s("b"));
        nav.open(s("c"));
        nav.apply(DaemonEvent::WorkState {
            session: s("b"),
            state: WorkState::Running,
        });
        nav.switch(&s("b")).unwrap();
        nav.close_active().unwrap();
        assert_eq!(nav.active().unwrap().session, s("c"));
        assert_eq!(nav.work_state(&s("b")), Some(WorkState::Running));
        nav.close_active().unwrap();
        assert_eq!(nav.active().unwrap().session, s("a"));
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
    fn close_clicked_background_tab_preserves_active_draft_unicode_cursor_and_daemon_work() {
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.open(s("b"));
        nav.open(s("c"));
        // "b" is active with a non-empty Unicode draft and cursor.
        nav.switch(&s("b")).unwrap();
        nav.active_mut().unwrap().draft = "hello 世界 🌍".into();
        nav.active_mut().unwrap().cursor = 12;
        // "c" has daemon-owned running work.
        nav.apply(DaemonEvent::WorkState {
            session: s("c"),
            state: WorkState::Running,
        });

        // Simulate a mouse click on "c"'s close button while "b" is active.
        // The close action targets hit.session ("c"), NOT active ("b").
        let receipt = nav.close(&s("c")).unwrap();
        assert_eq!(receipt.session, s("c"));
        assert_eq!(receipt.retained_work(), Some(WorkState::Running));

        // Active tab ("b") is unchanged: same session, draft, cursor.
        assert_eq!(nav.active().unwrap().session, s("b"));
        assert_eq!(nav.active().unwrap().draft, "hello 世界 🌍");
        assert_eq!(nav.active().unwrap().cursor, 12);

        // Daemon work for the closed session is still retained.
        assert_eq!(nav.work_state(&s("c")), Some(WorkState::Running));

        // The closed tab is removed from the tab list.
        assert_eq!(nav.tab_len(), 2);
        assert!(!nav.tabs().iter().any(|t| t.session == s("c")));
    }

    #[test]
    fn close_clicked_background_tab_does_not_cascade_to_active_tab() {
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.open(s("b"));
        nav.open(s("c"));
        nav.open(s("d"));
        nav.switch(&s("b")).unwrap();

        // Close "d" (background, right of active "b").
        let r1 = nav.close(&s("d")).unwrap();
        assert_eq!(r1.session, s("d"));
        assert_eq!(nav.active().unwrap().session, s("b"));

        // Close "a" (background, left of active "b").
        let r2 = nav.close(&s("a")).unwrap();
        assert_eq!(r2.session, s("a"));
        assert_eq!(nav.active().unwrap().session, s("b"));

        // Active tab "b" is still focused and intact.
        assert_eq!(nav.tab_len(), 2);
    }

    #[test]
    fn close_clicked_background_tab_missing_returns_nosuchtab_without_mutation() {
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.open(s("b"));
        nav.switch(&s("b")).unwrap();
        nav.active_mut().unwrap().draft = "keep".into();

        // Close a non-existent tab (stale click).
        let err = nav.close(&s("ghost")).unwrap_err();
        assert_eq!(err, NavRefusal::NoSuchTab);

        // Active tab and its draft are untouched.
        assert_eq!(nav.active().unwrap().session, s("b"));
        assert_eq!(nav.active().unwrap().draft, "keep");
        assert_eq!(nav.tab_len(), 2);
    }

    #[test]
    fn close_active_with_work_state_retains_daemon_work() {
        // Verifies the explicit user extension: active close still retains
        // daemon work (same as close_active) while also being callable via
        // the navigation.close entry used for background closes.
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.apply(DaemonEvent::WorkState {
            session: s("a"),
            state: WorkState::Running,
        });
        let receipt = nav.close(&s("a")).unwrap();
        assert_eq!(receipt.session, s("a"));
        assert_eq!(receipt.retained_work(), Some(WorkState::Running));
        assert_eq!(nav.work_state(&s("a")), Some(WorkState::Running));
        assert_eq!(nav.tab_len(), 0);
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
            ForkRequest {
                parent: s("parent")
            }
        );
        assert_eq!(
            nav.resume_request(&s("child")).unwrap(),
            ResumeRequest {
                session: s("child")
            }
        );
        assert_eq!(nav.fork_request(&s("ghost")), Err(NavRefusal::NoSuchTab));
        assert_eq!(nav.resume_request(&s("ghost")), Err(NavRefusal::NoSuchTab));
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

    #[test]
    fn keyboard_tab_steps_wrap_and_invalid_slots_do_not_mutate() {
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.open(s("b"));
        nav.open(s("c"));
        assert_eq!(nav.active().unwrap().session, s("c"));
        assert_eq!(nav.step_tab(true), Some(0));
        assert_eq!(nav.active().unwrap().session, s("a"));
        assert_eq!(nav.step_tab(false), Some(2));
        assert_eq!(nav.select_tab_slot(2), Some(1));
        assert_eq!(nav.active().unwrap().session, s("b"));
        assert_eq!(nav.select_tab_slot(0), None);
        assert_eq!(nav.select_tab_slot(4), None);
        assert_eq!(nav.active().unwrap().session, s("b"));
    }

    // RED (compile, fail on missing behavior): focus cycling must match
    // `NativeApp::cycle_focus` / `cycle_focus_prev` incl. the collapsed
    // sidebar skip, and the sidebar roster must keep a bounded selection.

    #[test]
    fn focus_next_cycles_order_and_skips_collapsed_sidebar() {
        // Tab order, full sidebar.
        assert_eq!(focus_next(Focus::Composer, false), Focus::Transcript);
        assert_eq!(focus_next(Focus::Transcript, false), Focus::Sidebar);
        assert_eq!(focus_next(Focus::Sidebar, false), Focus::Status);
        assert_eq!(focus_next(Focus::Status, false), Focus::Composer);
        // Collapsed sidebar: Transcript jumps straight to Status.
        assert_eq!(focus_next(Focus::Transcript, true), Focus::Status);
        assert_eq!(focus_next(Focus::Composer, true), Focus::Transcript);
        assert_eq!(focus_next(Focus::Status, true), Focus::Composer);
    }

    #[test]
    fn focus_prev_cycles_order_and_skips_collapsed_sidebar() {
        assert_eq!(focus_prev(Focus::Composer, false), Focus::Status);
        assert_eq!(focus_prev(Focus::Status, false), Focus::Sidebar);
        assert_eq!(focus_prev(Focus::Sidebar, false), Focus::Transcript);
        assert_eq!(focus_prev(Focus::Transcript, false), Focus::Composer);
        // Collapsed sidebar: Status jumps straight to Transcript.
        assert_eq!(focus_prev(Focus::Status, true), Focus::Transcript);
        assert_eq!(focus_prev(Focus::Composer, true), Focus::Status);
        assert_eq!(focus_prev(Focus::Transcript, true), Focus::Composer);
    }

    #[test]
    fn focus_round_trips_and_sidebar_fallback() {
        for focus in Focus::ORDER {
            assert_eq!(focus_next(focus, false).prev(), focus);
            assert_eq!(focus_prev(focus, false).next(), focus);
        }
        // Stranded sidebar request falls back to transcript.
        assert_eq!(
            resolve_sidebar_focus(Focus::Sidebar, true),
            Focus::Transcript
        );
        assert_eq!(resolve_sidebar_focus(Focus::Sidebar, false), Focus::Sidebar);
        assert_eq!(
            resolve_sidebar_focus(Focus::Composer, true),
            Focus::Composer
        );
    }

    #[test]
    fn sidebar_selection_moves_and_clamps() {
        let mut bar = SidebarNav::new();
        assert!(bar.is_empty());
        assert_eq!(bar.len(), 0);
        assert_eq!(bar.selected(), None);
        assert!(!bar.select_next());
        assert!(!bar.select_prev());
        assert!(!bar.select(&s("x")));
        bar.set_entries(vec![s("a"), s("b"), s("c")]);
        assert_eq!(bar.len(), 3);
        assert_eq!(bar.selected(), Some(&s("a")));
        assert_eq!(bar.selected_index(), 0);
        assert!(bar.select(&s("c")));
        assert_eq!(bar.selected_index(), 2);
        // Clamp at the ends.
        assert!(bar.select_next());
        assert_eq!(bar.selected(), Some(&s("c")));
        assert!(bar.select_prev());
        assert!(bar.select_prev());
        assert_eq!(bar.selected(), Some(&s("a")));
        assert!(bar.select_prev());
        assert_eq!(bar.selected(), Some(&s("a")));
        // Unknown session leaves the selection untouched.
        assert!(!bar.select(&s("ghost")));
        assert_eq!(bar.selected(), Some(&s("a")));
    }

    #[test]
    fn sidebar_roster_is_bounded_and_paginates() {
        let mut bar = SidebarNav::new();
        let many: Vec<SessionRef> = (0..(MAX_SIDEBAR + 8))
            .map(|i| s(&format!("s{i}")))
            .collect();
        bar.set_entries(many);
        assert_eq!(MAX_SIDEBAR, 32);
        assert_eq!(bar.len(), MAX_SIDEBAR);
        assert_eq!(bar.entries().first(), Some(&s("s0")));
        assert_eq!(bar.entries().last(), Some(&s("s31")));
        assert_eq!(bar.page(0, 16).len(), 16);
        assert_eq!(bar.page(0, 16)[0], s("s0"));
        assert_eq!(bar.page(1, 16).len(), 16);
        assert!(bar.page(2, 16).is_empty());
        assert!(bar.page(0, 0).is_empty());
        // Replacing the roster keeps the selection when it survives ...
        assert!(bar.select(&s("s5")));
        bar.set_entries(vec![s("s5"), s("s6")]);
        assert_eq!(bar.selected(), Some(&s("s5")));
        // ... and clamps to a valid entry when it does not.
        bar.set_entries(vec![s("x"), s("y"), s("z")]);
        assert_eq!(bar.selected(), Some(&s("x")));
        bar.set_entries(vec![]);
        assert_eq!(bar.selected(), None);
        assert_eq!(bar.selected_index(), 0);
    }

    #[test]
    fn snapshot_titles_are_separate_from_revisioned_renames_and_tombstones() {
        let mut nav = NavigationState::new();
        let a = s("a");
        let b = s("b");
        nav.ingest_snapshot_titles(vec![(a.clone(), "Alpha".into()), (b.clone(), "Beta".into())]);
        assert_eq!(nav.title(&a), Some("Alpha"));
        assert_eq!(nav.title(&b), Some("Beta"));
        nav.apply(DaemonEvent::Renamed { session: a.clone(), title: "A2".into(), rev: 1 });
        assert_eq!(nav.title(&a), Some("A2"));
        assert_eq!(nav.title(&b), Some("Beta"));
        nav.apply(DaemonEvent::Archived { session: a.clone(), rev: 2 });
        nav.ingest_snapshot_title(a.clone(), "zombie".into());
        assert_eq!(nav.title(&a), None);
    }

    #[test]
    fn snapshot_cache_evicts_oldest_refresh_and_retains_target_seeded_last() {
        let mut nav = NavigationState::new();
        for i in 0..MAX_SNAPSHOT_TITLES {
            nav.ingest_snapshot_title(s(&format!("s{i}")), format!("title-{i}"));
        }
        // Refresh s0, making s1 the deterministic eviction victim.
        nav.ingest_snapshot_title(s("s0"), "refreshed".into());
        nav.ingest_snapshot_title(s("target"), "target-title".into());
        assert_eq!(nav.title(&s("s0")), Some("refreshed"));
        assert_eq!(nav.title(&s("s1")), None);
        assert_eq!(nav.title(&s("target")), Some("target-title"));
        assert_eq!(nav.title(&s("s31")), Some("title-31"));
    }

    #[test]
    fn snapshot_title_bound_is_utf8_safe_and_preserves_empty() {
        let mut nav = NavigationState::new();
        let a = s("a");
        nav.ingest_snapshot_title(a.clone(), "é".repeat(300));
        assert_eq!(nav.title(&a).unwrap().len(), 512);
        nav.ingest_snapshot_title(a.clone(), String::new());
        assert_eq!(nav.title(&a), Some(""));
    }


    #[test]
    fn bounded_tombstone_horizon_is_explicit() {
        let mut nav = NavigationState::new();
        let archived = s("archived");
        nav.apply(DaemonEvent::Archived { session: archived.clone(), rev: 1 });
        for i in 0..MAX_TABS {
            nav.apply(DaemonEvent::Archived { session: s(&format!("other-{i}")), rev: 1 });
        }
        // Existing bounded FIFO tombstone behavior: after the horizon passes,
        // the old tombstone is no longer retained and snapshot ingestion may
        // observe that limitation rather than pretending archive durability.
        nav.ingest_snapshot_title(archived.clone(), "reobserved".into());
        assert_eq!(nav.title(&archived), Some("reobserved"));
    }


    #[test]
    fn home_dispatch_uses_retained_facts_only_and_stays_home_without_tabs() {
        let mut nav = NavigationState::default();
        assert_eq!(home_side(nav.home_dispatch_target()), None, "Home with no tabs has no target and the route stays Home");
        nav.open(s("retained"));
        nav.open(s("other"));
        nav.enter_home();
        assert!(nav.is_home(), "enter_home is the explicit route fact");
        // Canonical close(None) mapping: the retained focused tab is the
        // previous-session fact; upstream per-tab move history is not modeled
        // here and stays an explicit documented gap.
        assert_eq!(home_side(nav.home_dispatch_target()), Some(s("other")));
        nav.switch(&s("retained"));
        nav.enter_home();
        assert_eq!(home_side(nav.home_dispatch_target()), Some(s("retained")));
    }

    fn home_side(target: Option<SessionRef>) -> Option<SessionRef> {
        // Identity-only seam so the assertion messages above stay readable.
        target
    }

    #[test]
    fn tab_19_close_active_chooses_mru_then_neighbor_and_reopen_restores_slot() {
        // context/session-tabs.tsx:326-374: successor = newest open history
        // entry, else right-then-left; background close keeps focus; reopen
        // restores the recorded slot.
        let mut nav = NavigationState::new();
        for id in ["a", "b", "c", "d"] {
            nav.open(s(id));
        }
        nav.switch(&s("a")).unwrap();
        nav.switch(&s("c")).unwrap();
        nav.close_active().unwrap();
        assert_eq!(nav.active().unwrap().session, s("a"), "MRU wins over the right neighbor");
        assert_eq!(
            nav.tabs().iter().map(|tab| tab.session.clone()).collect::<Vec<_>>(),
            vec![s("a"), s("b"), s("d")]
        );

        // Background close leaves the active tab untouched.
        nav.switch(&s("d")).unwrap();
        nav.close(&s("b")).unwrap();
        assert_eq!(nav.active().unwrap().session, s("d"));
        assert_eq!(nav.recently_closed().last().unwrap().session, s("b"));
        // Reopen restores b at its original slot 1 and focuses it.
        assert_eq!(nav.reopen(None), Some(s("b")));
        assert_eq!(
            nav.tabs().iter().map(|tab| tab.session.clone()).collect::<Vec<_>>(),
            vec![s("a"), s("b"), s("d")]
        );
        assert_eq!(nav.active().unwrap().session, s("b"));
        assert_eq!(
            nav.recently_closed().iter().map(|entry| entry.session.clone()).collect::<Vec<_>>(),
            vec![s("c")]
        );
        assert_eq!(
            nav.recently_closed().iter().map(|entry| entry.session.clone()).collect::<Vec<_>>(),
            vec![s("c")],
            "closing c earlier stays eligible"
        );
        assert_eq!(nav.reopen(None), Some(s("c")));
        assert_eq!(
            nav.tabs().iter().map(|tab| tab.session.clone()).collect::<Vec<_>>(),
            vec![s("a"), s("b"), s("c"), s("d")],
            "c returns at its recorded slot"
        );
        assert!(nav.recently_closed().is_empty());
        assert_eq!(nav.reopen(None), None);
        assert_eq!(nav.reopen(Some(&s("missing"))), None);
    }

    #[test]
    fn tab_24_move_tab_reorders_and_tracks_active() {
        // session-tabs-model.ts:93-101: remove then splice, index clamped to
        // the last slot; the focused tab keeps its identity, not its index.
        let mut nav = NavigationState::new();
        nav.open(s("one"));
        nav.open(s("two"));
        nav.open(s("three"));
        nav.switch(&s("two")).unwrap();
        nav.active_mut().unwrap().draft = "kept".to_owned();
        let ids = |nav: &NavigationState| {
            nav.tabs().iter().map(|tab| tab.session.clone()).collect::<Vec<_>>()
        };
        assert!(nav.move_tab(&s("two"), 2));
        assert_eq!(ids(&nav), vec![s("one"), s("three"), s("two")]);
        assert_eq!(nav.active().unwrap().session, s("two"), "focus follows the moved tab");
        assert_eq!(nav.active().unwrap().draft, "kept", "per-tab state travels");
        // Moving an inactive tab across the focused one shifts focus with it.
        assert!(nav.move_tab(&s("one"), 2));
        assert_eq!(ids(&nav), vec![s("three"), s("two"), s("one")]);
        assert_eq!(nav.active().unwrap().session, s("two"));
        // Unknown sessions and same-slot moves are no-ops.
        assert!(!nav.move_tab(&s("missing"), 0));
        assert!(!nav.move_tab(&s("three"), 0));
        // Out-of-range targets clamp to the last slot.
        assert!(nav.move_tab(&s("three"), 99));
        assert_eq!(ids(&nav), vec![s("two"), s("one"), s("three")]);
        assert_eq!(nav.active().unwrap().session, s("two"));
    }

    #[test]
    fn tab_20_order_and_scope_survive_restart_but_closed_history_does_not() {
        // context/session-tabs.tsx:72-102 persists only { tabs } per scope.
        let mut nav = NavigationState::new();
        nav.open(s("one"));
        nav.open(s("two"));
        nav.open(s("three"));
        nav.apply(DaemonEvent::Renamed {
            session: s("two"),
            title: "Second".to_owned(),
            rev: 1,
        });
        nav.close(&s("three")).unwrap();
        nav.switch(&s("one")).unwrap();
        let persisted = nav.persist_tabs();
        assert_eq!(persisted.order, vec![s("one"), s("two")]);
        assert_eq!(persisted.titles, vec![(s("two"), "Second".to_owned())]);

        // A fresh process restores the same order; the active route, the
        // recently-closed stack and navigation history are not persisted.
        let mut restored = NavigationState::new();
        restored.restore_tabs(&persisted);
        assert_eq!(
            restored.tabs().iter().map(|tab| tab.session.clone()).collect::<Vec<_>>(),
            vec![s("one"), s("two")]
        );
        assert_eq!(restored.title(&s("two")), Some("Second"));
        assert_eq!(restored.active().unwrap().session, s("one"));
        assert!(restored.recently_closed().is_empty());
        assert!(restored.history_entries().is_empty());
        assert_eq!(restored.reopen(None), None);
    }

    #[test]
    fn tab_21_focus_confirmed_ack_retries_250ms_to_5s() {
        // context/session-tabs.tsx:53-55,229-267.
        let mut ack = ViewAcknowledgement::new();
        ack.observe(true, Some(100), Some(40));
        assert!(ack.unread());
        assert_eq!(ack.poll(0), Some(100), "focused unread view posts the watermark");
        assert_eq!(ack.poll(0), None, "the watermark is recorded before the request");
        assert_eq!(ack.failed(0), VIEW_RETRY_DELAY_MS);
        assert_eq!(ack.poll(249), None, "retry waits");
        assert_eq!(ack.poll(250), Some(100));
        assert_eq!(ack.failed(250), 500);
        assert_eq!(ack.poll(749), None);
        assert_eq!(ack.poll(750), Some(100));
        assert_eq!(ack.failed(750), 1_000);
        ack.failed(1_750);
        assert_eq!(ack.attempts(), 4);
        for expected in [4_000u64, 5_000, 5_000, 5_000] {
            assert_eq!(ack.failed(0), expected);
        }
        ack.succeeded();
        assert_eq!(ack.attempts(), 0);
        // Delivery only happens for the focused route with a newer idle.
        ack.observe(false, Some(200), Some(40));
        assert_eq!(ack.poll(10_000), None);
        ack.observe(true, Some(200), Some(200));
        assert!(!ack.unread());
        assert_eq!(ack.poll(10_000), None);
    }

    #[test]
    fn tab_22_async_snapshot_and_rename_cannot_resurrect_a_closed_tab() {
        // A late snapshot/rename after close may refresh the display cache but
        // must not re-admit a tab; only an explicit open does.
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.open(s("b"));
        nav.close(&s("b")).unwrap();
        nav.ingest_snapshot_title(s("b"), "Snapshot title".to_owned());
        nav.apply(DaemonEvent::Renamed {
            session: s("b"),
            title: "Late rename".to_owned(),
            rev: 2,
        });
        assert_eq!(nav.tabs().len(), 1);
        assert_eq!(nav.active().unwrap().session, s("a"));
        // No alternate fallback title replaced the closed entry's identity.
        assert!(nav.tabs().iter().all(|tab| tab.session != s("b")));
        assert_eq!(nav.title(&s("b")), Some("Late rename"));
        // Explicit user open clears the closed state.
        nav.open(s("b"));
        assert_eq!(nav.tabs().len(), 2);
        assert!(nav.recently_closed().iter().all(|entry| entry.session != s("b")));
    }

    #[test]
    fn tab_23_default_and_custom_shortcuts_dispatch_the_same_commands() {
        // config/keybind.ts:113-120,152-161 and app.tsx:854-902.
        assert_eq!(
            resolve_tab_shortcut(DEFAULT_TAB_KEYBINDS, "ctrl+tab"),
            Some(TabCommand::Next)
        );
        assert_eq!(
            resolve_tab_shortcut(DEFAULT_TAB_KEYBINDS, "alt+down"),
            Some(TabCommand::Next),
            "second default binding maps to the same command"
        );
        assert_eq!(
            resolve_tab_shortcut(DEFAULT_TAB_KEYBINDS, "ctrl+shift+tab"),
            Some(TabCommand::Previous)
        );
        assert_eq!(
            resolve_tab_shortcut(DEFAULT_TAB_KEYBINDS, "alt+shift+down"),
            Some(TabCommand::NextUnread)
        );
        assert_eq!(
            resolve_tab_shortcut(DEFAULT_TAB_KEYBINDS, "ctrl+shift+t"),
            Some(TabCommand::Reopen)
        );
        assert_eq!(
            resolve_tab_shortcut(DEFAULT_TAB_KEYBINDS, "ctrl+0"),
            Some(TabCommand::Select(9)),
            "slot 10 is zero-based index 9"
        );
        assert_eq!(resolve_tab_shortcut(DEFAULT_TAB_KEYBINDS, "ctrl+alt+z"), None);
        // A customized table still resolves the semantic command.
        let custom = [("x", "session.tab.next"), ("y", "session.tab.select.3")];
        assert_eq!(
            resolve_tab_shortcut(&custom, "x"),
            resolve_tab_shortcut(DEFAULT_TAB_KEYBINDS, "ctrl+tab")
        );
        assert_eq!(
            resolve_tab_shortcut(&custom, "y"),
            Some(TabCommand::Select(2))
        );
        for command in [
            TabCommand::Next,
            TabCommand::Previous,
            TabCommand::NextUnread,
            TabCommand::PreviousUnread,
            TabCommand::Close,
            TabCommand::Reopen,
            TabCommand::Select(0),
            TabCommand::Select(9),
        ] {
            assert_eq!(TabCommand::from_name(&command.name()), Some(command));
        }

        // Unread cycling skips slots that do not match and never mutates on a
        // full miss.
        let mut nav = NavigationState::new();
        nav.open(s("a"));
        nav.open(s("b"));
        nav.open(s("c"));
        nav.switch(&s("a")).unwrap();
        assert_eq!(nav.step_tab_where(true, |id| id == &s("b")), Some(1));
        assert_eq!(nav.active().unwrap().session, s("b"));
        // `cycleSessionTab` scans exactly `tabs.length` slots starting after
        // the active one, so a full cycle that only matches the current slot
        // wraps back to it (`context/session-tabs-model.ts:111-114`).
        assert_eq!(nav.step_tab_where(true, |id| id == &s("b")), Some(1));
        assert_eq!(nav.active().unwrap().session, s("b"));
        // A predicate that matches no tab leaves the active tab untouched.
        assert_eq!(nav.step_tab_where(true, |id| id == &s("missing")), None);
        assert_eq!(nav.active().unwrap().session, s("b"));
        assert_eq!(nav.step_tab_where(false, |_| true), Some(0));
    }
}
