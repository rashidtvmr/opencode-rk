//! Lifecycle snapshot helper (FIX-SESSIONS-STUBS step 1).
//!
//! Pure constructor folding recorded `(action, timestamp)` pairs into a
//! [`SessionSummary`]: first action timestamp becomes `created_at`, last
//! becomes `updated_at`. Returns `None` when no actions are recorded, so the
//! caller's transitions vector stays extensible.
//!
//! Generic over the action type so `lifecycle.rs` adopts it with zero edits:
//! pass `&lifecycle.actions` directly once the integrator wires the modules.
//!
//! `ponytail:` wire `pub mod snapshot;` into `sessions/lib.rs` and call
//! [`build_snapshot`] from `SessionLifecycle::snapshot`; upgrade path is
//! moving `id`/`title` into `SessionLifecycle` instead of params (it binds
//! neither today, so the caller supplies identity).
#![forbid(unsafe_code)]

use opencode_rk_contracts::{SessionId, SessionState, SessionSummary, Timestamp};

/// Build a [`SessionSummary`] from recorded lifecycle actions.
///
/// - `created_at` = timestamp of the first action, `updated_at` = last.
/// - `archived_at` = `Some(updated_at)` iff `state` is `Archived`, else `None`.
/// - Returns `None` when `actions` is empty.
pub fn build_snapshot<A>(
    id: SessionId,
    title: impl Into<String>,
    state: SessionState,
    actions: &[(A, Timestamp)],
) -> Option<SessionSummary> {
    let (first, last) = match (actions.first(), actions.last()) {
        (Some((_, first)), Some((_, last))) => (*first, *last),
        _ => return None,
    };
    Some(SessionSummary {
        id,
        title: title.into(),
        state,
        created_at: first,
        updated_at: last,
        archived_at: (state == SessionState::Archived).then_some(last),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn ts(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> Timestamp {
        Timestamp::from_datetime(
            Utc.with_ymd_and_hms(y, mo, d, h, mi, s)
                .single()
                .expect("fixed test timestamp"),
        )
    }

    #[test]
    fn empty_actions_returns_none() {
        let none: Option<SessionSummary> = build_snapshot(
            SessionId::new(),
            "t",
            SessionState::Active,
            &[] as &[(u8, Timestamp)],
        );
        assert!(none.is_none());
    }

    #[test]
    fn single_action_binds_first_last_equal() {
        let at = ts(2026, 1, 2, 3, 4, 5);
        let id = SessionId::new();
        let got = build_snapshot(id, "solo", SessionState::Active, &[(0u8, at)])
            .expect("one action must snapshot");
        assert_eq!(got.id, id);
        assert_eq!(got.title, "solo");
        assert_eq!(got.state, SessionState::Active);
        assert_eq!(got.created_at, at);
        assert_eq!(got.updated_at, at);
        assert_eq!(got.archived_at, None);
    }

    #[test]
    fn multi_action_first_created_last_updated() {
        let first = ts(2026, 3, 1, 0, 0, 0);
        let mid = ts(2026, 3, 2, 0, 0, 0);
        let last = ts(2026, 3, 3, 0, 0, 0);
        let id = SessionId::new();
        let actions = [(0u8, first), (1u8, mid), (2u8, last)];
        let got = build_snapshot(id, "trio", SessionState::Active, &actions)
            .expect("actions must snapshot");
        assert_eq!(got.created_at, first);
        assert_eq!(got.updated_at, last);
        assert_eq!(got.archived_at, None);
        let archived =
            build_snapshot(id, "trio", SessionState::Archived, &actions)
                .expect("actions must snapshot");
        assert_eq!(archived.state, SessionState::Archived);
        assert_eq!(archived.archived_at, Some(last));
    }
}
