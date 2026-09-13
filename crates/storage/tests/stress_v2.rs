//! Concurrency stress for the format-2 schema: seq allocation atomicity and the
//! executions single-owner partial UNIQUE under threads.
//!
//! Threads each open their OWN connection to the SAME file DB via
//! SchemaV2::open_existing (busy_timeout=5000, WAL). &mut Connection is not
//! Sync, so writer access is per-thread; SQLite busy_timeout + IMMEDIATE
//! transactions (plus the partial UNIQUE index) provide serialization.
//!
//! The BUSY retry loop below is TEST-ONLY insurance on top of the blocking
//! busy_timeout, mirroring the writer's deferral policy without shipping a
//! hidden runtime retry path.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use opencode_rk_contracts::{MessageId, MessageRole, PayloadRef, SessionId};
use opencode_rk_storage::schema_v2::SchemaV2;
use opencode_rk_storage::writer_v2::{NewMessage, NewSession, V2Writer};
use rusqlite::{params, Connection, ErrorCode};
use tempfile::TempDir;

/// Test-only retry over SQLITE_BUSY / SQLITE_LOCKED. The real writer relies on
/// the blocking busy_timeout=5000 PRAGMA; this loop only re-arms a transient
/// lock that could otherwise escape a fast race window.
fn retry_busy<T>(mut f: impl FnMut() -> Result<T, rusqlite::Error>) -> Result<T, rusqlite::Error> {
    let mut attempts = 0usize;
    loop {
        match f() {
            Ok(v) => return Ok(v),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if (e.code == ErrorCode::DatabaseBusy || e.code == ErrorCode::DatabaseLocked)
                    && attempts < 50 =>
            {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(e) => return Err(e),
        }
    }
}

fn initialized() -> (TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap();
    (directory, path)
}

fn new_session(id: SessionId, updated_at_us: i64) -> NewSession {
    NewSession {
        id,
        title: "stress".into(),
        created_at_us: updated_at_us,
        updated_at_us,
    }
}

fn new_message(session_id: SessionId, id: MessageId, text: &str) -> NewMessage {
    NewMessage {
        id,
        session_id,
        role: MessageRole::User,
        body: PayloadRef::Inline {
            text: text.to_owned(),
        },
        created_at_us: 10,
    }
}

#[test]
fn concurrent_outbox_seq_unique() {
    let (_dir, path) = initialized();

    // Seed one session so events carry a session_id.
    {
        let mut conn = SchemaV2::open_existing(&path).unwrap();
        V2Writer::create_session(&mut conn, &new_session(SessionId::new(), 1)).unwrap();
    }

    const THREADS: usize = 8;
    const PER_THREAD: usize = 25;
    let seqs: Arc<Mutex<Vec<i64>>> = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let seqs = Arc::clone(&seqs);
            let path = path.clone();
            std::thread::spawn(move || {
                let mut conn = SchemaV2::open_existing(&path).unwrap();
                for _ in 0..PER_THREAD {
                    V2Writer::append_outbox_event(
                        &mut conn,
                        SessionId::new(),
                        "evt",
                        "{}",
                    )
                    .unwrap();
                }
                // Drain the event head from this connection.
                let head: i64 = conn
                    .query_row("SELECT event_head_seq FROM workspace_state WHERE id=1", [], |r| {
                        r.get(0)
                    })
                    .unwrap();
                seqs.lock().unwrap().push(head);
                drop(t); // silence unused warning; t is unique per thread
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    // 200 distinct outbox seqs; read them all back.
    let mut conn = SchemaV2::open_existing(&path).unwrap();
    let mut stmt = conn
        .prepare("SELECT seq FROM event_outbox ORDER BY seq")
        .unwrap();
    let rows: Vec<i64> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows.len(), THREADS * PER_THREAD, "event count");
    let mut unique = rows.clone();
    unique.dedup();
    assert_eq!(rows.len(), unique.len(), "seqs must be unique");
    assert_eq!(rows[0], 1, "contiguous from 1");
    assert_eq!(rows[rows.len() - 1], (THREADS * PER_THREAD) as i64, "contiguous to N");

    // head == 200 (workspace_state event_head_seq mirrors the count).
    let head: i64 = conn
        .query_row("SELECT event_head_seq FROM workspace_state WHERE id=1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(head, (THREADS * PER_THREAD) as i64);
}

#[test]
fn concurrent_message_seq_per_session() {
    let (_dir, path) = initialized();
    let session_id = SessionId::new();
    {
        let mut conn = SchemaV2::open_existing(&path).unwrap();
        V2Writer::create_session(&mut conn, &new_session(session_id, 1)).unwrap();
    }

    const THREADS: usize = 4;
    const PER_THREAD: usize = 25;
    let seqs: Arc<Mutex<Vec<i64>>> = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let seqs = Arc::clone(&seqs);
            let path = path.clone();
            std::thread::spawn(move || {
                let mut conn = SchemaV2::open_existing(&path).unwrap();
                for _ in 0..PER_THREAD {
                    V2Writer::append_message(
                        &mut conn,
                        &new_message(session_id, MessageId::new(), "hi"),
                    )
                    .unwrap();
                }
                let next: i64 = conn
                    .query_row(
                        "SELECT next_message_seq FROM sessions WHERE id=?1",
                        params![session_id.as_uuid().as_bytes().as_slice()],
                        |r| r.get(0),
                    )
                    .unwrap();
                seqs.lock().unwrap().push(next);
                drop(t);
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    let mut conn = SchemaV2::open_existing(&path).unwrap();
    let rows: Vec<i64> = {
        let mut stmt = conn
            .prepare(
                "SELECT seq FROM messages WHERE session_pk=(SELECT pk FROM sessions WHERE id=?1) ORDER BY seq",
            )
            .unwrap();
        stmt.query_map(params![session_id.as_uuid().as_bytes().as_slice()], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(rows.len(), THREADS * PER_THREAD, "message count");
    let mut unique = rows.clone();
    unique.dedup();
    assert_eq!(rows.len(), unique.len(), "per-session seqs must be unique");
    assert_eq!(rows[0], 1, "contiguous from 1");
    assert_eq!(rows[rows.len() - 1], (THREADS * PER_THREAD) as i64, "contiguous to N");

    // next_message_seq advanced past the last allocated seq.
    let next: i64 = conn
        .query_row(
            "SELECT next_message_seq FROM sessions WHERE id=?1",
            params![session_id.as_uuid().as_bytes().as_slice()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(next, (THREADS * PER_THREAD) as i64 + 1);
}

#[test]
fn single_owner_race() {
    let (_dir, path) = initialized();
    let session_id = SessionId::new();

    // Seed a session and one config payload (shared by all racers). Mirror the
    // writer's row shapes directly since ExecV2 does not exist yet.
    let config_payload_pk: i64 = {
        let mut conn = SchemaV2::open_existing(&path).unwrap();
        V2Writer::create_session(&mut conn, &new_session(session_id, 1)).unwrap();
        conn.execute(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, ?2, ?3)",
            params![b"cfg".as_slice(), 3i64, 10i64],
        )
        .unwrap();
        conn.last_insert_rowid()
    };

    const THREADS: usize = 8;
    let wins = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let wins = Arc::clone(&wins);
            let path = path.clone();
            std::thread::spawn(move || {
                let mut conn = SchemaV2::open_existing(&path).unwrap();
                // Fresh blob PK for each racer's id field; UNIQUE on id.
                let id = [t as u8 + 1; 16];
                let session_pk: i64 = conn
                    .query_row(
                        "SELECT pk FROM sessions WHERE id=?1",
                        params![session_id.as_uuid().as_bytes().as_slice()],
                        |r| r.get(0),
                    )
                    .unwrap();
                // No BUSY retry needed for a definitive UNIQUE result: a
                // constraint violation is a final outcome, not a transient lock.
                let res = retry_busy(|| {
                    conn.execute(
                        "INSERT INTO executions (id, session_pk, mode, state, owner_generation, config_payload_pk, provider_id, model_id, created_at_us) VALUES (?1,?2,0,0,0,?3,'p','m',10)",
                        params![id.as_slice(), session_pk, config_payload_pk],
                    )
                });
                if res.is_ok() {
                    wins.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    // The partial UNIQUE executions_single_owner_idx permits exactly one
    // queued/running/uncertain execution per session.
    assert_eq!(
        wins.load(Ordering::SeqCst),
        1,
        "single-owner guard must admit exactly one winner"
    );

    let mut conn = SchemaV2::open_existing(&path).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM executions WHERE session_pk=(SELECT pk FROM sessions WHERE id=?1) AND state=0",
            params![session_id.as_uuid().as_bytes().as_slice()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "exactly one owning execution row persisted");
}

#[test]
fn busy_timeout_survives_contention() {
    let (_dir, path) = initialized();
    let session_id = SessionId::new();
    {
        let mut conn = SchemaV2::open_existing(&path).unwrap();
        V2Writer::create_session(&mut conn, &new_session(session_id, 1)).unwrap();
    }

    // Churn the same per-session seq counter from many threads; any SQLITE_BUSY
    // that escapes the blocking busy_timeout would surface as a panic.
    const THREADS: usize = 8;
    const PER_THREAD: usize = 12;
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut conn = SchemaV2::open_existing(&path).unwrap();
                for _ in 0..PER_THREAD {
                    V2Writer::append_message(
                        &mut conn,
                        &new_message(session_id, MessageId::new(), "x"),
                    )
                    .unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap(); // any leaked BUSY would panic here
    }

    let mut conn = SchemaV2::open_existing(&path).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM messages WHERE session_pk=(SELECT pk FROM sessions WHERE id=?1)",
            params![session_id.as_uuid().as_bytes().as_slice()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, (THREADS * PER_THREAD) as i64, "all writes landed");
}
