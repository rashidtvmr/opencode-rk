//! Bounded fork operations for the format-2 workspace schema.
//!
//! A fork creates a NEW session whose history up to a boundary reuses the parent's
//! immutable payload references (never copies bodies) and records fork provenance
//! on the session row. Parent deletion cascades through the parent's own rows only;
//! the fork's shared payload references (message_parts -> payloads ON DELETE
//! RESTRICT) keep the content alive. Copying is bounded so a caller can page.
//!
//! ponytail: cap errors reuse `invalid_input()` (Sqlite InvalidQuery) instead of a
//! dedicated StorageError variant to avoid widening the shared error enum. Upgrade
//! path: add fork-specific variants when the error surface is versioned.

use opencode_rk_contracts::{MessageId, SessionId};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::StorageError;

const MAX_FORK_COPY_MESSAGES: usize = 500;
const MAX_FORK_DEPTH: usize = 8;

pub struct ForkV2;

impl ForkV2 {
    /// Create a new session forking the parent's history through `through_seq`.
    ///
    /// Returns the new session's integer pk. Copies messages as NEW rows reusing
    /// the same payload_pk references (bodies are shared, not duplicated). If more
    /// than 500 messages fall in the range, returns Err: the caller must page.
    pub fn fork_session(
        connection: &mut Connection,
        parent_session_id_bytes: &[u8],
        through_seq: i64,
        new_session_id: SessionId,
        title: &str,
        now_us: i64,
    ) -> Result<i64, StorageError> {
        if title.len() > 1024 {
            return Err(invalid_input());
        }
        if through_seq < 0 {
            return Err(invalid_input());
        }

        // enforce fork depth: walk parent chain and reject if at cap
        {
            let mut current_id: Vec<u8> = parent_session_id_bytes.to_vec();
            let mut depth = 0usize;
            while depth < MAX_FORK_DEPTH {
                let is_fork: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM sessions WHERE id=?1 AND fork_parent_id IS NOT NULL",
                    params![current_id],
                    |r| r.get(0),
                )?;
                if is_fork == 0 {
                    break;
                }
                depth += 1;
                let parent: Vec<u8> = connection.query_row(
                    "SELECT fork_parent_id FROM sessions WHERE id=?1",
                    params![current_id],
                    |r| r.get(0),
                )?;
                current_id = parent;
            }
            if depth >= MAX_FORK_DEPTH {
                return Err(invalid_input());
            }
        }

        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let parent_pk: i64 = transaction
            .query_row(
                "SELECT pk FROM sessions WHERE id=?1",
                params![parent_session_id_bytes],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(invalid_input)?;

        let in_range: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM messages WHERE session_pk=?1 AND seq<=?2",
            params![parent_pk, through_seq],
            |row| row.get(0),
        )?;
        if in_range as usize > MAX_FORK_COPY_MESSAGES {
            // bounded: the caller pages over boundaries
            return Err(invalid_input());
        }

        transaction.execute(
            "INSERT INTO sessions
             (id, title, state, agent_name, fork_parent_id, fork_message_seq,
              next_message_seq, next_input_seq, created_at_us, updated_at_us)
             VALUES (?1, ?2, 0, 'default', ?3, ?4, ?5, 1, ?6, ?6)",
            params![
                new_session_id.as_uuid().as_bytes().as_slice(),
                title,
                parent_session_id_bytes,
                through_seq,
                in_range + 1,
                now_us,
            ],
        )?;
        let new_pk = transaction.last_insert_rowid();

        // fetch source messages: pk, seq, role, status, provider, model, times
        let rows: Vec<(
            i64,
            i64,
            i64,
            i64,
            Option<String>,
            Option<String>,
            i64,
            Option<i64>,
        )> = {
            let mut stmt = transaction.prepare(
                "SELECT pk, seq, role, status, provider_id, model_id, created_at_us, completed_at_us
                 FROM messages WHERE session_pk=?1 AND seq<=?2 ORDER BY seq ASC",
            )?;
            let mapped = stmt.query_map(params![parent_pk, through_seq], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            })?;
            let mut out = Vec::new();
            for row in mapped {
                out.push(row?);
            }
            out
        };

        let mut max_seq = 0i64;
        for (source_pk, seq, role, status, provider, model, created, completed) in rows {
            let new_id = MessageId::new();
            transaction.execute(
                "INSERT INTO messages
                 (id, session_pk, seq, role, status, provider_id, model_id,
                  created_at_us, completed_at_us)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    new_id.as_uuid().as_bytes().as_slice(),
                    new_pk,
                    seq,
                    role,
                    status,
                    provider,
                    model,
                    created,
                    completed,
                ],
            )?;
            let new_message_pk = transaction.last_insert_rowid();

            let parts: Vec<(
                i64,
                i64,
                i64,
                Option<String>,
                Option<String>,
                Option<String>,
            )> = {
                let mut pstmt = transaction.prepare(
                    "SELECT ordinal, kind, payload_pk, mime, name, metadata_json
                     FROM message_parts WHERE message_pk=?1 ORDER BY ordinal ASC",
                )?;
                let mapped = pstmt.query_map(params![source_pk], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                })?;
                let mut out = Vec::new();
                for part in mapped {
                    out.push(part?);
                }
                out
            };
            for (ordinal, kind, payload_pk, mime, name, metadata) in parts {
                transaction.execute(
                    "INSERT INTO message_parts
                     (message_pk, ordinal, kind, payload_pk, mime, name, metadata_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        new_message_pk,
                        ordinal,
                        kind,
                        payload_pk,
                        mime,
                        name,
                        metadata
                    ],
                )?;
            }
            if seq > max_seq {
                max_seq = seq;
            }
        }

        let next_seq = max_seq.checked_add(1).ok_or_else(invalid_input)?;
        transaction.execute(
            "UPDATE sessions SET next_message_seq=?1 WHERE pk=?2",
            params![next_seq, new_pk],
        )?;
        transaction.commit()?;
        Ok(new_pk)
    }

    /// Verify a copied fork before catalog switch: message count matches and the
    /// head hash (blake3 over concatenated payload inline bytes in seq/ordinal
    /// order) matches. Any mismatch returns Err.
    pub fn verify_copy(
        connection: &Connection,
        new_pk: i64,
        expected_count: i64,
        expected_head_hash: &[u8],
    ) -> Result<(), StorageError> {
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM messages WHERE session_pk=?1",
            params![new_pk],
            |row| row.get(0),
        )?;
        if count != expected_count {
            return Err(invalid_input());
        }

        let mut hasher = blake3::Hasher::new();
        let mut stmt = connection.prepare(
            "SELECT p.inline_data
             FROM messages m
             JOIN message_parts mp ON mp.message_pk = m.pk
             JOIN payloads p ON p.pk = mp.payload_pk
             WHERE m.session_pk=?1
             ORDER BY m.seq ASC, mp.ordinal ASC",
        )?;
        let rows = stmt.query_map(params![new_pk], |row| Ok(row.get::<_, Option<Vec<u8>>>(0)?))?;
        for row in rows {
            if let Some(data) = row? {
                hasher.update(&data);
            }
        }
        drop(stmt);

        if hasher.finalize().as_bytes() != expected_head_hash {
            return Err(invalid_input());
        }
        Ok(())
    }

    /// Delete a whole session and its dependent rows via ON DELETE CASCADE.
    /// Shared payloads survive because the fork's message_parts keep RESTRICT
    /// references; payload rows are never cascade-deleted.
    pub fn delete_session_tree(
        connection: &mut Connection,
        session_pk: i64,
    ) -> Result<(), StorageError> {
        connection.execute("DELETE FROM sessions WHERE pk=?1", params![session_pk])?;
        Ok(())
    }
}

fn invalid_input() -> StorageError {
    StorageError::Sqlite(rusqlite::Error::InvalidQuery)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::{MessageRole, PayloadRef, SessionId};
    use rusqlite::Connection;
    use std::path::Path;

    fn workspace(path: &Path) -> Connection {
        crate::schema_v2::SchemaV2::initialize_workspace(path, [1_u8; 16], [2_u8; 16], 10).unwrap()
    }

    fn create_parent(conn: &mut Connection, id: SessionId) -> i64 {
        crate::writer_v2::V2Writer::create_session(
            conn,
            &crate::writer_v2::NewSession {
                id,
                title: "parent".to_owned(),
                created_at_us: 100,
                updated_at_us: 100,
            },
        )
        .unwrap();
        conn.query_row(
            "SELECT pk FROM sessions WHERE id=?1",
            params![id.as_uuid().as_bytes().as_slice()],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn append(conn: &mut Connection, session: SessionId, seq: i64, body: &str) {
        crate::writer_v2::V2Writer::append_message(
            conn,
            &crate::writer_v2::NewMessage {
                id: MessageId::new(),
                session_id: session,
                role: MessageRole::User,
                body: PayloadRef::Inline {
                    text: body.to_owned(),
                },
                created_at_us: 1000 + seq,
            },
        )
        .unwrap();
    }

    fn payload_pks(conn: &Connection, session_pk: i64) -> Vec<i64> {
        let mut stmt = conn
            .prepare(
                "SELECT p.pk FROM messages m
                 JOIN message_parts mp ON mp.message_pk = m.pk
                 JOIN payloads p ON p.pk = mp.payload_pk
                 WHERE m.session_pk=?1 ORDER BY m.seq ASC",
            )
            .unwrap();
        stmt.query_map(params![session_pk], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    fn head_hash(conn: &Connection, session_pk: i64) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        let mut stmt = conn
            .prepare(
                "SELECT p.inline_data FROM messages m
                 JOIN message_parts mp ON mp.message_pk = m.pk
                 JOIN payloads p ON p.pk = mp.payload_pk
                 WHERE m.session_pk=?1 ORDER BY m.seq ASC, mp.ordinal ASC",
            )
            .unwrap();
        let rows = stmt
            .query_map(params![session_pk], |r| r.get::<_, Option<Vec<u8>>>(0))
            .unwrap();
        for row in rows {
            if let Some(data) = row.unwrap() {
                hasher.update(&data);
            }
        }
        let mut out = [0_u8; 32];
        out.copy_from_slice(hasher.finalize().as_bytes());
        out
    }

    #[test]
    fn fork_copies_metadata_and_shares_payload_pk() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let parent = SessionId::new();
        let parent_pk = create_parent(&mut conn, parent);
        append(&mut conn, parent, 1, "m1");
        append(&mut conn, parent, 2, "m2");
        append(&mut conn, parent, 3, "m3");

        let fork_id = SessionId::new();
        let fork_pk = ForkV2::fork_session(
            &mut conn,
            parent.as_uuid().as_bytes(),
            2,
            fork_id,
            "fork",
            5000,
        )
        .unwrap();

        let (got_parent, got_seq): (Option<Vec<u8>>, Option<i64>) = conn
            .query_row(
                "SELECT fork_parent_id, fork_message_seq FROM sessions WHERE pk=?1",
                params![fork_pk],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            got_parent.as_deref(),
            Some(parent.as_uuid().as_bytes().as_slice())
        );
        assert_eq!(got_seq, Some(2));

        let msg_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_pk=?1",
                params![fork_pk],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(msg_count, 2); // bounded: through_seq=2 copied 2

        // shared payload references: fork payload pk set equals parent's first-2
        let parent_pks = payload_pks(&conn, parent_pk);
        let fork_pks = payload_pks(&conn, fork_pk);
        assert_eq!(fork_pks, parent_pks[..2]);

        let next_seq: i64 = conn
            .query_row(
                "SELECT next_message_seq FROM sessions WHERE pk=?1",
                params![fork_pk],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(next_seq, 3); // max_copied_seq + 1
    }

    #[test]
    fn fork_beyond_cap_errors() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let parent = SessionId::new();
        create_parent(&mut conn, parent);
        for i in 1..=501 {
            append(&mut conn, parent, i, "x");
        }
        let fork_id = SessionId::new();
        let result = ForkV2::fork_session(
            &mut conn,
            parent.as_uuid().as_bytes(),
            501,
            fork_id,
            "big",
            5000,
        );
        assert!(result.is_err(), "fork beyond the copy cap must error");
    }

    #[test]
    fn verify_copy_detects_tamper() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let parent = SessionId::new();
        let parent_pk = create_parent(&mut conn, parent);
        append(&mut conn, parent, 1, "m1");
        append(&mut conn, parent, 2, "m2");

        let fork_id = SessionId::new();
        let fork_pk = ForkV2::fork_session(
            &mut conn,
            parent.as_uuid().as_bytes(),
            2,
            fork_id,
            "fork",
            5000,
        )
        .unwrap();

        let expected = head_hash(&conn, fork_pk);
        ForkV2::verify_copy(&conn, fork_pk, 2, &expected).unwrap();

        // Tamper the fork's structure: bind an extra, unrelated payload to the
        // first message part. Payload rows are immutable, but adding a new part
        // binding is legal and changes the verified hash. This is the realistic
        // tamper vector given payload immutability.
        let extra_payload: i64 = conn
            .query_row(
                "INSERT INTO payloads(inline_data,raw_bytes,created_at_us) VALUES(?1,?2,1) RETURNING pk",
                params![b"injected".to_vec(), 8_i64],
                |r| r.get(0),
            )
            .unwrap();
        let msg_pk: i64 = conn
            .query_row(
                "SELECT pk FROM messages WHERE session_pk=?1 ORDER BY seq LIMIT 1",
                params![fork_pk],
                |r| r.get(0),
            )
            .unwrap();
        let max_ord: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(ordinal)+1,0) FROM message_parts WHERE message_pk=?1",
                params![msg_pk],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO message_parts(message_pk,ordinal,kind,payload_pk) VALUES(?1,?2,0,?3)",
            params![msg_pk, max_ord, extra_payload],
        )
        .unwrap();
        assert!(ForkV2::verify_copy(&conn, fork_pk, 2, &expected).is_err());
    }

    #[test]
    fn parent_delete_keeps_fork_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let parent = SessionId::new();
        create_parent(&mut conn, parent);
        append(&mut conn, parent, 1, "m1");
        append(&mut conn, parent, 2, "m2");

        let fork_id = SessionId::new();
        let fork_pk = ForkV2::fork_session(
            &mut conn,
            parent.as_uuid().as_bytes(),
            2,
            fork_id,
            "fork",
            5000,
        )
        .unwrap();
        let fork_pks = payload_pks(&conn, fork_pk);

        // delete the whole parent session via its actual pk
        let parent_pk: i64 = conn
            .query_row(
                "SELECT pk FROM sessions WHERE id=?1",
                params![parent.as_uuid().as_bytes().as_slice()],
                |r| r.get(0),
            )
            .unwrap();
        ForkV2::delete_session_tree(&mut conn, parent_pk).unwrap();

        // parent gone, fork survives and still references its payloads
        let parent_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE id=?1",
                params![parent.as_uuid().as_bytes().as_slice()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(parent_exists, 0);

        let msg_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_pk=?1",
                params![fork_pk],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(msg_count, 2);

        let still = payload_pks(&conn, fork_pk);
        assert_eq!(still, fork_pks); // same payload references survive parent delete
        for p in &still {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM payloads WHERE pk=?1",
                    params![p],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1);
        }
    }

    #[test]
    fn fork_provenance_cols_set() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let parent = SessionId::new();
        create_parent(&mut conn, parent);
        append(&mut conn, parent, 1, "m1");
        append(&mut conn, parent, 2, "m2");
        append(&mut conn, parent, 3, "m3");

        let fork_id = SessionId::new();
        let fork_pk = ForkV2::fork_session(
            &mut conn,
            parent.as_uuid().as_bytes(),
            1,
            fork_id,
            "fork",
            5000,
        )
        .unwrap();

        let (fork_parent, fork_seq): (Vec<u8>, i64) = conn
            .query_row(
                "SELECT fork_parent_id, fork_message_seq FROM sessions WHERE pk=?1",
                params![fork_pk],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            fork_parent.as_slice(),
            parent.as_uuid().as_bytes().as_slice()
        );
        assert_eq!(fork_seq, 1);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_pk=?1",
                params![fork_pk],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn fork_inherits_history() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let parent = SessionId::new();
        let parent_pk = create_parent(&mut conn, parent);
        append(&mut conn, parent, 1, "m1");
        append(&mut conn, parent, 2, "m2");

        let fork_id = SessionId::new();
        let fork_pk = ForkV2::fork_session(
            &mut conn,
            parent.as_uuid().as_bytes(),
            2,
            fork_id,
            "fork",
            5000,
        )
        .unwrap();

        // fork inherits parent's messages
        let fork_msg_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_pk=?1",
                params![fork_pk],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fork_msg_count, 2);

        // fork messages share payload references with parent
        let parent_pks = payload_pks(&conn, parent_pk);
        let fork_pks = payload_pks(&conn, fork_pk);
        assert_eq!(fork_pks, parent_pks[..2]);

        // verify fork points to parent
        let got_parent: Option<Vec<u8>> = conn
            .query_row(
                "SELECT fork_parent_id FROM sessions WHERE pk=?1",
                params![fork_pk],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            got_parent.as_deref(),
            Some(parent.as_uuid().as_bytes().as_slice())
        );
    }

    #[test]
    fn fork_depth_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));

        // create a chain of MAX_FORK_DEPTH forks
        let mut ancestors: Vec<SessionId> = Vec::new();
        let mut current = SessionId::new();
        ancestors.push(current);

        // create root session
        create_parent(&mut conn, current);
        append(&mut conn, current, 1, "root");

        // create MAX_FORK_DEPTH forks chained together
        for i in 0..MAX_FORK_DEPTH {
            let fork_id = SessionId::new();
            let result = ForkV2::fork_session(
                &mut conn,
                current.as_uuid().as_bytes(),
                1,
                fork_id,
                "forked",
                5000 + i as i64,
            );
            assert!(result.is_ok(), "fork {} should succeed", i);
            let fork_pk = result.unwrap();

            // verify fork has fork_parent_id set
            let is_fork: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sessions WHERE pk=?1 AND fork_parent_id IS NOT NULL",
                    params![fork_pk],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(is_fork, 1, "fork {} should have fork_parent_id", i);

            ancestors.push(fork_id);
            current = fork_id;
        }

        // attempt to fork at depth cap - should fail
        let overflow_fork = SessionId::new();
        let result = ForkV2::fork_session(
            &mut conn,
            current.as_uuid().as_bytes(),
            1,
            overflow_fork,
            "overflow",
            5000 + MAX_FORK_DEPTH as i64,
        );
        assert!(
            result.is_err(),
            "fork at depth {} should fail",
            MAX_FORK_DEPTH
        );
    }
}
