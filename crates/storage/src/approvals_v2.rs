//! Approval transition guards for the format-2 workspace schema.
//!
//! The `approvals` DDL declares states but enforces no transitions
//! (zero triggers), so every pending-to-terminal move is guarded here:
//! single-winner resolve on `state = 0`, terminal set limited to
//! allowed-once/denied/consumed, mandatory-human approvals requiring a
//! client identity, and expiry owned solely by the sweep.
#![forbid(unsafe_code)]

use rusqlite::{params, Connection, OptionalExtension};

use crate::StorageError;

const MAX_ACTION_BYTES: usize = 128;
const MAX_RESOURCE_BYTES: usize = 4096;
const MAX_SWEEP_ROWS: usize = 500;

/// Decision-record transitions. Records are evidence only: the trusted
/// broker still authorizes use. States: 0 pending, 1 allowed-once,
/// 2 denied, 3 expired, 4 consumed. Expiry (3) is owned by
/// [`ApprovalsV2::expire_sweep`]; [`ApprovalsV2::resolve`] reaches
/// 1, 2, or 4 only.
pub struct ApprovalsV2;

impl ApprovalsV2 {
    pub fn request(
        connection: &Connection,
        session_pk: i64,
        tool_call: Option<(i64, i64)>,
        intent_hash: &[u8; 32],
        policy_generation: i64,
        action: &str,
        mandatory_human: bool,
        human_client: Option<&[u8; 16]>,
        created_us: i64,
        expires_us: i64,
    ) -> Result<i64, StorageError> {
        if expires_us <= created_us {
            return Err(invalid_input(
                "approvals expires_at_us must be after created_at_us",
            ));
        }
        if !(1..=MAX_ACTION_BYTES).contains(&action.len()) {
            return Err(invalid_input(
                "approvals action must be between 1 and 128 bytes",
            ));
        }
        if policy_generation < 0 {
            return Err(invalid_input("approvals policy_generation must be >= 0"));
        }
        let tool_call_pk: Option<i64> = match tool_call {
            Some((tool_pk, tool_session_pk)) => {
                if tool_session_pk != session_pk {
                    return Err(invalid_input(
                        "approvals tool_call must belong to the same session",
                    ));
                }
                Some(tool_pk)
            }
            None => None,
        };
        connection.execute(
            "INSERT INTO approvals
             (id, session_pk, tool_call_pk, intent_hash, policy_generation, action,
              state, mandatory_human, human_client_id, created_at_us, expires_at_us, resolved_at_us)
             VALUES (randomblob(16), ?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8, ?9, NULL)",
            params![
                session_pk,
                tool_call_pk,
                &intent_hash[..],
                policy_generation,
                action,
                i64::from(mandatory_human),
                human_client.map(|id| &id[..]),
                created_us,
                expires_us,
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn resolve(
        connection: &Connection,
        approval_pk: i64,
        to_state: u8,
        human_client: Option<&[u8; 16]>,
        resolved_us: i64,
    ) -> Result<(), StorageError> {
        if !matches!(to_state, 1 | 2 | 4) {
            return Err(invalid_input("approvals resolve target must be 1, 2, or 4"));
        }
        let mandatory: Option<i64> = connection
            .query_row(
                "SELECT mandatory_human FROM approvals WHERE pk = ?1 AND state = 0",
                [approval_pk],
                |row| row.get(0),
            )
            .optional()?;
        let Some(mandatory) = mandatory else {
            return Err(invalid_input(
                "approvals resolve requires a pending approval",
            ));
        };
        if mandatory == 1 && human_client.is_none() {
            return Err(invalid_input(
                "approvals mandatory_human requires a human client identity",
            ));
        }
        let changed = connection.execute(
            "UPDATE approvals
             SET state = ?1,
                 human_client_id = COALESCE(?2, human_client_id),
                 resolved_at_us = ?3
             WHERE pk = ?4 AND state = 0",
            params![
                i64::from(to_state),
                human_client.map(|id| &id[..]),
                resolved_us,
                approval_pk,
            ],
        )?;
        if changed == 0 {
            return Err(invalid_input(
                "approvals resolve requires a pending approval",
            ));
        }
        Ok(())
    }

    pub fn expire_sweep(
        connection: &Connection,
        now_us: i64,
        limit: usize,
    ) -> Result<usize, StorageError> {
        let limit = limit.clamp(1, MAX_SWEEP_ROWS) as i64;
        let changed = connection.execute(
            "UPDATE approvals SET state = 3 WHERE pk IN (
               SELECT pk FROM approvals
               WHERE state = 0 AND expires_at_us <= ?1
               ORDER BY expires_at_us ASC, pk ASC LIMIT ?2
             )",
            params![now_us, limit],
        )?;
        Ok(changed as usize)
    }

    pub fn add_resource(
        connection: &Connection,
        approval_pk: i64,
        ordinal: i64,
        resource: &str,
    ) -> Result<(), StorageError> {
        if !(0..=255).contains(&ordinal) {
            return Err(invalid_input(
                "approval_resources ordinal must be between 0 and 255",
            ));
        }
        if !(1..=MAX_RESOURCE_BYTES).contains(&resource.len()) {
            return Err(invalid_input(
                "approval_resources resource must be between 1 and 4096 bytes",
            ));
        }
        connection.execute(
            "INSERT INTO approval_resources (approval_pk, ordinal, resource)
             VALUES (?1, ?2, ?3)",
            params![approval_pk, ordinal, resource],
        )?;
        Ok(())
    }
}

fn invalid_input(message: &str) -> StorageError {
    StorageError::Sqlite(rusqlite::Error::InvalidParameterName(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_v2::SchemaV2;
    use tempfile::TempDir;

    fn workspace() -> (TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = SchemaV2::initialize_workspace(
            &dir.path().join("workspace.db"),
            [1_u8; 16],
            [2_u8; 16],
            0,
        )
        .unwrap();
        (dir, conn)
    }

    fn session_pk(connection: &Connection) -> i64 {
        connection
            .execute(
                "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
                 VALUES (randomblob(16), 't', 0, 1)",
                [],
            )
            .unwrap();
        connection.last_insert_rowid()
    }

    fn pending(connection: &Connection, session: i64, created_us: i64, expires_us: i64) -> i64 {
        ApprovalsV2::request(
            connection,
            session,
            None,
            &[7_u8; 32],
            1,
            "delete",
            false,
            None,
            created_us,
            expires_us,
        )
        .unwrap()
    }

    fn state_of(connection: &Connection, pk: i64) -> (i64, Option<i64>) {
        connection
            .query_row(
                "SELECT state, resolved_at_us FROM approvals WHERE pk = ?1",
                [pk],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    }

    #[test]
    fn request_ok_starts_pending() {
        let (_dir, conn) = workspace();
        let session = session_pk(&conn);
        let pk = pending(&conn, session, 0, 100);
        assert!(pk > 0);
        assert_eq!(state_of(&conn, pk), (0, None));
        ApprovalsV2::add_resource(&conn, pk, 0, "file:///tmp/x").unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM approval_resources WHERE approval_pk = ?1",
                [pk],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn double_resolve_errors() {
        let (_dir, conn) = workspace();
        let session = session_pk(&conn);
        let pk = pending(&conn, session, 0, 100);
        ApprovalsV2::resolve(&conn, pk, 1, None, 10).unwrap();
        assert_eq!(state_of(&conn, pk), (1, Some(10)));
        assert!(ApprovalsV2::resolve(&conn, pk, 2, None, 11).is_err());
        assert!(ApprovalsV2::resolve(&conn, 999_999, 1, None, 11).is_err());
        assert_eq!(state_of(&conn, pk), (1, Some(10)));
    }

    #[test]
    fn resolve_to_0_or_3_errors() {
        let (_dir, conn) = workspace();
        let session = session_pk(&conn);
        let pk = pending(&conn, session, 0, 100);
        assert!(ApprovalsV2::resolve(&conn, pk, 0, None, 10).is_err());
        assert!(ApprovalsV2::resolve(&conn, pk, 3, None, 10).is_err());
        assert!(ApprovalsV2::resolve(&conn, pk, 5, None, 10).is_err());
        assert_eq!(state_of(&conn, pk), (0, None));
    }

    #[test]
    fn mandatory_without_human_errors() {
        let (_dir, conn) = workspace();
        let session = session_pk(&conn);
        let pk = ApprovalsV2::request(
            &conn,
            session,
            None,
            &[9_u8; 32],
            1,
            "delete",
            true,
            None,
            0,
            100,
        )
        .unwrap();
        assert!(ApprovalsV2::resolve(&conn, pk, 1, None, 10).is_err());
        assert!(ApprovalsV2::resolve(&conn, pk, 2, None, 10).is_err());
        ApprovalsV2::resolve(&conn, pk, 1, Some(&[3_u8; 16]), 10).unwrap();
        let stored: Vec<u8> = conn
            .query_row(
                "SELECT human_client_id FROM approvals WHERE pk = ?1",
                [pk],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, vec![3_u8; 16]);
    }

    #[test]
    fn sweep_only_due() {
        let (_dir, conn) = workspace();
        let session = session_pk(&conn);
        let due = pending(&conn, session, 0, 100);
        let fresh = pending(&conn, session, 0, 10_000);
        assert_eq!(ApprovalsV2::expire_sweep(&conn, 100, 500).unwrap(), 1);
        assert_eq!(state_of(&conn, due).0, 3);
        assert_eq!(state_of(&conn, fresh).0, 0);
        assert!(ApprovalsV2::resolve(&conn, due, 1, None, 101).is_err());
        assert_eq!(ApprovalsV2::expire_sweep(&conn, 100, 500).unwrap(), 0);
    }

    #[test]
    fn request_rejects_bad_window_action_policy() {
        let (_dir, conn) = workspace();
        let session = session_pk(&conn);
        let base = |conn: &Connection| {
            ApprovalsV2::request(
                conn,
                session,
                None,
                &[7_u8; 32],
                1,
                "delete",
                false,
                None,
                0,
                100,
            )
        };
        assert!(base(&conn).is_ok());
        assert!(ApprovalsV2::request(
            &conn,
            session,
            None,
            &[7_u8; 32],
            1,
            "delete",
            false,
            None,
            100,
            100
        )
        .is_err());
        assert!(ApprovalsV2::request(
            &conn,
            session,
            None,
            &[7_u8; 32],
            1,
            "",
            false,
            None,
            0,
            100
        )
        .is_err());
        assert!(ApprovalsV2::request(
            &conn,
            session,
            None,
            &[7_u8; 32],
            1,
            &"a".repeat(129),
            false,
            None,
            0,
            100
        )
        .is_err());
        assert!(ApprovalsV2::request(
            &conn,
            session,
            None,
            &[7_u8; 32],
            -1,
            "delete",
            false,
            None,
            0,
            100
        )
        .is_err());
        assert!(ApprovalsV2::request(
            &conn,
            session,
            Some((1, session + 1)),
            &[7_u8; 32],
            1,
            "delete",
            false,
            None,
            0,
            100
        )
        .is_err());
    }

    #[test]
    fn add_resource_rejects_bad_bounds() {
        let (_dir, conn) = workspace();
        let session = session_pk(&conn);
        let pk = pending(&conn, session, 0, 100);
        assert!(ApprovalsV2::add_resource(&conn, pk, -1, "r").is_err());
        assert!(ApprovalsV2::add_resource(&conn, pk, 256, "r").is_err());
        assert!(ApprovalsV2::add_resource(&conn, pk, 0, "").is_err());
        assert!(ApprovalsV2::add_resource(&conn, pk, 0, &"r".repeat(4097)).is_err());
        assert!(ApprovalsV2::add_resource(&conn, 999_999, 0, "r").is_err());
    }

    #[test]
    fn resolve_denied_and_consumed_ok() {
        let (_dir, conn) = workspace();
        let session = session_pk(&conn);
        let denied = pending(&conn, session, 0, 100);
        ApprovalsV2::resolve(&conn, denied, 2, None, 20).unwrap();
        assert_eq!(state_of(&conn, denied), (2, Some(20)));
        let consumed = pending(&conn, session, 0, 100);
        ApprovalsV2::resolve(&conn, consumed, 4, Some(&[5_u8; 16]), 21).unwrap();
        assert_eq!(state_of(&conn, consumed), (4, Some(21)));
    }
}
