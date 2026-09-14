//! Execution, attempt, and tool lifecycle owner for the format-2 workspace.
//!
//! State gating mirrors the DDL CHECKs in schema/v2/workspace.sql: every
//! transition is one conditional UPDATE, so a lost CAS race surfaces as
//! changed == 0. IDs use SQLite randomblob(16); no new dependencies.
//!
//! ponytail: errors reuse `invalid_input()` (Sqlite InvalidQuery) instead of a
//! dedicated StorageError variant to avoid widening the shared error enum.
//! Upgrade path: add typed variants when the error surface is versioned.

use rusqlite::{params, Connection};

use crate::StorageError;

pub struct ExecV2;

impl ExecV2 {
    /// Start a queued execution. The partial UNIQUE index on sessions with an
    /// active execution rejects a second owner while one is live.
    pub fn start_execution(
        connection: &Connection,
        session_pk: i64,
        mode: u8,
        owner_generation: i64,
        config_payload_pk: i64,
        provider: &str,
        model: &str,
        now_us: i64,
    ) -> Result<i64, StorageError> {
        if mode > 1 || owner_generation < 0 {
            return Err(invalid_input());
        }
        if !(1..=128).contains(&provider.len()) || !(1..=512).contains(&model.len()) {
            return Err(invalid_input());
        }
        connection.execute(
            "INSERT INTO executions
             (id, session_pk, mode, state, owner_generation, config_payload_pk,
              provider_id, model_id, created_at_us)
             VALUES (randomblob(16), ?1, ?2, 0, ?3, ?4, ?5, ?6, ?7)",
            params![
                session_pk,
                i64::from(mode),
                owner_generation,
                config_payload_pk,
                provider,
                model,
                now_us,
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    /// CAS an execution state. finished_us must be Some exactly for terminal
    /// states (2, 3, 5). A stale from_state or missing row yields changed == 0.
    pub fn transition_execution(
        connection: &Connection,
        exec_pk: i64,
        from_state: u8,
        to_state: u8,
        finished_us: Option<i64>,
    ) -> Result<(), StorageError> {
        if !is_allowed_execution_transition(from_state, to_state) {
            return Err(invalid_input());
        }
        if finished_us.is_some() != is_terminal_execution_state(to_state) {
            return Err(invalid_input());
        }
        let changed = connection.execute(
            "UPDATE executions SET state=?1, finished_at_us=?2 WHERE pk=?3 AND state=?4",
            params![
                i64::from(to_state),
                finished_us,
                exec_pk,
                i64::from(from_state),
            ],
        )?;
        if changed == 0 {
            return Err(invalid_input());
        }
        Ok(())
    }

    /// Record a prepared provider attempt for an execution. Rejects terminal
    /// executions (2,3,5) via one conditional INSERT, so the state check and
    /// the insert are atomic without widening the `&Connection` signature to
    /// `&mut` for an IMMEDIATE transaction. Missing row yields changed == 0.
    pub fn record_attempt(
        connection: &Connection,
        exec_pk: i64,
        ordinal: i64,
        request_hash: &[u8; 32],
        through_seq: i64,
        now_us: i64,
    ) -> Result<i64, StorageError> {
        if ordinal < 0 || through_seq < 0 {
            return Err(invalid_input());
        }
        let changed = connection.execute(
            "INSERT INTO provider_attempts
             (id, execution_pk, ordinal, state, request_hash, through_message_seq,
              created_at_us)
             SELECT randomblob(16), ?1, ?2, 0, ?3, ?4, ?5
             WHERE EXISTS (SELECT 1 FROM executions WHERE pk=?1 AND state IN (0,1,4))",
            params![exec_pk, ordinal, &request_hash[..], through_seq, now_us],
        )?;
        if changed == 0 {
            return Err(invalid_input());
        }
        Ok(connection.last_insert_rowid())
    }

    /// Finish an attempt. Uncertain (4) stays open per DDL, so finished_us is
    /// ignored and finished_at_us is cleared to NULL.
    pub fn finish_attempt(
        connection: &Connection,
        attempt_pk: i64,
        to_state: u8,
        finished_us: i64,
    ) -> Result<(), StorageError> {
        if !matches!(to_state, 2 | 3 | 4 | 5) {
            return Err(invalid_input());
        }
        let finished: Option<i64> = if to_state == 4 {
            None
        } else {
            Some(finished_us)
        };
        let changed = connection.execute(
            "UPDATE provider_attempts SET state=?1, finished_at_us=?2 \
             WHERE pk=?3 AND state NOT IN (2,3,5)",
            params![i64::from(to_state), finished, attempt_pk],
        )?;
        if changed == 0 {
            return Err(invalid_input());
        }
        Ok(())
    }

    /// Plan a tool call owned by an assistant message in the same session.
    /// The tool_assistant_owner trigger rejects non-assistant owners.
    pub fn plan_tool(
        connection: &Connection,
        session_pk: i64,
        exec_pk: i64,
        assistant_msg_pk: i64,
        ordinal: i64,
        name: &str,
        intent_hash: &[u8; 32],
        input_payload_pk: i64,
        now_us: i64,
    ) -> Result<i64, StorageError> {
        if !(0..=255).contains(&ordinal) || !(1..=256).contains(&name.len()) {
            return Err(invalid_input());
        }
        connection.execute(
            "INSERT INTO tool_calls
             (id, session_pk, execution_pk, assistant_message_pk, ordinal,
              name, state, intent_hash, input_payload_pk, created_at_us)
             VALUES (randomblob(16), ?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)",
            params![
                session_pk,
                exec_pk,
                assistant_msg_pk,
                ordinal,
                name,
                &intent_hash[..],
                input_payload_pk,
                now_us,
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    /// Finish a tool call. Success (2) needs an output payload, failure (3)
    /// needs an error payload. Uncertain (5) stays open per DDL, so
    /// finished_us is ignored and finished_at_us is cleared to NULL.
    pub fn finish_tool(
        connection: &Connection,
        tool_pk: i64,
        to_state: u8,
        output: Option<i64>,
        error: Option<i64>,
        finished_us: i64,
    ) -> Result<(), StorageError> {
        if !matches!(to_state, 2 | 3 | 4 | 5) {
            return Err(invalid_input());
        }
        if to_state == 2 && output.is_none() {
            return Err(invalid_input());
        }
        if to_state == 3 && error.is_none() {
            return Err(invalid_input());
        }
        let finished: Option<i64> = if to_state == 5 {
            None
        } else {
            Some(finished_us)
        };
        let changed = connection.execute(
            "UPDATE tool_calls
             SET state=?1, output_payload_pk=?2, error_payload_pk=?3, finished_at_us=?4
             WHERE pk=?5 AND state NOT IN (2,3,4)",
            params![i64::from(to_state), output, error, finished, tool_pk],
        )?;
        if changed == 0 {
            return Err(invalid_input());
        }
        Ok(())
    }
}

fn is_terminal_execution_state(state: u8) -> bool {
    matches!(state, 2 | 3 | 5)
}

fn is_allowed_execution_transition(from: u8, to: u8) -> bool {
    matches!(
        (from, to),
        (0, 1) | (0, 5) | (1, 2) | (1, 3) | (1, 4) | (1, 5) | (4, 2) | (4, 3) | (4, 5)
    )
}

fn invalid_input() -> StorageError {
    StorageError::Sqlite(rusqlite::Error::InvalidQuery)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::{MessageId, MessageRole, PayloadRef, SessionId};

    struct Fixture {
        _dir: tempfile::TempDir,
        conn: Connection,
        session_pk: i64,
        config_pk: i64,
        assistant_msg_pk: i64,
        input_pk: i64,
    }

    fn insert_payload(conn: &Connection, data: &[u8], now_us: i64) -> i64 {
        conn.execute(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, ?2, ?3)",
            params![data, data.len() as i64, now_us],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::schema_v2::SchemaV2::initialize_workspace(
            &dir.path().join("w.db"),
            [1_u8; 16],
            [2_u8; 16],
            10,
        )
        .unwrap();
        let session = SessionId::new();
        crate::writer_v2::V2Writer::create_session(
            &mut conn,
            &crate::writer_v2::NewSession {
                id: session,
                title: "exec".to_owned(),
                created_at_us: 100,
                updated_at_us: 100,
            },
        )
        .unwrap();
        let session_pk: i64 = conn
            .query_row(
                "SELECT pk FROM sessions WHERE id=?1",
                params![session.as_uuid().as_bytes().as_slice()],
                |row| row.get(0),
            )
            .unwrap();
        let config_pk = insert_payload(&conn, b"cfg", 100);
        let input_pk = insert_payload(&conn, b"input", 100);
        crate::writer_v2::V2Writer::append_message(
            &mut conn,
            &crate::writer_v2::NewMessage {
                id: MessageId::new(),
                session_id: session,
                role: MessageRole::Assistant,
                body: PayloadRef::Inline {
                    text: "draft".to_owned(),
                },
                created_at_us: 200,
            },
        )
        .unwrap();
        let assistant_msg_pk: i64 = conn
            .query_row(
                "SELECT pk FROM messages WHERE session_pk=?1",
                params![session_pk],
                |row| row.get(0),
            )
            .unwrap();
        Fixture {
            _dir: dir,
            conn,
            session_pk,
            config_pk,
            assistant_msg_pk,
            input_pk,
        }
    }

    fn start(f: &Fixture) -> i64 {
        ExecV2::start_execution(
            &f.conn,
            f.session_pk,
            0,
            0,
            f.config_pk,
            "prov",
            "model",
            1000,
        )
        .unwrap()
    }

    fn exec_state(conn: &Connection, pk: i64) -> (i64, Option<i64>) {
        conn.query_row(
            "SELECT state, finished_at_us FROM executions WHERE pk=?1",
            params![pk],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn start_ok() {
        let f = fixture();
        let pk = start(&f);
        assert!(pk > 0);
        let (state, finished) = exec_state(&f.conn, pk);
        assert_eq!((state, finished), (0, None));
        let (provider, model): (String, String) = f
            .conn
            .query_row(
                "SELECT provider_id, model_id FROM executions WHERE pk=?1",
                params![pk],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((provider.as_str(), model.as_str()), ("prov", "model"));
    }

    #[test]
    fn duplicate_owner_rejected() {
        let f = fixture();
        start(&f);
        let second = ExecV2::start_execution(
            &f.conn,
            f.session_pk,
            0,
            0,
            f.config_pk,
            "prov",
            "model",
            1001,
        );
        assert!(
            second.is_err(),
            "single-owner index must reject a second live execution"
        );
    }

    #[test]
    fn legal_transition() {
        let f = fixture();
        let pk = start(&f);
        ExecV2::transition_execution(&f.conn, pk, 0, 1, None).unwrap();
        assert_eq!(exec_state(&f.conn, pk), (1, None));
    }

    #[test]
    fn illegal_transition_errs() {
        let f = fixture();
        let pk = start(&f);
        assert!(ExecV2::transition_execution(&f.conn, pk, 0, 2, Some(5)).is_err());
        assert!(ExecV2::transition_execution(&f.conn, pk, 1, 0, None).is_err());
        assert!(ExecV2::transition_execution(&f.conn, pk, 1, 2, None).is_err());
        assert_eq!(exec_state(&f.conn, pk), (0, None));
    }

    #[test]
    fn uncertain_to_complete_releases_owner() {
        let f = fixture();
        let pk = start(&f);
        ExecV2::transition_execution(&f.conn, pk, 0, 1, None).unwrap();
        ExecV2::transition_execution(&f.conn, pk, 1, 4, None).unwrap();
        ExecV2::transition_execution(&f.conn, pk, 4, 2, Some(2000)).unwrap();
        assert_eq!(exec_state(&f.conn, pk), (2, Some(2000)));
        // terminal state releases the single-owner slot
        start(&f);
    }

    #[test]
    fn record_and_finish_attempt_ok() {
        let f = fixture();
        let exec = start(&f);
        let attempt = ExecV2::record_attempt(&f.conn, exec, 0, &[9_u8; 32], 1, 1100).unwrap();
        ExecV2::finish_attempt(&f.conn, attempt, 2, 1200).unwrap();
        let (state, finished): (i64, Option<i64>) = f
            .conn
            .query_row(
                "SELECT state, finished_at_us FROM provider_attempts WHERE pk=?1",
                params![attempt],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((state, finished), (2, Some(1200)));
        // uncertain attempts stay open per DDL
        let open = ExecV2::record_attempt(&f.conn, exec, 1, &[8_u8; 32], 1, 1300).unwrap();
        ExecV2::finish_attempt(&f.conn, open, 4, 1400).unwrap();
        let (state, finished): (i64, Option<i64>) = f
            .conn
            .query_row(
                "SELECT state, finished_at_us FROM provider_attempts WHERE pk=?1",
                params![open],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((state, finished), (4, None));
        assert!(ExecV2::finish_attempt(&f.conn, open, 1, 1500).is_err());
    }

    #[test]
    fn finish_attempt_rejects_terminal_rewrite() {
        let f = fixture();
        let exec = start(&f);
        let attempt = ExecV2::record_attempt(&f.conn, exec, 0, &[9_u8; 32], 1, 1100).unwrap();
        ExecV2::finish_attempt(&f.conn, attempt, 2, 1200).unwrap();
        // A terminal attempt must not be rewritten.
        assert!(ExecV2::finish_attempt(&f.conn, attempt, 3, 1300).is_err());
        let state: i64 = f
            .conn
            .query_row(
                "SELECT state FROM provider_attempts WHERE pk=?1",
                params![attempt],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, 2);
    }

    #[test]
    fn record_attempt_rejected_on_terminal_execution() {
        let f = fixture();
        let exec = start(&f);
        ExecV2::transition_execution(&f.conn, exec, 0, 1, None).unwrap();
        ExecV2::transition_execution(&f.conn, exec, 1, 2, Some(2000)).unwrap();
        assert!(ExecV2::record_attempt(&f.conn, exec, 0, &[9_u8; 32], 1, 2100).is_err());
    }

    #[test]
    fn tool_success_without_output_rejected() {
        let f = fixture();
        let exec = start(&f);
        let tool = ExecV2::plan_tool(
            &f.conn,
            f.session_pk,
            exec,
            f.assistant_msg_pk,
            0,
            "tool-a",
            &[7_u8; 32],
            f.input_pk,
            1100,
        )
        .unwrap();
        assert!(ExecV2::finish_tool(&f.conn, tool, 2, None, None, 1200).is_err());
        let out = insert_payload(&f.conn, b"result", 1200);
        ExecV2::finish_tool(&f.conn, tool, 2, Some(out), None, 1200).unwrap();
        let (state, finished): (i64, Option<i64>) = f
            .conn
            .query_row(
                "SELECT state, finished_at_us FROM tool_calls WHERE pk=?1",
                params![tool],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((state, finished), (2, Some(1200)));
    }

    #[test]
    fn tool_failure_without_error_rejected() {
        let f = fixture();
        let exec = start(&f);
        let tool = ExecV2::plan_tool(
            &f.conn,
            f.session_pk,
            exec,
            f.assistant_msg_pk,
            1,
            "tool-b",
            &[6_u8; 32],
            f.input_pk,
            1100,
        )
        .unwrap();
        assert!(ExecV2::finish_tool(&f.conn, tool, 3, None, None, 1200).is_err());
        let err = insert_payload(&f.conn, b"boom", 1200);
        ExecV2::finish_tool(&f.conn, tool, 3, None, Some(err), 1200).unwrap();
        let state: i64 = f
            .conn
            .query_row(
                "SELECT state FROM tool_calls WHERE pk=?1",
                params![tool],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, 3);
    }

    #[test]
    fn finish_tool_rejects_terminal_rewrite() {
        let f = fixture();
        let exec = start(&f);
        let tool = ExecV2::plan_tool(
            &f.conn,
            f.session_pk,
            exec,
            f.assistant_msg_pk,
            2,
            "tool-c",
            &[5_u8; 32],
            f.input_pk,
            1100,
        )
        .unwrap();
        let out = insert_payload(&f.conn, b"ok", 1200);
        ExecV2::finish_tool(&f.conn, tool, 2, Some(out), None, 1200).unwrap();
        // A terminal tool call must not be rewritten.
        assert!(ExecV2::finish_tool(&f.conn, tool, 3, None, Some(out), 1300).is_err());
        let state: i64 = f
            .conn
            .query_row(
                "SELECT state FROM tool_calls WHERE pk=?1",
                params![tool],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, 2);
    }

    #[test]
    fn input_validation_rejects() {
        let f = fixture();
        assert!(
            ExecV2::start_execution(&f.conn, f.session_pk, 0, 0, f.config_pk, "", "m", 1).is_err()
        );
        assert!(
            ExecV2::start_execution(&f.conn, f.session_pk, 0, 0, f.config_pk, "p", "", 1).is_err()
        );
        assert!(
            ExecV2::start_execution(&f.conn, f.session_pk, 2, 0, f.config_pk, "p", "m", 1).is_err()
        );
        let exec = start(&f);
        assert!(ExecV2::plan_tool(
            &f.conn,
            f.session_pk,
            exec,
            f.assistant_msg_pk,
            256,
            "t",
            &[1_u8; 32],
            f.input_pk,
            1
        )
        .is_err());
        assert!(ExecV2::plan_tool(
            &f.conn,
            f.session_pk,
            exec,
            f.assistant_msg_pk,
            0,
            "",
            &[1_u8; 32],
            f.input_pk,
            1
        )
        .is_err());
        assert!(ExecV2::finish_tool(&f.conn, 999_999, 0, None, None, 1).is_err());
    }
}
