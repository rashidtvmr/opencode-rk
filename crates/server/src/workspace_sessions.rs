//! APP-006: workspace session workflow types for the app server.
//!
//! Caller-owned workspace/session lifecycle: create/rename/archive/resume,
//! first-session-on-empty-home marker, bounded history pagination, and an
//! inactive-store release receipt. Pure `std` logic: no I/O, no clock, no
//! threads, no globals beyond a process-local ID counter, no logging. The
//! caller owns persistence (via [`HomeSnapshot`]) and any sockets/watchers.
//!
//! Scope rule: every op takes the caller's `scope` (workspace IDs it holds).
//! A workspace outside `scope` fails with [`WorkspaceError::ScopeRejected`];
//! a session ID looked up under the wrong workspace fails with
//! [`WorkspaceError::SessionNotFound`] (same string as a missing session, so
//! no existence oracle). Errors never carry ID bytes or titles.
//!
//! Bounds: at most [`MAX_WORKSPACES`] stores per home, [`MAX_SESSIONS_PER_WORKSPACE`]
//! sessions per store, [`MAX_HISTORY_ENTRIES`] messages per session.
//! History pages clamp to `[PAGE_MIN, PAGE_MAX]`; a zero limit returns one
//! row, never a dump.

#![forbid(unsafe_code)]

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Byte bound for session titles.
pub const MAX_TITLE_BYTES: usize = 512;
/// Byte bound for one history message.
pub const MAX_MESSAGE_BYTES: usize = 32_768;
/// Cap on workspace stores tracked per home (bounded retention).
pub const MAX_WORKSPACES: usize = 64;
/// Cap on sessions tracked per workspace store.
pub const MAX_SESSIONS_PER_WORKSPACE: usize = 1024;
/// Cap on messages retained per session.
pub const MAX_HISTORY_ENTRIES: usize = 10_000;
/// Floor for paged queries.
pub const PAGE_MIN: usize = 1;
/// Default page size for history queries.
pub const PAGE_DEFAULT: usize = 50;
/// Ceiling for paged queries.
pub const PAGE_MAX: usize = 100;

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn fresh_bytes() -> [u8; 16] {
    let n = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&n.to_le_bytes());
    b[8..].copy_from_slice(&n.wrapping_mul(0x9E37_79B9_7F4A_7C15).to_le_bytes());
    b
}

fn hex_of(bytes: &[u8; 16], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for b in bytes {
        write!(f, "{b:02x}")?;
    }
    Ok(())
}

/// Opaque workspace scope identifier. Bytes never appear in errors.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct WorkspaceId([u8; 16]);

impl WorkspaceId {
    /// Fresh locally-unique ID (monotonic counter; no RNG/clock needed).
    #[must_use]
    pub fn new() -> Self {
        Self(fresh_bytes())
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WorkspaceId(")?;
        hex_of(&self.0, f)?;
        f.write_str(")")
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex_of(&self.0, f)
    }
}

/// Opaque session identifier, always scoped to one workspace.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SessionId([u8; 16]);

impl SessionId {
    #[must_use]
    pub fn new() -> Self {
        Self(fresh_bytes())
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionId(")?;
        hex_of(&self.0, f)?;
        f.write_str(")")
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex_of(&self.0, f)
    }
}

/// Typed failures. No variant carries IDs, titles, or message text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WorkspaceError {
    /// Workspace outside the caller's scope.
    ScopeRejected,
    /// Workspace in scope but not present in this home.
    WorkspaceNotFound,
    /// No such session in this workspace (same string for foreign IDs).
    SessionNotFound,
    /// Op not allowed on an archived session.
    SessionArchived,
    /// Title empty.
    TitleEmpty,
    /// Title over [`MAX_TITLE_BYTES`].
    TitleTooLong { max: usize, actual: usize },
    /// Message text empty.
    MessageEmpty,
    /// Message over [`MAX_MESSAGE_BYTES`].
    MessageTooLarge { max: usize, actual: usize },
    /// Home already tracks [`MAX_WORKSPACES`] stores.
    WorkspaceLimit,
    /// Store already tracks [`MAX_SESSIONS_PER_WORKSPACE`] sessions.
    SessionLimit,
    /// Session already retains [`MAX_HISTORY_ENTRIES`] messages.
    HistoryFull,
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeRejected => f.write_str("workspace scope rejected"),
            Self::WorkspaceNotFound => f.write_str("workspace not found"),
            Self::SessionNotFound => f.write_str("session not found"),
            Self::SessionArchived => f.write_str("session is archived"),
            Self::TitleEmpty => f.write_str("session title must not be empty"),
            Self::TitleTooLong { max, actual } => {
                write!(f, "session title too long: max {max} bytes, got {actual}")
            }
            Self::MessageEmpty => f.write_str("message must not be empty"),
            Self::MessageTooLarge { max, actual } => {
                write!(f, "message too large: max {max} bytes, got {actual}")
            }
            Self::WorkspaceLimit => f.write_str("workspace limit reached"),
            Self::SessionLimit => f.write_str("session limit reached"),
            Self::HistoryFull => f.write_str("session history full"),
        }
    }
}

impl std::error::Error for WorkspaceError {}

fn check_title(title: &str) -> Result<(), WorkspaceError> {
    if title.is_empty() {
        return Err(WorkspaceError::TitleEmpty);
    }
    let len = title.len();
    if len > MAX_TITLE_BYTES {
        return Err(WorkspaceError::TitleTooLong {
            max: MAX_TITLE_BYTES,
            actual: len,
        });
    }
    Ok(())
}

fn check_message(text: &str) -> Result<(), WorkspaceError> {
    if text.is_empty() {
        return Err(WorkspaceError::MessageEmpty);
    }
    let len = text.len();
    if len > MAX_MESSAGE_BYTES {
        return Err(WorkspaceError::MessageTooLarge {
            max: MAX_MESSAGE_BYTES,
            actual: len,
        });
    }
    Ok(())
}

/// Clamp a requested page limit into `[PAGE_MIN, PAGE_MAX]`.
#[must_use]
pub fn clamp_limit(limit: usize) -> usize {
    limit.clamp(PAGE_MIN, PAGE_MAX)
}

/// One retained history message. `seq` is store-monotonic (acts as the
/// stable timestamp across close/reopen); never zero, never reused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub seq: u64,
    pub text: String,
}

/// One session inside a workspace store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub id: SessionId,
    pub title: String,
    pub archived: bool,
    pub created_seq: u64,
    pub updated_seq: u64,
    pub history: Vec<Message>,
}

/// One page of session history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryPage {
    pub entries: Vec<Message>,
    pub total: usize,
    /// Offset for the next page, or `None` when this page is the tail.
    pub next_offset: Option<usize>,
}

/// Result of [`Home::ensure_first_session`]: the usable session plus the
/// first-session-on-empty-home marker (`true` only when the home held zero
/// sessions before this call).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirstSession {
    pub session: Session,
    pub is_first: bool,
}

/// Receipt for [`Home::release_inactive`]: exact counts of dropped state so
/// the caller can assert watchers/stores were released within bounds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReleaseReceipt {
    pub released_workspaces: usize,
    pub released_sessions: usize,
    pub released_messages: usize,
}

impl ReleaseReceipt {
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.released_workspaces == 0
            && self.released_sessions == 0
            && self.released_messages == 0
    }
}

/// One workspace's session store. Caller-owned; no I/O.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceStore {
    id: WorkspaceId,
    sessions: Vec<Session>,
    next_seq: u64,
}

impl WorkspaceStore {
    fn fresh_seq(&mut self) -> u64 {
        let seq = self.next_seq.max(1);
        self.next_seq = seq.saturating_add(1).max(1);
        seq
    }

    fn find_mut(&mut self, session: &SessionId) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| &s.id == session)
    }

    fn find(&self, session: &SessionId) -> Option<&Session> {
        self.sessions.iter().find(|s| &s.id == session)
    }
}

/// Caller-owned home: the set of workspace session stores for the app.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Home {
    stores: Vec<WorkspaceStore>,
}

/// Owned persistence image of a [`Home`]. Clone it across close/reopen;
/// the caller owns the bytes/channel it travels on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomeSnapshot(Home);

fn check_scope(scope: &[WorkspaceId], workspace: &WorkspaceId) -> Result<(), WorkspaceError> {
    if scope.iter().any(|held| held == workspace) {
        Ok(())
    } else {
        Err(WorkspaceError::ScopeRejected)
    }
}

impl Home {
    /// Empty home; allocates nothing.
    #[must_use]
    pub fn new() -> Self {
        Self { stores: Vec::new() }
    }

    /// True when no workspace store exists.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stores.is_empty()
    }

    /// Number of workspace stores.
    #[must_use]
    pub fn workspace_count(&self) -> usize {
        self.stores.len()
    }

    /// Total sessions across all stores.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.stores.iter().map(|s| s.sessions.len()).sum()
    }

    fn store_mut<'a>(
        stores: &'a mut [WorkspaceStore],
        workspace: &WorkspaceId,
    ) -> Option<&'a mut WorkspaceStore> {
        stores.iter_mut().find(|s| s.id == *workspace)
    }

    fn store<'a>(
        stores: &'a [WorkspaceStore],
        workspace: &WorkspaceId,
    ) -> Option<&'a WorkspaceStore> {
        stores.iter().find(|s| s.id == *workspace)
    }

    /// Get-or-create the store for `workspace`. Creates the store when the
    /// caller holds it in `scope` but the home has no store yet.
    fn store_or_create(
        &mut self,
        scope: &[WorkspaceId],
        workspace: &WorkspaceId,
    ) -> Result<&mut WorkspaceStore, WorkspaceError> {
        check_scope(scope, workspace)?;
        if Self::store(&self.stores, workspace).is_none() {
            if self.stores.len() >= MAX_WORKSPACES {
                return Err(WorkspaceError::WorkspaceLimit);
            }
            self.stores.push(WorkspaceStore {
                id: *workspace,
                sessions: Vec::new(),
                next_seq: 1,
            });
        }
        Ok(Self::store_mut(&mut self.stores, workspace)
            .expect("store present after get-or-create"))
    }

    /// First-session workflow: on an empty home (zero sessions anywhere),
    /// create the store plus the first usable session and mark
    /// [`FirstSession::is_first`]. Otherwise return the workspace's earliest
    /// session unchanged with `is_first == false`, creating one only when
    /// this workspace has none.
    pub fn ensure_first_session(
        &mut self,
        scope: &[WorkspaceId],
        workspace: &WorkspaceId,
        title: &str,
    ) -> Result<FirstSession, WorkspaceError> {
        check_title(title)?;
        let home_was_empty = self.session_count() == 0;
        let store = self.store_or_create(scope, workspace)?;
        if let Some(earliest) = store.sessions.first().cloned() {
            return Ok(FirstSession {
                session: earliest,
                is_first: false,
            });
        }
        if store.sessions.len() >= MAX_SESSIONS_PER_WORKSPACE {
            return Err(WorkspaceError::SessionLimit);
        }
        let seq = store.fresh_seq();
        let session = Session {
            id: SessionId::new(),
            title: title.to_owned(),
            archived: false,
            created_seq: seq,
            updated_seq: seq,
            history: Vec::new(),
        };
        store.sessions.push(session.clone());
        Ok(FirstSession {
            session,
            is_first: home_was_empty,
        })
    }

    /// Create a new active session in `workspace`.
    pub fn create(
        &mut self,
        scope: &[WorkspaceId],
        workspace: &WorkspaceId,
        title: &str,
    ) -> Result<Session, WorkspaceError> {
        check_title(title)?;
        let store = self.store_or_create(scope, workspace)?;
        if store.sessions.len() >= MAX_SESSIONS_PER_WORKSPACE {
            return Err(WorkspaceError::SessionLimit);
        }
        let seq = store.fresh_seq();
        let session = Session {
            id: SessionId::new(),
            title: title.to_owned(),
            archived: false,
            created_seq: seq,
            updated_seq: seq,
            history: Vec::new(),
        };
        store.sessions.push(session.clone());
        Ok(session)
    }

    /// Rename a session; bumps `updated_seq` and returns the fresh copy.
    pub fn rename(
        &mut self,
        scope: &[WorkspaceId],
        workspace: &WorkspaceId,
        session: &SessionId,
        title: &str,
    ) -> Result<Session, WorkspaceError> {
        check_scope(scope, workspace)?;
        check_title(title)?;
        let store = Self::store_mut(&mut self.stores, workspace)
            .ok_or(WorkspaceError::WorkspaceNotFound)?;
        let seq = store.fresh_seq();
        let entry = store
            .find_mut(session)
            .ok_or(WorkspaceError::SessionNotFound)?;
        entry.title = title.to_owned();
        entry.updated_seq = seq;
        Ok(entry.clone())
    }

    /// Archive a session. Idempotent: re-archiving returns the same copy.
    pub fn archive(
        &mut self,
        scope: &[WorkspaceId],
        workspace: &WorkspaceId,
        session: &SessionId,
    ) -> Result<Session, WorkspaceError> {
        check_scope(scope, workspace)?;
        let store = Self::store_mut(&mut self.stores, workspace)
            .ok_or(WorkspaceError::WorkspaceNotFound)?;
        let seq = store.fresh_seq();
        let entry = store
            .find_mut(session)
            .ok_or(WorkspaceError::SessionNotFound)?;
        entry.archived = true;
        entry.updated_seq = seq;
        Ok(entry.clone())
    }

    /// Resume an archived session. Resuming an active session is a no-op
    /// returning the current copy.
    pub fn resume(
        &mut self,
        scope: &[WorkspaceId],
        workspace: &WorkspaceId,
        session: &SessionId,
    ) -> Result<Session, WorkspaceError> {
        check_scope(scope, workspace)?;
        let store = Self::store_mut(&mut self.stores, workspace)
            .ok_or(WorkspaceError::WorkspaceNotFound)?;
        let seq = store.fresh_seq();
        let entry = store
            .find_mut(session)
            .ok_or(WorkspaceError::SessionNotFound)?;
        entry.archived = false;
        entry.updated_seq = seq;
        Ok(entry.clone())
    }

    /// Fetch one session copy.
    pub fn get(
        &self,
        scope: &[WorkspaceId],
        workspace: &WorkspaceId,
        session: &SessionId,
    ) -> Result<Session, WorkspaceError> {
        check_scope(scope, workspace)?;
        Self::store(&self.stores, workspace)
            .ok_or(WorkspaceError::WorkspaceNotFound)?
            .find(session)
            .cloned()
            .ok_or(WorkspaceError::SessionNotFound)
    }

    /// Append one history message. Archived sessions reject appends.
    pub fn append(
        &mut self,
        scope: &[WorkspaceId],
        workspace: &WorkspaceId,
        session: &SessionId,
        text: &str,
    ) -> Result<Message, WorkspaceError> {
        check_scope(scope, workspace)?;
        check_message(text)?;
        let store = Self::store_mut(&mut self.stores, workspace)
            .ok_or(WorkspaceError::WorkspaceNotFound)?;
        let idx = store
            .sessions
            .iter()
            .position(|s| s.id == *session)
            .ok_or(WorkspaceError::SessionNotFound)?;
        if store.sessions[idx].archived {
            return Err(WorkspaceError::SessionArchived);
        }
        if store.sessions[idx].history.len() >= MAX_HISTORY_ENTRIES {
            return Err(WorkspaceError::HistoryFull);
        }
        let seq = store.fresh_seq();
        let message = Message {
            seq,
            text: text.to_owned(),
        };
        let entry = &mut store.sessions[idx];
        entry.history.push(message.clone());
        entry.updated_seq = seq;
        Ok(message)
    }

    /// Bounded history page starting at `offset`, taking
    /// `clamp_limit(limit)` rows in `seq` order.
    pub fn history_page(
        &self,
        scope: &[WorkspaceId],
        workspace: &WorkspaceId,
        session: &SessionId,
        offset: usize,
        limit: usize,
    ) -> Result<HistoryPage, WorkspaceError> {
        check_scope(scope, workspace)?;
        let entry = Self::store(&self.stores, workspace)
            .ok_or(WorkspaceError::WorkspaceNotFound)?
            .find(session)
            .ok_or(WorkspaceError::SessionNotFound)?;
        let total = entry.history.len();
        let take = clamp_limit(limit);
        let entries: Vec<Message> =
            entry.history.iter().skip(offset).take(take).cloned().collect();
        let consumed = offset.saturating_add(entries.len());
        Ok(HistoryPage {
            entries,
            total,
            next_offset: if consumed < total {
                Some(consumed)
            } else {
                None
            },
        })
    }

    /// Capture the full home state. Reopen it later with [`Home::restore`].
    #[must_use]
    pub fn snapshot(&self) -> HomeSnapshot {
        HomeSnapshot(self.clone())
    }

    /// Reopen a previously captured snapshot (close/reopen round-trip).
    #[must_use]
    pub fn restore(snapshot: HomeSnapshot) -> Self {
        snapshot.0
    }

    /// Drop every store whose workspace is not in `active`. Returns an exact
    /// [`ReleaseReceipt`] so the caller can assert inactive stores and their
    /// watcher state were released within measured bounds.
    #[must_use]
    pub fn release_inactive(&mut self, active: &[WorkspaceId]) -> ReleaseReceipt {
        let mut receipt = ReleaseReceipt::default();
        let mut kept = Vec::with_capacity(self.stores.len());
        for store in std::mem::take(&mut self.stores) {
            if active.iter().any(|held| held == &store.id) {
                kept.push(store);
            } else {
                receipt.released_workspaces += 1;
                receipt.released_sessions += store.sessions.len();
                receipt.released_messages +=
                    store.sessions.iter().map(|s| s.history.len()).sum::<usize>();
            }
        }
        self.stores = kept;
        receipt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope_of(ids: &[WorkspaceId]) -> Vec<WorkspaceId> {
        ids.to_vec()
    }

    #[test]
    fn empty_home_creates_first_session() {
        let mut home = Home::new();
        assert!(home.is_empty());
        let workspace = WorkspaceId::new();
        let scope = scope_of(&[workspace]);
        let first = home
            .ensure_first_session(&scope, &workspace, "first")
            .unwrap();
        assert!(first.is_first, "empty home must mark the first session");
        assert_eq!(first.session.title, "first");
        assert!(!first.session.archived);
        // Session is immediately usable: append works.
        home.append(&scope, &workspace, &first.session.id, "hello")
            .unwrap();
        // Second call returns the same earliest session, marker cleared.
        let again = home
            .ensure_first_session(&scope, &workspace, "first")
            .unwrap();
        assert!(!again.is_first);
        assert_eq!(again.session.id, first.session.id);
        assert_eq!(home.session_count(), 1);
    }

    #[test]
    fn close_reopen_restores_history_and_timestamps() {
        let mut home = Home::new();
        let workspace = WorkspaceId::new();
        let scope = scope_of(&[workspace]);
        let created = home.create(&scope, &workspace, "work").unwrap();
        home.append(&scope, &workspace, &created.id, "one").unwrap();
        home.append(&scope, &workspace, &created.id, "two").unwrap();
        let renamed = home
            .rename(&scope, &workspace, &created.id, "work v2")
            .unwrap();
        // Close: capture snapshot. Reopen: restore into a fresh home.
        let snapshot = home.snapshot();
        drop(home);
        let mut reopened = Home::restore(snapshot);
        let got = reopened.get(&scope, &workspace, &created.id).unwrap();
        assert_eq!(got.title, "work v2");
        assert_eq!(got.created_seq, created.created_seq);
        assert_eq!(got.updated_seq, renamed.updated_seq);
        assert_eq!(got.history.len(), 2);
        assert_eq!(got.history[0].text, "one");
        assert_eq!(got.history[1].text, "two");
        assert!(got.history[0].seq < got.history[1].seq);
        // Reopened home keeps working with continued seqs.
        let third = reopened
            .append(&scope, &workspace, &created.id, "three")
            .unwrap();
        assert!(third.seq > got.history[1].seq);
        let page = reopened
            .history_page(&scope, &workspace, &created.id, 0, PAGE_MAX)
            .unwrap();
        assert_eq!(page.total, 3);
    }

    #[test]
    fn cross_scope_id_rejected_without_leakage() {
        let mut home = Home::new();
        let workspace_a = WorkspaceId::new();
        let workspace_b = WorkspaceId::new();
        let foreign = WorkspaceId::from_bytes([0xAB; 16]);
        let scope = scope_of(&[workspace_a, workspace_b]);
        let session_a = home.create(&scope, &workspace_a, "secret-title").unwrap();
        // Seed workspace B so the cross-scope probe reaches session lookup,
        // not the missing-store path.
        let _seed_b = home.create(&scope, &workspace_b, "other").unwrap();
        // Workspace outside the caller's scope: opaque rejection.
        let err = home
            .rename(&scope, &foreign, &session_a.id, "x")
            .unwrap_err();
        assert_eq!(err, WorkspaceError::ScopeRejected);
        // Session from workspace A used under workspace B (both held):
        // rejected exactly like a missing session, no oracle.
        let err = home
            .rename(&scope, &workspace_b, &session_a.id, "x")
            .unwrap_err();
        assert_eq!(err, WorkspaceError::SessionNotFound);
        let err = home
            .history_page(&scope, &workspace_b, &session_a.id, 0, 10)
            .unwrap_err();
        assert_eq!(err, WorkspaceError::SessionNotFound);
        // Neither error echoes IDs or titles.
        for probe in [WorkspaceError::ScopeRejected, WorkspaceError::SessionNotFound] {
            let rendered = format!("{probe} {probe:?}");
            assert!(
                !rendered.contains("secret-title"),
                "error must not echo titles: {rendered}"
            );
            assert!(
                !rendered.contains("ab"),
                "error must not echo foreign ID bytes: {rendered}"
            );
        }
        // Home unchanged by the rejected ops.
        assert_eq!(home.get(&scope, &workspace_a, &session_a.id).unwrap().title, "secret-title");
    }

    #[test]
    fn large_history_paginated_with_caps() {
        let mut home = Home::new();
        let workspace = WorkspaceId::new();
        let scope = scope_of(&[workspace]);
        let session = home.create(&scope, &workspace, "log").unwrap();
        let total_messages = PAGE_MAX * 2 + 50;
        for i in 0..total_messages {
            home.append(&scope, &workspace, &session.id, &format!("m{i}"))
                .unwrap();
        }
        assert_eq!(clamp_limit(0), PAGE_MIN);
        assert_eq!(clamp_limit(usize::MAX), PAGE_MAX);
        // Zero limit returns one row, never a dump.
        let floor = home
            .history_page(&scope, &workspace, &session.id, 0, 0)
            .unwrap();
        assert_eq!(floor.entries.len(), 1);
        assert_eq!(floor.total, total_messages);
        // Oversize limit is capped.
        let capped = home
            .history_page(&scope, &workspace, &session.id, 0, usize::MAX)
            .unwrap();
        assert_eq!(capped.entries.len(), PAGE_MAX);
        assert_eq!(capped.next_offset, Some(PAGE_MAX));
        // Walk every page: full coverage, seq order, no gaps or dups.
        let mut offset = 0;
        let mut seen = Vec::new();
        loop {
            let page = home
                .history_page(&scope, &workspace, &session.id, offset, PAGE_DEFAULT)
                .unwrap();
            assert_eq!(page.total, total_messages);
            seen.extend(page.entries.iter().map(|m| m.text.clone()));
            match page.next_offset {
                Some(next) => {
                    assert!(next > offset, "pages must advance");
                    offset = next;
                }
                None => break,
            }
        }
        assert_eq!(seen.len(), total_messages);
        assert_eq!(seen[0], "m0");
        assert_eq!(seen[total_messages - 1], format!("m{}", total_messages - 1));
        // Offset past the end: empty tail, no next page.
        let tail = home
            .history_page(&scope, &workspace, &session.id, total_messages + 10, 50)
            .unwrap();
        assert!(tail.entries.is_empty());
        assert_eq!(tail.next_offset, None);
    }

    #[test]
    fn rename_archive_resume_lifecycle() {
        let mut home = Home::new();
        let workspace = WorkspaceId::new();
        let scope = scope_of(&[workspace]);
        let session = home.create(&scope, &workspace, "draft").unwrap();
        let renamed = home
            .rename(&scope, &workspace, &session.id, "draft v2")
            .unwrap();
        assert_eq!(renamed.title, "draft v2");
        assert!(renamed.updated_seq >= renamed.created_seq);
        let archived = home.archive(&scope, &workspace, &session.id).unwrap();
        assert!(archived.archived);
        // Re-archive is idempotent.
        let again = home.archive(&scope, &workspace, &session.id).unwrap();
        assert!(again.archived);
        // Archived sessions reject appends.
        assert_eq!(
            home.append(&scope, &workspace, &session.id, "late")
                .unwrap_err(),
            WorkspaceError::SessionArchived
        );
        let resumed = home.resume(&scope, &workspace, &session.id).unwrap();
        assert!(!resumed.archived);
        home.append(&scope, &workspace, &session.id, "back")
            .unwrap();
        // Title bounds hold on every path.
        assert_eq!(
            home.create(&scope, &workspace, "").unwrap_err(),
            WorkspaceError::TitleEmpty
        );
        let long = "x".repeat(MAX_TITLE_BYTES + 1);
        assert!(matches!(
            home.rename(&scope, &workspace, &session.id, &long).unwrap_err(),
            WorkspaceError::TitleTooLong { .. }
        ));
        assert_eq!(
            home.get(&scope, &workspace, &session.id).unwrap().title,
            "draft v2",
            "failed rename must not mutate the title"
        );
    }

    #[test]
    fn inactive_store_release_receipt() {
        let mut home = Home::new();
        let workspace_a = WorkspaceId::new();
        let workspace_b = WorkspaceId::new();
        let scope = scope_of(&[workspace_a, workspace_b]);
        let keep = home.create(&scope, &workspace_a, "keep").unwrap();
        home.append(&scope, &workspace_a, &keep.id, "k1").unwrap();
        home.append(&scope, &workspace_a, &keep.id, "k2").unwrap();
        let drop_me = home.create(&scope, &workspace_b, "drop").unwrap();
        home.append(&scope, &workspace_b, &drop_me.id, "d1").unwrap();
        home.create(&scope, &workspace_b, "drop2").unwrap();
        // Release everything except workspace A.
        let receipt = home.release_inactive(&[workspace_a]);
        assert_eq!(
            receipt,
            ReleaseReceipt {
                released_workspaces: 1,
                released_sessions: 2,
                released_messages: 1,
            }
        );
        assert!(!receipt.is_empty());
        assert_eq!(home.workspace_count(), 1);
        assert!(home.get(&scope, &workspace_a, &keep.id).is_ok());
        assert_eq!(
            home.get(&scope, &workspace_b, &drop_me.id).unwrap_err(),
            WorkspaceError::WorkspaceNotFound
        );
        // Nothing inactive left: empty receipt.
        let quiet = home.release_inactive(&[workspace_a]);
        assert!(quiet.is_empty());
        assert_eq!(quiet.released_workspaces, 0);
    }
}
