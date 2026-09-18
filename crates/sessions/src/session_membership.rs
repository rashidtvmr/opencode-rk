//! Workspace session membership: scope checks, rename/archive events, paging caps.
//!
//! Scope: APP-006 membership slice. Pure `std` model with no I/O so concurrent
//! clients (terminal/web) can share one convergence rule: versioned events win
//! by highest version, stale or foreign-scope events change nothing.
//!
//! Security: [`authorize`] rejects IDs outside the caller's membership without
//! echoing the rejected ID (no existence oracle beyond allow/deny).

#![forbid(unsafe_code)]

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Bounds
// ---------------------------------------------------------------------------

/// Byte bound for workspace titles (mirrors `MAX_TITLE_BYTES` in contracts).
pub const MAX_TITLE_BYTES: usize = 512;
/// Floor for paged queries; a zero limit still returns one row, never a dump.
pub const PAGE_MIN: usize = 1;
/// Default page size for session/history queries.
pub const PAGE_DEFAULT: usize = 100;
/// Ceiling for paged queries (mirrors `PAGE_MAX` in sessions manager).
pub const PAGE_MAX: usize = 500;
/// Cap on memberships tracked per workspace (bounded retention).
pub const MAX_MEMBERS_PER_WORKSPACE: usize = 1024;

/// Clamp a requested page limit into `[PAGE_MIN, PAGE_MAX]`.
#[must_use]
pub fn clamp_page(limit: usize) -> usize {
    limit.clamp(PAGE_MIN, PAGE_MAX)
}

/// Bounded slice of `items` starting at `offset`, taking `clamp_page(limit)`.
#[must_use]
pub fn paginate<T: Clone>(items: &[T], limit: usize, offset: usize) -> Vec<T> {
    items
        .iter()
        .skip(offset)
        .take(clamp_page(limit))
        .cloned()
        .collect()
}

/// True while `count` fits the per-workspace membership cap.
#[must_use]
pub fn within_member_cap(count: usize) -> bool {
    count <= MAX_MEMBERS_PER_WORKSPACE
}

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Opaque workspace scope identifier. Bytes never appear in rejection errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct WorkspaceId([u8; 16]);

impl WorkspaceId {
    /// Fresh locally-unique ID (counter x time-nanos; no RNG needed).
    #[must_use]
    pub fn new() -> Self {
        let n = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(n as u128);
        let mut b = [0u8; 16];
        b[..8].copy_from_slice(&(t as u64 ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15)).to_le_bytes());
        b[8..].copy_from_slice(&n.to_le_bytes());
        Self(b)
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

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Membership and scope check
// ---------------------------------------------------------------------------

/// Role inside one workspace scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Role {
    Owner,
    Member,
}

impl Role {
    #[must_use]
    pub const fn can_rename(self) -> bool {
        true // any member may propose a rename; convergence by version
    }

    #[must_use]
    pub const fn can_archive(self) -> bool {
        matches!(self, Self::Owner)
    }
}

/// One workspace grant held by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Membership {
    pub workspace: WorkspaceId,
    pub role: Role,
}

impl Membership {
    #[must_use]
    pub const fn new(workspace: WorkspaceId, role: Role) -> Self {
        Self { workspace, role }
    }
}

/// Opaque scope denial. Carries no ID bytes: safe to surface to the caller.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct ScopeReject;

impl fmt::Display for ScopeReject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("workspace scope rejected")
    }
}

impl std::error::Error for ScopeReject {}

/// Resolve the caller's role for `workspace`, or [`ScopeReject`] when the ID
/// is outside every held membership. No ID bytes leak into the error.
pub fn authorize(memberships: &[Membership], workspace: &WorkspaceId) -> Result<Role, ScopeReject> {
    memberships
        .iter()
        .find(|m| &m.workspace == workspace)
        .map(|m| m.role)
        .ok_or(ScopeReject)
}

// ---------------------------------------------------------------------------
// Rename/archive events with last-writer-wins convergence
// ---------------------------------------------------------------------------

/// Title validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TitleError {
    Empty,
    TooLong { max: usize, actual: usize },
}

impl fmt::Display for TitleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("workspace title must not be empty"),
            Self::TooLong { max, actual } => {
                write!(f, "workspace title too long: max {max} bytes, got {actual}")
            }
        }
    }
}

impl std::error::Error for TitleError {}

fn check_title(title: &str) -> Result<(), TitleError> {
    if title.is_empty() {
        return Err(TitleError::Empty);
    }
    let len = title.len();
    if len > MAX_TITLE_BYTES {
        return Err(TitleError::TooLong {
            max: MAX_TITLE_BYTES,
            actual: len,
        });
    }
    Ok(())
}

/// Mutation failure: scope denial, missing permission, or bad title.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MembershipError {
    Rejected(ScopeReject),
    Forbidden(&'static str),
    Title(TitleError),
}

impl fmt::Display for MembershipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(e) => write!(f, "{e}"),
            Self::Forbidden(what) => write!(f, "forbidden: {what}"),
            Self::Title(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for MembershipError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Rejected(e) => Some(e),
            Self::Title(e) => Some(e),
            Self::Forbidden(_) => None,
        }
    }
}

impl From<ScopeReject> for MembershipError {
    fn from(e: ScopeReject) -> Self {
        Self::Rejected(e)
    }
}

impl From<TitleError> for MembershipError {
    fn from(e: TitleError) -> Self {
        Self::Title(e)
    }
}

/// Versioned mutation broadcast to every client holding the workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MembershipEvent {
    Renamed {
        workspace: WorkspaceId,
        title: String,
        version: u64,
    },
    Archived {
        workspace: WorkspaceId,
        version: u64,
    },
}

impl MembershipEvent {
    #[must_use]
    pub const fn workspace(&self) -> &WorkspaceId {
        match self {
            Self::Renamed { workspace, .. } | Self::Archived { workspace, .. } => workspace,
        }
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        match self {
            Self::Renamed { version, .. } | Self::Archived { version, .. } => *version,
        }
    }
}

/// Local replica of one workspace's visible session-membership state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceView {
    id: WorkspaceId,
    title: String,
    archived: bool,
    version: u64,
}

impl WorkspaceView {
    /// Fresh unarchived view at version 0.
    pub fn create(id: WorkspaceId, title: &str) -> Result<Self, TitleError> {
        check_title(title)?;
        Ok(Self {
            id,
            title: title.to_owned(),
            archived: false,
            version: 0,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &WorkspaceId {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub const fn archived(&self) -> bool {
        self.archived
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Rename as `role`; bumps the version and returns the broadcast event.
    pub fn rename(&mut self, role: Role, title: &str) -> Result<MembershipEvent, MembershipError> {
        if !role.can_rename() {
            return Err(MembershipError::Forbidden("rename requires membership"));
        }
        check_title(title)?;
        self.version = self.version.saturating_add(1);
        self.title = title.to_owned();
        Ok(MembershipEvent::Renamed {
            workspace: self.id,
            title: self.title.clone(),
            version: self.version,
        })
    }

    /// Archive as `role`. Owner-only; idempotent (repeat emits same version).
    pub fn archive(&mut self, role: Role) -> Result<MembershipEvent, MembershipError> {
        if !role.can_archive() {
            return Err(MembershipError::Forbidden("archive requires owner role"));
        }
        if !self.archived {
            self.version = self.version.saturating_add(1);
            self.archived = true;
        }
        Ok(MembershipEvent::Archived {
            workspace: self.id,
            version: self.version,
        })
    }

    /// Merge a remote event. Returns true when state changed; stale
    /// (older-or-equal version) and foreign-workspace events change nothing.
    pub fn apply(&mut self, event: &MembershipEvent) -> bool {
        if event.workspace() != &self.id || event.version() <= self.version {
            return false;
        }
        match event {
            MembershipEvent::Renamed { title, version, .. } => {
                self.title = title.clone();
                self.version = *version;
            }
            MembershipEvent::Archived { version, .. } => {
                self.archived = true;
                self.version = *version;
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Tests (frozen RED taxonomy: cross-scope reject, rename converge, paging)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_with_title(title: &str) -> (WorkspaceId, WorkspaceView) {
        let id = WorkspaceId::new();
        let view = WorkspaceView::create(id, title).unwrap();
        (id, view)
    }

    #[test]
    fn cross_scope_reject_leaks_nothing() {
        let (mine, _) = workspace_with_title("mine");
        let foreign = WorkspaceId::from_bytes([0xAB; 16]);
        assert_ne!(mine, foreign);
        let memberships = [Membership::new(mine, Role::Owner)];
        let err = authorize(&memberships, &foreign).unwrap_err();
        assert_eq!(err, ScopeReject);
        let rendered = format!("{err} {err:?}");
        assert!(
            !rendered.contains("ab"),
            "rejection must not echo foreign ID bytes: {rendered}"
        );
        let as_membership: MembershipError = err.into();
        let rendered = format!("{as_membership} {as_membership:?}");
        assert!(
            !rendered.contains("ab"),
            "wrapped rejection must not echo foreign ID bytes: {rendered}"
        );
    }

    #[test]
    fn authorize_returns_held_role() {
        let (owned, _) = workspace_with_title("owned");
        let (joined, _) = workspace_with_title("joined");
        let memberships = [
            Membership::new(owned, Role::Owner),
            Membership::new(joined, Role::Member),
        ];
        assert_eq!(authorize(&memberships, &owned), Ok(Role::Owner));
        assert_eq!(authorize(&memberships, &joined), Ok(Role::Member));
        assert_eq!(authorize(&[], &owned), Err(ScopeReject));
    }

    #[test]
    fn rename_converges_across_clients() {
        let (id, seed) = workspace_with_title("seed");
        let mut client_a = seed.clone();
        let mut client_b = seed;
        // Two renames issued in version order; delivered in opposite orders.
        let first = client_a.rename(Role::Owner, "one").unwrap();
        client_b.apply(&first);
        let second = client_b.rename(Role::Member, "two").unwrap();
        // A gets them in order, B already has both; then replay reversed.
        assert!(client_a.apply(&second));
        let mut replay_a = WorkspaceView::create(id, "seed").unwrap();
        let mut replay_b = WorkspaceView::create(id, "seed").unwrap();
        assert!(replay_a.apply(&second));
        assert!(!replay_a.apply(&first)); // stale: ignored
        assert!(replay_b.apply(&first));
        assert!(replay_b.apply(&second));
        assert_eq!(client_a, client_b);
        assert_eq!(replay_a, replay_b);
        assert_eq!(replay_a.title(), "two");
        assert_eq!(replay_a.version(), 2);
    }

    #[test]
    fn stale_and_foreign_events_ignored() {
        let (id, _) = workspace_with_title("v");
        let mut view = WorkspaceView::create(id, "v").unwrap();
        let (other_id, _) = workspace_with_title("other");
        let foreign = MembershipEvent::Renamed {
            workspace: other_id,
            title: "hijack".to_owned(),
            version: 99,
        };
        assert!(!view.apply(&foreign));
        assert_eq!(view.title(), "v");
        let fresh = view.rename(Role::Owner, "new").unwrap();
        assert_eq!(view.version(), 1);
        assert!(!view.apply(&fresh)); // equal version: no-op
        let stale = MembershipEvent::Renamed {
            workspace: id,
            title: "old".to_owned(),
            version: 1,
        };
        assert!(!view.apply(&stale));
        assert_eq!(view.title(), "new");
    }

    #[test]
    fn archive_owner_only_and_converges() {
        let (id, seed) = workspace_with_title("doc");
        let mut owner = seed.clone();
        let mut member = seed;
        assert_eq!(
            member.archive(Role::Member),
            Err(MembershipError::Forbidden("archive requires owner role"))
        );
        assert!(!member.archived());
        // Member rename is allowed but does not archive.
        let rename_v1 = member.rename(Role::Member, "doc v2").unwrap();
        assert!(owner.apply(&rename_v1));
        // Owner archives on top: version 2 wins everywhere.
        let archived_v2 = owner.archive(Role::Owner).unwrap();
        assert_eq!(archived_v2.version(), 2);
        assert!(member.apply(&archived_v2));
        assert!(member.archived());
        assert_eq!(owner, member);
        assert_eq!(member.title(), "doc v2");
        // Same-version conflict: a v2 rename arriving after the v2 archive
        // is stale and keeps the archived state (first-writer wins).
        let rival_v2 = MembershipEvent::Renamed {
            workspace: id,
            title: "rival".to_owned(),
            version: 2,
        };
        assert!(!member.apply(&rival_v2));
        assert!(member.archived());
        // Idempotent re-archive emits the same version.
        let again = member.archive(Role::Owner).unwrap();
        assert_eq!(again.version(), 2);
    }

    #[test]
    fn paginate_bounds() {
        assert_eq!(clamp_page(0), PAGE_MIN);
        assert_eq!(clamp_page(1), 1);
        assert_eq!(clamp_page(PAGE_DEFAULT), PAGE_DEFAULT);
        assert_eq!(clamp_page(usize::MAX), PAGE_MAX);
        let items: Vec<u64> = (0..10).collect();
        assert_eq!(paginate(&items, 0, 0), vec![0]); // floor, never a dump
        assert_eq!(paginate::<u64>(&[], 100, 0), Vec::<u64>::new());
        assert_eq!(paginate(&items, 3, 8), vec![8, 9]);
        assert!(paginate(&items, 5, 10).is_empty()); // offset past end
        assert!(paginate(&items, 5, usize::MAX).is_empty());
        let big: Vec<u64> = (0..1000).collect();
        assert_eq!(paginate(&big, usize::MAX, 0).len(), PAGE_MAX);
        assert!(within_member_cap(MAX_MEMBERS_PER_WORKSPACE));
        assert!(!within_member_cap(MAX_MEMBERS_PER_WORKSPACE + 1));
    }

    #[test]
    fn title_bounds_reject_empty_and_oversize() {
        let id = WorkspaceId::new();
        assert_eq!(
            WorkspaceView::create(id, ""),
            Err(TitleError::Empty)
        );
        let long = "x".repeat(MAX_TITLE_BYTES + 1);
        assert!(matches!(
            WorkspaceView::create(id, &long),
            Err(TitleError::TooLong { .. })
        ));
        let mut view = WorkspaceView::create(id, "ok").unwrap();
        assert_eq!(view.rename(Role::Owner, ""), Err(TitleError::Empty.into()));
        assert_eq!(view.title(), "ok");
        assert_eq!(view.version(), 0); // failed rename bumps nothing
    }
}
