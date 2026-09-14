//! Session query module with filtering, pagination, and sorting.

use opencode_rk_contracts::{SessionId, SessionState, SessionSummary, Timestamp};

pub use crate::{SessionError, SessionManager};

/// Field to sort query results by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortField {
    Created,
    Updated,
    Title,
}

/// Filter criteria for querying sessions.
#[derive(Clone, Debug, Default)]
pub struct SessionFilter {
    pub query_id: Option<SessionId>,
    pub title_contains: Option<String>,
    pub created_after: Option<Timestamp>,
    pub state: Option<SessionState>,
}

/// Parameters for querying sessions.
#[derive(Clone, Debug)]
pub struct SessionQuery {
    pub filter: SessionFilter,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort_by: SortField,
}

impl Default for SessionQuery {
    fn default() -> Self {
        Self {
            filter: SessionFilter::default(),
            limit: None,
            offset: None,
            sort_by: SortField::Created,
        }
    }
}

/// Query sessions from a manager with filtering, pagination, and sorting.
pub fn query_sessions(
    manager: &SessionManager,
    query: SessionQuery,
) -> Result<Vec<SessionSummary>, SessionError> {
    let mut results = manager.list_all_sessions(10000, 0)?;

    // Apply filters
    results.retain(|record| matches_filter(&query.filter, record));

    // Sort results
    match query.sort_by {
        SortField::Created => {
            results.sort_by(|a, b| a.created_at.as_datetime().cmp(&b.created_at.as_datetime()))
        }
        SortField::Updated => {
            results.sort_by(|a, b| a.updated_at.as_datetime().cmp(&b.updated_at.as_datetime()))
        }
        SortField::Title => results.sort_by(|a, b| a.title.cmp(&b.title)),
    }

    // Apply pagination
    let offset = query.offset.unwrap_or(0).min(1_000_000);
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);

    let start = offset.min(results.len());
    let end = (offset + limit).min(results.len());

    Ok(results[start..end].to_vec())
}

fn matches_filter(filter: &SessionFilter, record: &SessionSummary) -> bool {
    // Query by ID
    if let Some(id) = filter.query_id {
        if record.id != id {
            return false;
        }
    }

    // Title contains filter (case-insensitive)
    if let Some(ref title_pattern) = filter.title_contains {
        if !record
            .title
            .to_lowercase()
            .contains(&title_pattern.to_lowercase())
        {
            return false;
        }
    }

    // Created after filter
    if let Some(cutoff) = filter.created_after {
        if record.created_at <= cutoff {
            return false;
        }
    }

    // State filter
    if let Some(state) = filter.state {
        if record.state != state {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::SessionState;
    use opencode_rk_storage::SchemaV2;

    fn manager() -> SessionManager {
        let dir = tempfile::tempdir().unwrap();
        let conn =
            SchemaV2::initialize_workspace(&dir.path().join("w.db"), [1_u8; 16], [2_u8; 16], 10)
                .unwrap();
        SessionManager::new(conn)
    }

    #[test]
    fn query_by_state() {
        let mut m = manager();

        // Create sessions and archive one
        let active_id = m.create_session("Active Session").unwrap();
        let archived_id = m.create_session("Archived Session").unwrap();
        m.archive_session(archived_id).unwrap();

        // Query for active sessions only
        let filter = SessionFilter {
            state: Some(SessionState::Active),
            ..Default::default()
        };
        let query = SessionQuery {
            filter,
            ..Default::default()
        };
        let results = query_sessions(&m, query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, active_id);
        assert_eq!(results[0].state, SessionState::Active);

        // Query for archived sessions
        let filter = SessionFilter {
            state: Some(SessionState::Archived),
            ..Default::default()
        };
        let query = SessionQuery {
            filter,
            ..Default::default()
        };
        let results = query_sessions(&m, query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, archived_id);
        assert_eq!(results[0].state, SessionState::Archived);
    }

    #[test]
    fn pagination_works() {
        let mut m = manager();

        // Create 5 sessions
        for i in 0..5 {
            m.create_session(&format!("Session {}", i)).unwrap();
        }

        // Query with limit 2, offset 0
        let query = SessionQuery {
            filter: SessionFilter::default(),
            limit: Some(2),
            offset: Some(0),
            sort_by: SortField::Created,
        };
        let results = query_sessions(&m, query).unwrap();
        assert_eq!(results.len(), 2);

        // Query with limit 2, offset 2
        let query = SessionQuery {
            filter: SessionFilter::default(),
            limit: Some(2),
            offset: Some(2),
            sort_by: SortField::Created,
        };
        let results = query_sessions(&m, query).unwrap();
        assert_eq!(results.len(), 2);

        // Query with limit 10, offset 10 (beyond available)
        let query = SessionQuery {
            filter: SessionFilter::default(),
            limit: Some(10),
            offset: Some(10),
            sort_by: SortField::Created,
        };
        let results = query_sessions(&m, query).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn sort_by_created() {
        let mut m = manager();

        // Create sessions in order
        let id1 = m.create_session("Alpha").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id2 = m.create_session("Beta").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id3 = m.create_session("Gamma").unwrap();

        // Query sorted by created ascending
        let query = SessionQuery {
            filter: SessionFilter::default(),
            limit: None,
            offset: None,
            sort_by: SortField::Created,
        };
        let results = query_sessions(&m, query).unwrap();
        assert_eq!(results.len(), 3);

        // Results should be sorted by created_at ascending
        for i in 1..results.len() {
            assert!(results[i - 1].created_at.as_datetime() <= results[i].created_at.as_datetime());
        }

        // The first one should be Alpha (created first)
        assert_eq!(results[0].id, id1);
        assert_eq!(results[0].title, "Alpha");
    }

    #[test]
    fn title_filter() {
        let mut m = manager();
        m.create_session("Alpha Project").unwrap();
        m.create_session("Beta Project").unwrap();
        m.create_session("Gamma Task").unwrap();

        // Filter by "project" (case-insensitive)
        let filter = SessionFilter {
            title_contains: Some("project".to_string()),
            ..Default::default()
        };
        let query = SessionQuery {
            filter,
            ..Default::default()
        };
        let results = query_sessions(&m, query).unwrap();
        assert_eq!(results.len(), 2);
        for record in &results {
            assert!(record.title.to_lowercase().contains("project"));
        }

        // Filter by "GAMMA" (case-insensitive)
        let filter = SessionFilter {
            title_contains: Some("GAMMA".to_string()),
            ..Default::default()
        };
        let query = SessionQuery {
            filter,
            ..Default::default()
        };
        let results = query_sessions(&m, query).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("Gamma"));
    }

    #[test]
    fn combined_filters() {
        let mut m = manager();
        let active_id = m.create_session("Project Alpha").unwrap();
        let _ = m.create_session("Project Beta").unwrap();
        let _ = m.create_session("Task Gamma").unwrap();
        // Archive one project session
        m.archive_session(active_id).unwrap();

        // Filter by state AND title_contains
        let filter = SessionFilter {
            state: Some(SessionState::Active),
            title_contains: Some("Project".to_string()),
            ..Default::default()
        };
        let query = SessionQuery {
            filter,
            limit: Some(10),
            offset: Some(0),
            sort_by: SortField::Title,
        };
        let results = query_sessions(&m, query).unwrap();

        // Should find only "Project Beta" (Beta is active, Alpha is archived)
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Project Beta");
        assert_eq!(results[0].state, SessionState::Active);
    }
}
