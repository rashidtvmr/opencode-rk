#![forbid(unsafe_code)]
//! HEAD-002 frozen RED tests: deterministic redacted session export.
//! Module under test lives at `src/session_export.rs` (owned by HEAD-002);
//! included by path so no shared lib.rs/Cargo.toml wiring is touched.

#[path = "../src/session_export.rs"]
mod session_export;

use session_export::{
    ExportError, ExportMessage, ExportOpts, ExportSession, ExportSink, ExportSummary, SessionStore,
    EXPORT_MAX_BYTES,
};
use std::cell::Cell;

struct FixtureStore {
    sessions: Vec<ExportSession>,
    fail: bool,
    calls: Cell<u64>,
}

impl FixtureStore {
    fn new(sessions: Vec<ExportSession>) -> Self {
        Self {
            sessions,
            fail: false,
            calls: Cell::new(0),
        }
    }
    fn failing() -> Self {
        Self {
            sessions: Vec::new(),
            fail: true,
            calls: Cell::new(0),
        }
    }
}

impl SessionStore for FixtureStore {
    fn load(&self, id: &str) -> Result<Option<ExportSession>, ExportError> {
        self.calls.set(self.calls.get() + 1);
        if self.fail {
            return Err(ExportError::StoreFailed);
        }
        Ok(self.sessions.iter().find(|s| s.id == id).cloned())
    }
}

struct VecSink {
    buf: Vec<u8>,
}

impl VecSink {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }
}

impl ExportSink for VecSink {
    fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }
}

fn opts() -> ExportOpts {
    ExportOpts { tty: false }
}

fn two_message_session() -> ExportSession {
    ExportSession {
        id: "s1".to_owned(),
        messages: vec![
            ExportMessage {
                role: "user".to_owned(),
                content: "hello".to_owned(),
                fields: Vec::new(),
            },
            ExportMessage {
                role: "assistant".to_owned(),
                content: "world".to_owned(),
                fields: Vec::new(),
            },
        ],
    }
}

// HEAD-002-T01 (happy path)
#[test]
fn head_002_t01_happy_path_writes_canonical_json_with_exact_summary() {
    let store = FixtureStore::new(vec![two_message_session()]);
    let mut sink = VecSink::new();
    let summary: ExportSummary =
        session_export::export_json("s1", &store, &mut sink, &opts()).expect("export s1");
    assert_eq!(summary.session_id, "s1");
    assert_eq!(summary.messages, 2);
    assert_eq!(summary.bytes, sink.buf.len() as u64);
    let text = String::from_utf8(sink.buf.clone()).expect("export must be valid UTF-8");
    assert!(text.contains("\"s1\""), "export must carry the session id");
    assert!(text.contains("hello"), "export must carry message content");
    assert!(text.contains("world"), "export must carry message content");
}

// HEAD-002-T02 (non-TTY missing id)
#[test]
fn head_002_t02_empty_id_non_tty_is_missing_id_with_zero_side_effects() {
    let store = FixtureStore::new(vec![two_message_session()]);
    let mut sink = VecSink::new();
    let err = session_export::export_json("", &store, &mut sink, &opts()).expect_err("empty id");
    assert!(
        matches!(err, ExportError::MissingId),
        "want MissingId, got {err:?}"
    );
    assert_eq!(store.calls.get(), 0, "no store call on missing id");
    assert_eq!(sink.buf.len(), 0, "no sink bytes on missing id");
    // TTY callers get the same typed error: picker owned by a later UI slice.
    let tty_err = session_export::export_json("", &store, &mut sink, &ExportOpts { tty: true })
        .expect_err("empty id on tty");
    assert!(matches!(tty_err, ExportError::MissingId));
    assert_eq!(store.calls.get(), 0);
}

// HEAD-002-T03 (oversize atomic)
#[test]
fn head_002_t03_oversize_export_fails_atomically_with_zero_sink_bytes() {
    assert_eq!(EXPORT_MAX_BYTES, 16_777_216);
    let big = "x".repeat(EXPORT_MAX_BYTES + 1);
    let store = FixtureStore::new(vec![ExportSession {
        id: "big".to_owned(),
        messages: vec![ExportMessage {
            role: "user".to_owned(),
            content: big,
            fields: Vec::new(),
        }],
    }]);
    let mut sink = VecSink::new();
    let err = session_export::export_json("big", &store, &mut sink, &opts()).expect_err("oversize");
    assert!(
        matches!(err, ExportError::TooLarge { .. }),
        "want TooLarge, got {err:?}"
    );
    assert_eq!(
        sink.buf.len(),
        0,
        "oversize export must emit zero bytes (atomic)"
    );
}

// HEAD-002-T04 (leak scan)
#[test]
fn head_002_t04_secret_shaped_values_redacted_in_output_and_errors() {
    let store = FixtureStore::new(vec![ExportSession {
        id: "s9".to_owned(),
        messages: vec![ExportMessage {
            role: "user".to_owned(),
            content: "sk-abc-value".to_owned(),
            fields: vec![
                ("password".to_owned(), "hunter2".to_owned()),
                ("api_key".to_owned(), "AKIA-FIXTURE".to_owned()),
                ("note".to_owned(), "bearer xyz-fixture".to_owned()),
            ],
        }],
    }]);
    let mut sink = VecSink::new();
    let summary = session_export::export_json("s9", &store, &mut sink, &opts()).expect("export");
    assert_eq!(summary.messages, 1);
    let text = String::from_utf8(sink.buf.clone()).expect("valid UTF-8");
    for leaked in [
        "sk-abc-value",
        "hunter2",
        "AKIA-FIXTURE",
        "bearer xyz-fixture",
    ] {
        assert!(
            !text.contains(leaked),
            "secret bytes must not appear: {leaked}"
        );
    }
    assert!(
        text.contains("\"***\""),
        "redacted values must render as ***"
    );
    // Error Debug/Display carry variant names and ids only, never secret content.
    for err in [
        ExportError::MissingId,
        ExportError::NotFound,
        ExportError::TooLarge { estimated: 1 },
        ExportError::StoreFailed,
        ExportError::InvalidId,
    ] {
        let rendered = format!("{err:?} | {err}");
        for leaked in ["sk-abc", "bearer xyz", "hunter2"] {
            assert!(
                !rendered.contains(leaked),
                "error must not leak secrets: {rendered}"
            );
        }
    }
}

// HEAD-002-T05 (deterministic bytes)
#[test]
fn head_002_t05_same_export_twice_is_byte_identical_with_stable_key_order() {
    let session = ExportSession {
        id: "s7".to_owned(),
        messages: vec![ExportMessage {
            role: "user".to_owned(),
            content: "deterministic".to_owned(),
            fields: vec![
                ("zeta".to_owned(), "1".to_owned()),
                ("alpha".to_owned(), "2".to_owned()),
            ],
        }],
    };
    let store = FixtureStore::new(vec![session]);
    let mut first = VecSink::new();
    let mut second = VecSink::new();
    session_export::export_json("s7", &store, &mut first, &opts()).expect("first");
    session_export::export_json("s7", &store, &mut second, &opts()).expect("second");
    assert_eq!(first.buf, second.buf, "exports must be byte-identical");
    let text = String::from_utf8(first.buf).expect("valid UTF-8");
    let zeta = text.find("zeta").expect("zeta key present");
    let alpha = text.find("alpha").expect("alpha key present");
    assert!(
        zeta < alpha,
        "first-seen key order must be stable, not sorted"
    );
}

// Unknown id: typed NotFound, sink untouched.
#[test]
fn head_002_unknown_id_is_not_found_with_untouched_sink() {
    let store = FixtureStore::new(vec![two_message_session()]);
    let mut sink = VecSink::new();
    sink.push(b"prior");
    let before = sink.buf.len();
    let err = session_export::export_json("nope", &store, &mut sink, &opts()).expect_err("unknown");
    assert!(matches!(err, ExportError::NotFound));
    assert_eq!(sink.buf.len(), before);
}

// Store failure: typed StoreFailed, sink untouched.
#[test]
fn head_002_store_failure_is_typed_with_untouched_sink() {
    let store = FixtureStore::failing();
    let mut sink = VecSink::new();
    let err = session_export::export_json("s1", &store, &mut sink, &opts()).expect_err("failing");
    assert!(matches!(err, ExportError::StoreFailed));
    assert_eq!(sink.buf.len(), 0);
}

// Overlong id (>128 chars): rejected before any store call.
#[test]
fn head_002_overlong_id_rejected_before_store_call() {
    let store = FixtureStore::new(vec![two_message_session()]);
    let mut sink = VecSink::new();
    let long = "s".repeat(129);
    let err = session_export::export_json(&long, &store, &mut sink, &opts()).expect_err("long");
    assert!(matches!(err, ExportError::InvalidId));
    assert_eq!(store.calls.get(), 0);
    assert_eq!(sink.buf.len(), 0);
}
