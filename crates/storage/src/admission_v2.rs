//! Admission and idempotency-dedup owner for the format-2 workspace.
//!
//! Owns `session_inputs` + `session_input_parts` (steer/queue admission with
//! promotion into `messages`, sharing payload references) and
//! `operation_receipts` (bounded dedup receipts with expiry).
//!
//! ponytail: errors reuse `invalid_input()` (Sqlite InvalidQuery) plus the two
//! shared size variants (`InlinePayloadTooLarge`, `EventPayloadTooLarge`)
//! instead of new StorageError variants. Upgrade path: add typed variants
//! when the error surface is versioned.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::StorageError;

const MAX_INLINE_PAYLOAD_BYTES: usize = 8192;
const MAX_INPUT_PARTS: usize = 256;
const MAX_PART_KIND: u8 = 15;
const MAX_RECEIPT_KIND_BYTES: usize = 128;
const MAX_RESULT_JSON_BYTES: usize = 4096;

pub struct AdmissionV2;

impl AdmissionV2 {
    /// Admit one input for a session. Allocates `seq` from
    /// `sessions.next_input_seq` and inserts the input (state 0) plus one
    /// inline payload + input part per entry, all in one IMMEDIATE tx.
    /// Returns `(input_pk, seq)`. Inline payloads over 8192 bytes are rejected.
    pub fn submit_input(
        connection: &mut Connection,
        session_id: &[u8],
        delivery: u8,
        request_hash: &[u8; 32],
        parts: &[(u8, Vec<u8>)],
    ) -> Result<(i64, i64), StorageError> {
        if session_id.len() != 16 || !matches!(delivery, 0 | 1) {
            return Err(invalid_input());
        }
        if parts.is_empty() || parts.len() > MAX_INPUT_PARTS {
            return Err(invalid_input());
        }
        for (kind, data) in parts {
            if *kind > MAX_PART_KIND {
                return Err(invalid_input());
            }
            if data.len() > MAX_INLINE_PAYLOAD_BYTES {
                return Err(StorageError::InlinePayloadTooLarge);
            }
        }

        let now_us = chrono::Utc::now().timestamp_micros();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (session_pk, sequence): (i64, i64) = transaction
            .query_row(
                "SELECT pk, next_input_seq FROM sessions WHERE id=?1",
                params![session_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(invalid_input)?;
        let next_sequence = sequence.checked_add(1).ok_or_else(invalid_input)?;
        transaction.execute(
            "UPDATE sessions SET next_input_seq=?1 WHERE pk=?2",
            params![next_sequence, session_pk],
        )?;
        transaction.execute(
            "INSERT INTO session_inputs
             (id, session_pk, seq, delivery, state, request_hash, created_at_us)
             VALUES (randomblob(16), ?1, ?2, ?3, 0, ?4, ?5)",
            params![
                session_pk,
                sequence,
                i64::from(delivery),
                &request_hash[..],
                now_us,
            ],
        )?;
        let input_pk = transaction.last_insert_rowid();
        for (ordinal, (kind, data)) in parts.iter().enumerate() {
            transaction.execute(
                "INSERT INTO payloads (inline_data, raw_bytes, created_at_us)
                 VALUES (?1, ?2, ?3)",
                params![data.as_slice(), data.len() as i64, now_us],
            )?;
            let payload_pk = transaction.last_insert_rowid();
            transaction.execute(
                "INSERT INTO session_input_parts (input_pk, ordinal, kind, payload_pk)
                 VALUES (?1, ?2, ?3, ?4)",
                params![input_pk, ordinal as i64, i64::from(*kind), payload_pk],
            )?;
        }
        transaction.commit()?;
        Ok((input_pk, sequence))
    }

    /// Promote a pending input into a user message. The message takes a fresh
    /// `seq` from `sessions.next_message_seq`, reuses the input's payload_pk
    /// references (bodies shared, never copied), drops the input parts, and
    /// marks the input promoted, all in ONE tx. Returns the message pk.
    pub fn promote_input(connection: &mut Connection, input_pk: i64) -> Result<i64, StorageError> {
        let now_us = chrono::Utc::now().timestamp_micros();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let session_pk: i64 = {
            let (pk, state): (i64, i64) = transaction
                .query_row(
                    "SELECT session_pk, state FROM session_inputs WHERE pk=?1",
                    params![input_pk],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?
                .ok_or_else(invalid_input)?;
            if state != 0 {
                return Err(invalid_input());
            }
            pk
        };

        let parts: Vec<(i64, i64)> = {
            let mut statement = transaction.prepare(
                "SELECT ordinal, kind, payload_pk FROM session_input_parts
                 WHERE input_pk=?1 ORDER BY ordinal ASC",
            )?;
            let mapped = statement.query_map(params![input_pk], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;
            let mut out = Vec::new();
            for part in mapped {
                let row = part?;
                out.push((row.1, row.2));
            }
            out
        };
        if parts.is_empty() {
            return Err(invalid_input());
        }

        let message_seq: i64 = transaction
            .query_row(
                "SELECT next_message_seq FROM sessions WHERE pk=?1",
                params![session_pk],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(invalid_input)?;
        let next_message_seq = message_seq.checked_add(1).ok_or_else(invalid_input)?;
        transaction.execute(
            "UPDATE sessions SET next_message_seq=?1 WHERE pk=?2",
            params![next_message_seq, session_pk],
        )?;
        transaction.execute(
            "INSERT INTO messages
             (id, session_pk, seq, role, status, created_at_us, completed_at_us)
             VALUES (randomblob(16), ?1, ?2, 1, 1, ?3, ?3)",
            params![session_pk, message_seq, now_us],
        )?;
        let message_pk = transaction.last_insert_rowid();
        for (ordinal, (kind, payload_pk)) in parts.iter().enumerate() {
            transaction.execute(
                "INSERT INTO message_parts (message_pk, ordinal, kind, payload_pk)
                 VALUES (?1, ?2, ?3, ?4)",
                params![message_pk, ordinal as i64, kind, payload_pk],
            )?;
        }
        transaction.execute(
            "DELETE FROM session_input_parts WHERE input_pk=?1",
            params![input_pk],
        )?;
        let changed = transaction.execute(
            "UPDATE session_inputs
             SET state=1, promoted_message_pk=?1, promoted_at_us=?2
             WHERE pk=?3 AND state=0",
            params![message_pk, now_us, input_pk],
        )?;
        if changed != 1 {
            return Err(invalid_input());
        }
        transaction.commit()?;
        Ok(message_pk)
    }

    /// Look up a dedup receipt. Returns None when the operation is unknown
    /// OR its `retry_until_us` has passed. Returns `(request_hash, result_json)`.
    pub fn receipt_lookup(
        connection: &Connection,
        operation_id: &[u8],
    ) -> Result<Option<(Vec<u8>, String)>, StorageError> {
        if operation_id.len() != 16 {
            return Err(invalid_input());
        }
        let row: Option<(Vec<u8>, String, i64)> = connection
            .query_row(
                "SELECT request_hash, result_json, retry_until_us
                 FROM operation_receipts WHERE operation_id=?1",
                params![operation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((request_hash, result_json, retry_until_us)) = row else {
            return Ok(None);
        };
        if retry_until_us <= chrono::Utc::now().timestamp_micros() {
            return Ok(None);
        }
        Ok(Some((request_hash, result_json)))
    }

    /// Store a dedup receipt. Rejects bad kind length, invalid or oversize
    /// JSON, and `retry_until_us <= created_us`.
    pub fn receipt_store(
        connection: &Connection,
        operation_id: &[u8],
        request_hash: &[u8; 32],
        kind: &str,
        result_json: &str,
        created_us: i64,
        retry_until_us: i64,
    ) -> Result<(), StorageError> {
        if operation_id.len() != 16 {
            return Err(invalid_input());
        }
        if !(1..=MAX_RECEIPT_KIND_BYTES).contains(&kind.len()) {
            return Err(invalid_input());
        }
        if result_json.len() > MAX_RESULT_JSON_BYTES {
            return Err(StorageError::EventPayloadTooLarge);
        }
        serde_json::from_str::<serde_json::Value>(result_json).map_err(|_| invalid_input())?;
        if retry_until_us <= created_us {
            return Err(invalid_input());
        }
        connection.execute(
            "INSERT INTO operation_receipts
             (operation_id, request_hash, kind, result_json, created_at_us, retry_until_us)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                operation_id,
                &request_hash[..],
                kind,
                result_json,
                created_us,
                retry_until_us,
            ],
        )?;
        Ok(())
    }
}

fn invalid_input() -> StorageError {
    StorageError::Sqlite(rusqlite::Error::InvalidQuery)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::SessionId;
    use std::path::Path;

    fn workspace(path: &Path) -> Connection {
        crate::schema_v2::SchemaV2::initialize_workspace(path, [1_u8; 16], [2_u8; 16], 10).unwrap()
    }

    fn session_bytes(connection: &mut Connection) -> Vec<u8> {
        let id = SessionId::new();
        crate::writer_v2::V2Writer::create_session(
            connection,
            &crate::writer_v2::NewSession {
                id,
                title: "admission".to_owned(),
                created_at_us: 100,
                updated_at_us: 100,
            },
        )
        .unwrap();
        id.as_uuid().as_bytes().to_vec()
    }

    fn submit(connection: &mut Connection, session: &[u8], body: &[u8]) -> (i64, i64) {
        AdmissionV2::submit_input(connection, session, 0, &[7_u8; 32], &[(0, body.to_vec())])
            .unwrap()
    }

    #[test]
    fn seq_allocates_1_then_2() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let session = session_bytes(&mut conn);
        let (pk1, seq1) = submit(&mut conn, &session, b"first");
        let (pk2, seq2) = submit(&mut conn, &session, b"second");
        assert_eq!((seq1, seq2), (1, 2));
        assert_ne!(pk1, pk2);
        let next: i64 = conn
            .query_row(
                "SELECT next_input_seq FROM sessions WHERE id=?1",
                params![session.as_slice()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next, 3);
    }

    #[test]
    fn promote_shares_payload_pk_and_marks_input() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let session = session_bytes(&mut conn);
        let (input_pk, _) = submit(&mut conn, &session, b"hello");
        let before: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT payload_pk FROM session_input_parts WHERE input_pk=?1")
                .unwrap();
            stmt.query_map(params![input_pk], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(before.len(), 1);

        let message_pk = AdmissionV2::promote_input(&mut conn, input_pk).unwrap();

        let after: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT payload_pk FROM message_parts WHERE message_pk=?1")
                .unwrap();
            stmt.query_map(params![message_pk], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(after, before); // shared payload refs, bodies never copied
        let leftover: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM session_input_parts WHERE input_pk=?1",
                params![input_pk],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(leftover, 0);
        let (state, promoted, at): (i64, Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT state, promoted_message_pk, promoted_at_us
                 FROM session_inputs WHERE pk=?1",
                params![input_pk],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(state, 1);
        assert_eq!(promoted, Some(message_pk));
        assert!(at.is_some());
        let (seq, role): (i64, i64) = conn
            .query_row(
                "SELECT seq, role FROM messages WHERE pk=?1",
                params![message_pk],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((seq, role), (1, 1));
    }

    #[test]
    fn promote_twice_or_missing_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let session = session_bytes(&mut conn);
        let (input_pk, _) = submit(&mut conn, &session, b"x");
        AdmissionV2::promote_input(&mut conn, input_pk).unwrap();
        assert!(AdmissionV2::promote_input(&mut conn, input_pk).is_err());
        assert!(AdmissionV2::promote_input(&mut conn, 999_999).is_err());
    }

    #[test]
    fn receipt_roundtrip_ok() {
        let dir = tempfile::tempdir().unwrap();
        let conn = workspace(&dir.path().join("w.db"));
        let op = [3_u8; 16];
        let now = chrono::Utc::now().timestamp_micros();
        AdmissionV2::receipt_store(
            &conn,
            &op,
            &[9_u8; 32],
            "rename",
            r#"{"ok":true}"#,
            now - 1000,
            now + 1_000_000,
        )
        .unwrap();
        let got = AdmissionV2::receipt_lookup(&conn, &op).unwrap().unwrap();
        assert_eq!(got.0, vec![9_u8; 32]);
        assert_eq!(got.1, r#"{"ok":true}"#.to_owned());
    }

    #[test]
    fn expired_receipt_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let conn = workspace(&dir.path().join("w.db"));
        let op = [4_u8; 16];
        // both stamps long past: valid at write time, expired at lookup time
        AdmissionV2::receipt_store(&conn, &op, &[1_u8; 32], "k", "{}", 1000, 2000).unwrap();
        assert!(AdmissionV2::receipt_lookup(&conn, &op).unwrap().is_none());
        // unknown operation is also None
        assert!(AdmissionV2::receipt_lookup(&conn, &[5_u8; 16])
            .unwrap()
            .is_none());
    }

    #[test]
    fn oversize_inline_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let session = session_bytes(&mut conn);
        let big = vec![b'x'; 8193];
        let err = AdmissionV2::submit_input(&mut conn, &session, 0, &[1_u8; 32], &[(0, big)])
            .unwrap_err();
        assert!(matches!(err, StorageError::InlinePayloadTooLarge));
        // boundary size passes
        let ok = vec![b'x'; 8192];
        let (_, seq) =
            AdmissionV2::submit_input(&mut conn, &session, 0, &[1_u8; 32], &[(0, ok)]).unwrap();
        assert_eq!(seq, 1);
    }

    #[test]
    fn submit_validation_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let session = session_bytes(&mut conn);
        // bad delivery
        assert!(AdmissionV2::submit_input(
            &mut conn,
            &session,
            2,
            &[1_u8; 32],
            &[(0, b"x".to_vec())]
        )
        .is_err());
        // bad part kind (>15)
        assert!(AdmissionV2::submit_input(
            &mut conn,
            &session,
            0,
            &[1_u8; 32],
            &[(16, b"x".to_vec())]
        )
        .is_err());
        // empty parts
        assert!(AdmissionV2::submit_input(&mut conn, &session, 0, &[1_u8; 32], &[]).is_err());
        // unknown session
        assert!(AdmissionV2::submit_input(
            &mut conn,
            &[0_u8; 16],
            0,
            &[1_u8; 32],
            &[(0, b"x".to_vec())]
        )
        .is_err());
    }

    #[test]
    fn receipt_validation_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let conn = workspace(&dir.path().join("w.db"));
        let op = [6_u8; 16];
        // empty kind
        assert!(AdmissionV2::receipt_store(&conn, &op, &[1_u8; 32], "", "{}", 1, 2).is_err());
        // invalid JSON
        assert!(AdmissionV2::receipt_store(&conn, &op, &[1_u8; 32], "k", "{bad", 1, 2).is_err());
        // oversize JSON
        let big = "x".repeat(4097);
        assert!(AdmissionV2::receipt_store(&conn, &op, &[1_u8; 32], "k", &big, 1, 2).is_err());
        // retry must exceed created
        assert!(AdmissionV2::receipt_store(&conn, &op, &[1_u8; 32], "k", "{}", 5, 5).is_err());
        // bad operation id length
        assert!(
            AdmissionV2::receipt_store(&conn, &[1_u8; 8], &[1_u8; 32], "k", "{}", 1, 2).is_err()
        );
        assert!(AdmissionV2::receipt_lookup(&conn, &[1_u8; 8]).is_err());
    }
}
