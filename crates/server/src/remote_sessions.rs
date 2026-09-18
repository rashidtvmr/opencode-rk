//! NET-007: multi-session routing for remote tabs across devices/workspaces.
//!
//! Daemon owns session history and running work; tab drafts stay client-local.
//! Every tab addresses a [`RouteTriple`] (device/workspace/session). A wrong
//! workspace for a known session is cross-routing and is rejected. Session
//! mutations converge through versioned [`ConvergeEvent`]s (later version
//! wins, stale loses). Closing a tab never kills its session; killing needs
//! an explicit request. Revoking a device drops its subscriptions and blocks
//! subsequently queued actions.
#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

macro_rules! id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub String);
        impl $name {
            pub fn new(s: &str) -> Self {
                Self(s.to_string())
            }
        }
    };
}

id!(DeviceId);
id!(WorkspaceId);
id!(SessionId);
id!(TabId);

/// Where a piece of state lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Home {
    ClientLocal,
    DaemonOwned,
}

/// Full address of one tab's session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteTriple {
    pub device: DeviceId,
    pub workspace: WorkspaceId,
    pub session: SessionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingError {
    UnknownSession,
    CrossWorkspace {
        expected: WorkspaceId,
        got: WorkspaceId,
    },
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvergeError {
    UnknownSession,
    Stale { current: u64, got: u64 },
}

/// Daemon-owned session: history + running work live here.
#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub id: SessionId,
    pub workspace: WorkspaceId,
    pub title: String,
    pub version: u64,
    pub archived: bool,
    pub running: bool,
    pub history: Vec<String>,
}

impl SessionRecord {
    pub fn history_home() -> Home {
        Home::DaemonOwned
    }

    pub fn append_history(&mut self, entry: &str) {
        self.history.push(entry.to_string());
    }
}

/// Client-local tab: draft text never leaves the device until submitted.
#[derive(Debug, Clone)]
pub struct TabState {
    pub tab: TabId,
    pub device: DeviceId,
    pub session: SessionId,
    pub draft: String,
}

impl TabState {
    pub fn draft_home() -> Home {
        Home::ClientLocal
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionOp {
    Rename(String),
    Archive,
    Fork(SessionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvergeEvent {
    pub session: SessionId,
    pub version: u64,
    pub op: SessionOp,
    pub actor: DeviceId,
}

#[derive(Debug, Default)]
pub struct Router {
    sessions: HashMap<SessionId, SessionRecord>,
    tabs: HashMap<TabId, TabState>,
    subs: HashMap<SessionId, HashSet<DeviceId>>,
    revoked: HashSet<DeviceId>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_session(
        &mut self,
        workspace: WorkspaceId,
        session: SessionId,
        title: &str,
    ) {
        self.sessions.insert(
            session.clone(),
            SessionRecord {
                id: session,
                workspace,
                title: title.to_string(),
                version: 0,
                archived: false,
                running: false,
                history: Vec::new(),
            },
        );
    }

    pub fn set_running(&mut self, session: &SessionId, running: bool) -> bool {
        match self.sessions.get_mut(session) {
            Some(r) => {
                r.running = running;
                true
            }
            None => false,
        }
    }

    fn check_route(
        &self,
        device: &DeviceId,
        workspace: &WorkspaceId,
        session: &SessionId,
    ) -> Result<(), RoutingError> {
        if self.revoked.contains(device) {
            return Err(RoutingError::Revoked);
        }
        match self.sessions.get(session) {
            None => Err(RoutingError::UnknownSession),
            Some(rec) if &rec.workspace != workspace => Err(RoutingError::CrossWorkspace {
                expected: rec.workspace.clone(),
                got: workspace.clone(),
            }),
            Some(_) => Ok(()),
        }
    }

    /// Open a tab on a session. Rejects cross-workspace routing.
    pub fn open_tab(&mut self, route: RouteTriple, tab: TabId) -> Result<(), RoutingError> {
        self.check_route(&route.device, &route.workspace, &route.session)?;
        self.tabs.insert(
            tab.clone(),
            TabState {
                tab,
                device: route.device.clone(),
                session: route.session.clone(),
                draft: String::new(),
            },
        );
        self.subs
            .entry(route.session)
            .or_default()
            .insert(route.device);
        Ok(())
    }

    /// Client-local draft edit: touches the tab only, never daemon history.
    pub fn set_draft(&mut self, tab: &TabId, text: &str) -> bool {
        match self.tabs.get_mut(tab) {
            Some(t) => {
                t.draft = text.to_string();
                true
            }
            None => false,
        }
    }

    /// Gate for a queued action (prompt submit). Revoked devices are blocked.
    pub fn queue_action(
        &self,
        device: &DeviceId,
        workspace: &WorkspaceId,
        session: &SessionId,
    ) -> Result<(), RoutingError> {
        self.check_route(device, workspace, session)
    }

    /// Helper: true when revocation blocks this device's queued actions.
    pub fn revocation_blocks_queued(&self, device: &DeviceId) -> bool {
        self.revoked.contains(device)
    }

    /// Apply a versioned mutation; stale versions lose.
    pub fn apply_converge(&mut self, ev: ConvergeEvent) -> Result<(), ConvergeError> {
        let rec = self
            .sessions
            .get_mut(&ev.session)
            .ok_or(ConvergeError::UnknownSession)?;
        if ev.version <= rec.version {
            return Err(ConvergeError::Stale {
                current: rec.version,
                got: ev.version,
            });
        }
        match ev.op {
            SessionOp::Rename(title) => rec.title = title,
            SessionOp::Archive => rec.archived = true,
            SessionOp::Fork(ref new_id) => {
                let child = SessionRecord {
                    id: new_id.clone(),
                    workspace: rec.workspace.clone(),
                    title: format!("{} (fork)", rec.title),
                    version: 0,
                    archived: false,
                    running: false,
                    history: rec.history.clone(),
                };
                // Borrow ends via clone-then-insert: collect fields first.
                let workspace = rec.workspace.clone();
                let _ = workspace;
                rec.version = ev.version;
                self.sessions.insert(new_id.clone(), child);
                return Ok(());
            }
        }
        rec.version = ev.version;
        Ok(())
    }

    /// Close a tab: subscriptions for that tab drop, session keeps running.
    pub fn close_tab(&mut self, tab: &TabId) -> bool {
        match self.tabs.remove(tab) {
            None => false,
            Some(t) => {
                let still_open = self.tabs.values().any(|o| o.device == t.device && o.session == t.session);
                if !still_open {
                    if let Some(set) = self.subs.get_mut(&t.session) {
                        set.remove(&t.device);
                    }
                }
                true
            }
        }
    }

    /// Kill a session only on explicit request; otherwise a no-op.
    /// Also drops its tabs and subscriptions.
    pub fn kill_session(&mut self, session: &SessionId, explicit: bool) -> bool {
        if !explicit {
            return false;
        }
        let removed = self.sessions.remove(session).is_some();
        self.tabs.retain(|_, t| &t.session != session);
        self.subs.remove(session);
        removed
    }

    /// Revoke a device: drops all its subscriptions; queued actions block after.
    pub fn revoke(&mut self, device: &DeviceId) {
        self.revoked.insert(device.clone());
        for set in self.subs.values_mut() {
            set.remove(device);
        }
    }

    pub fn session(&self, id: &SessionId) -> Option<&SessionRecord> {
        self.sessions.get(id)
    }

    pub fn draft(&self, tab: &TabId) -> Option<&str> {
        self.tabs.get(tab).map(|t| t.draft.as_str())
    }

    pub fn is_subscribed(&self, session: &SessionId, device: &DeviceId) -> bool {
        self.subs
            .get(session)
            .map(|s| s.contains(device))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (DeviceId, DeviceId, WorkspaceId, WorkspaceId, SessionId, SessionId) {
        (
            DeviceId::new("phone-a"),
            DeviceId::new("phone-b"),
            WorkspaceId::new("ws-1"),
            WorkspaceId::new("ws-2"),
            SessionId::new("sess-1"),
            SessionId::new("sess-2"),
        )
    }

    fn two_session_router() -> (Router, DeviceId, DeviceId, WorkspaceId, WorkspaceId, SessionId, SessionId) {
        let (da, db, wa, wb, sa, sb) = ids();
        let mut r = Router::new();
        r.register_session(wa.clone(), sa.clone(), "one");
        r.register_session(wb.clone(), sb.clone(), "two");
        (r, da, db, wa, wb, sa, sb)
    }

    // NET-007-T01: no cross-routing between sessions/devices.
    #[test]
    fn cross_routing_rejected() {
        let (mut r, da, db, wa, wb, sa, sb) = two_session_router();
        // Each device drives its own session fine.
        assert!(r
            .open_tab(
                RouteTriple { device: da.clone(), workspace: wa.clone(), session: sa.clone() },
                TabId::new("tab-a"),
            )
            .is_ok());
        assert!(r
            .open_tab(
                RouteTriple { device: db.clone(), workspace: wb.clone(), session: sb.clone() },
                TabId::new("tab-b"),
            )
            .is_ok());
        // Phone A aimed at session B via the wrong workspace: rejected.
        assert_eq!(
            r.open_tab(
                RouteTriple { device: da.clone(), workspace: wa.clone(), session: sb.clone() },
                TabId::new("tab-x"),
            ),
            Err(RoutingError::CrossWorkspace { expected: wb.clone(), got: wa.clone() })
        );
        // Queued prompt with mismatched workspace rejected too.
        assert!(r.queue_action(&db, &wa, &sb).is_err());
        // Correct routes still work after the rejections.
        assert!(r.queue_action(&da, &wa, &sa).is_ok());
        assert!(r.queue_action(&db, &wb, &sb).is_ok());
    }

    // NET-007-T02: drafts are tab-local; history is daemon-owned.
    #[test]
    fn drafts_client_local() {
        let (mut r, da, _, wa, _, sa, _) = two_session_router();
        r.open_tab(
            RouteTriple { device: da, workspace: wa, session: sa.clone() },
            TabId::new("tab-a"),
        )
        .unwrap();
        assert_eq!(TabState::draft_home(), Home::ClientLocal);
        assert_eq!(SessionRecord::history_home(), Home::DaemonOwned);
        assert!(r.set_draft(&TabId::new("tab-a"), "half-typed prompt"));
        assert_eq!(r.draft(&TabId::new("tab-a")), Some("half-typed prompt"));
        // Daemon history untouched by the draft edit.
        assert!(r.session(&sa).unwrap().history.is_empty());
    }

    // NET-007-T03: concurrent rename/archive converge by version; stale loses.
    #[test]
    fn rename_converges() {
        let (mut r, da, db, _, _, sa, _) = two_session_router();
        let _ = (da.clone(), db.clone());
        r.apply_converge(ConvergeEvent {
            session: sa.clone(),
            version: 2,
            op: SessionOp::Rename("from-b".into()),
            actor: db.clone(),
        })
        .unwrap();
        // Stale version 1 loses even though it arrives later.
        assert_eq!(
            r.apply_converge(ConvergeEvent {
                session: sa.clone(),
                version: 1,
                op: SessionOp::Rename("from-a".into()),
                actor: da.clone(),
            }),
            Err(ConvergeError::Stale { current: 2, got: 1 })
        );
        assert_eq!(r.session(&sa).unwrap().title, "from-b");
        r.apply_converge(ConvergeEvent {
            session: sa.clone(),
            version: 3,
            op: SessionOp::Archive,
            actor: da,
        })
        .unwrap();
        let rec = r.session(&sa).unwrap();
        assert!(rec.archived);
        assert_eq!(rec.version, 3);
    }

    // NET-007-T04: closing a tab preserves the background session.
    #[test]
    fn close_tab_preserves_background_session() {
        let (mut r, da, _, wa, _, sa, _) = two_session_router();
        r.open_tab(
            RouteTriple { device: da, workspace: wa, session: sa.clone() },
            TabId::new("tab-a"),
        )
        .unwrap();
        r.set_running(&sa, true);
        r.sessions.get_mut(&sa).unwrap().append_history("turn-1");
        assert!(r.close_tab(&TabId::new("tab-a")));
        // Non-explicit kill is a no-op.
        assert!(!r.kill_session(&sa, false));
        let rec = r.session(&sa).expect("session survives tab close");
        assert!(rec.running);
        assert_eq!(rec.history, vec!["turn-1".to_string()]);
        // Explicit kill really removes it.
        assert!(r.kill_session(&sa, true));
        assert!(r.session(&sa).is_none());
    }

    // NET-007-T05: revocation drops subscriptions and blocks queued actions.
    #[test]
    fn revocation_blocks_queued() {
        let (mut r, da, _, wa, _, sa, _) = two_session_router();
        r.open_tab(
            RouteTriple { device: da.clone(), workspace: wa.clone(), session: sa.clone() },
            TabId::new("tab-a"),
        )
        .unwrap();
        assert!(r.is_subscribed(&sa, &da));
        assert!(!r.revocation_blocks_queued(&da));
        r.revoke(&da);
        assert!(r.revocation_blocks_queued(&da));
        assert!(!r.is_subscribed(&sa, &da));
        assert_eq!(r.queue_action(&da, &wa, &sa), Err(RoutingError::Revoked));
        assert_eq!(
            r.open_tab(
                RouteTriple { device: da, workspace: wa, session: sa },
                TabId::new("tab-rejoin"),
            ),
            Err(RoutingError::Revoked)
        );
    }
}
