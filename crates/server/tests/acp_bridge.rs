//! ACP-001 RED: ACP v1 JSON-lines bridge over stdio (codec + session map).
//! Pure route-layer boundary: decode one frame per line, encode one line per
//! response, in-memory caller-owned session map. Fails until acp_bridge exists.
#[path = "../src/acp_bridge.rs"]
mod acp_bridge;

use acp_bridge::{
    decode_frame, encode_frame, encode_frame_into, models_body, modes_body, session_load,
    session_new, session_prompt, variants_body, AcpErrorCode, AcpRequest, AcpResponse, AcpSessions,
    MAX_ACP_FRAME_BYTES,
};
use std::cell::Cell;

fn code_of(result: Result<AcpRequest, acp_bridge::AcpError>) -> AcpErrorCode {
    result.unwrap_err().code
}

#[test]
fn acp001_t01_happy_path_decode_session_round_trip_encode() {
    assert_eq!(
        decode_frame(br#"{"method":"initialize"}"#).expect("initialize decodes"),
        AcpRequest::Initialize
    );
    assert_eq!(
        decode_frame(br#"{"method":"session/new"}"#).expect("session/new decodes"),
        AcpRequest::SessionNew
    );

    let mut sessions = AcpSessions::new();
    let id = session_new(&mut sessions).expect("new mints").id.clone();
    let loaded = session_load(&sessions, &id).expect("load resolves");
    assert_eq!(loaded.id, id);
    session_prompt(&mut sessions, &id, "hello").expect("prompt records");
    assert_eq!(sessions.get(&id).expect("present").prompts, vec!["hello"]);

    // Prompt lands against that session only.
    let other = session_new(&mut sessions).expect("second mints").id.clone();
    assert_ne!(id, other);
    assert!(sessions.get(&other).expect("present").prompts.is_empty());

    let line = encode_frame(&AcpResponse::Ok {
        body: "done".to_owned(),
    })
    .expect("ok encodes");
    assert!(line.ends_with(b"\n"));
    assert_eq!(line.iter().filter(|b| **b == b'\n').count(), 1);
    // Original request bytes re-decode to the same kind.
    assert_eq!(
        decode_frame(br#"{"method":"initialize"}"#).expect("re-decodes"),
        AcpRequest::Initialize
    );
}

#[test]
fn acp001_t02_modes_models_variants_list_fixture_entries_in_order() {
    assert_eq!(
        decode_frame(br#"{"method":"modes"}"#).expect("modes decodes"),
        AcpRequest::GetModes
    );
    assert_eq!(
        decode_frame(br#"{"method":"models"}"#).expect("models decodes"),
        AcpRequest::GetModels
    );
    assert_eq!(
        decode_frame(br#"{"method":"variants"}"#).expect("variants decodes"),
        AcpRequest::GetVariants
    );

    assert_eq!(modes_body(), r#"["default","read-only","plan"]"#);
    assert_eq!(models_body(), r#"["fixture-llm-a","fixture-llm-b"]"#);
    assert_eq!(variants_body(), r#"["default"]"#);

    for body in [modes_body(), models_body(), variants_body()] {
        let line = encode_frame(&AcpResponse::Ok { body }).expect("ok encodes");
        assert!(line.ends_with(b"\n"));
        assert_eq!(line.iter().filter(|b| **b == b'\n').count(), 1);
        let value: serde_json::Value =
            serde_json::from_slice(&line[..line.len() - 1]).expect("single JSON object");
        assert_eq!(value["status"], "ok");
    }
}

#[test]
fn acp001_t03_framing_rejects_leave_map_untouched() {
    let mut sessions = AcpSessions::new();
    let id = session_new(&mut sessions).expect("new mints").id.clone();
    let before = sessions.clone();

    let big = vec![b'{'; MAX_ACP_FRAME_BYTES + 1];
    assert_eq!(code_of(decode_frame(&big)), AcpErrorCode::TooLarge);
    assert_eq!(sessions, before);

    assert_eq!(
        code_of(decode_frame(b"\xff\xfe{\"method\":\"modes\"}")),
        AcpErrorCode::InvalidUtf8
    );
    assert_eq!(sessions, before);

    assert_eq!(
        code_of(decode_frame(br#"{"method":"teleport"}"#)),
        AcpErrorCode::UnknownMethod
    );
    assert_eq!(sessions, before);

    // Multi-object line rejected.
    assert_eq!(
        code_of(decode_frame(br#"{"method":"modes"} {"method":"models"}"#)),
        AcpErrorCode::BadFrame
    );
    assert_eq!(sessions, before);

    // Malformed JSON rejected.
    assert_eq!(code_of(decode_frame(b"{nope")), AcpErrorCode::BadFrame);
    assert_eq!(sessions, before);

    assert_eq!(session_load(&sessions, &id).expect("untouched").id, id);
}

#[test]
fn acp001_t04_session_failures_and_no_partial_emit() {
    let mut sessions = AcpSessions::new();
    let id = session_new(&mut sessions).expect("new mints").id.clone();
    let before = sessions.clone();

    assert_eq!(
        session_load(&sessions, "ses_missing").unwrap_err().code,
        AcpErrorCode::UnknownSession
    );
    assert_eq!(sessions, before);
    assert_eq!(
        session_prompt(&mut sessions, "ses_missing", "hi")
            .unwrap_err()
            .code,
        AcpErrorCode::UnknownSession
    );
    assert_eq!(sessions, before);

    assert_eq!(
        session_prompt(&mut sessions, &id, "").unwrap_err().code,
        AcpErrorCode::InvalidInput
    );
    assert!(sessions.get(&id).expect("present").prompts.is_empty());

    let huge = "x".repeat(MAX_ACP_FRAME_BYTES + 1);
    let mut emitted = Vec::from(b"sentinel".as_slice());
    let err = encode_frame_into(&AcpResponse::Ok { body: huge }, &mut emitted).unwrap_err();
    assert_eq!(err.code, AcpErrorCode::TooLarge);
    assert_eq!(emitted, b"sentinel", "zero bytes emitted on oversize");
    assert!(encode_frame(&AcpResponse::Ok {
        body: "y".repeat(MAX_ACP_FRAME_BYTES + 1)
    })
    .is_err());
}

#[test]
fn acp001_t05_purity_and_safety_no_io_no_secret_leak() {
    let fs_calls = Cell::new(0usize);
    let net_calls = Cell::new(0usize);
    let spawn_calls = Cell::new(0usize);

    let dir = tempfile::tempdir().expect("disposable fixture");
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, b"sentinel").expect("sentinel fixture");

    // Full matrix over caller bytes only; harness performs no I/O itself.
    let mut sessions = AcpSessions::new();
    let mut log = String::new();
    let secret_frame =
        br#"{"method":"session/prompt","session_id":"ses_SECRETABC","text":"SECRET-TEXT-xyz"}"#;
    for frame in [
        br#"{"method":"initialize"}"#.as_slice(),
        br#"{"method":"session/new"}"#.as_slice(),
        br#"{"method":"modes"}"#.as_slice(),
        br#"{"method":"models"}"#.as_slice(),
        br#"{"method":"variants"}"#.as_slice(),
        secret_frame.as_slice(),
        b"{broken",
        br#"{"method":"nope"}"#,
    ] {
        match decode_frame(frame) {
            Ok(_) => log.push_str("decode_ok\n"),
            Err(err) => log.push_str(&format!("decode_failed err={err}\n")),
        }
    }
    let id = session_new(&mut sessions).expect("new mints").id.clone();
    session_prompt(&mut sessions, &id, "SECRET-TEXT-xyz").expect("prompt records");
    if let Err(err) = session_load(&sessions, "ses_nope") {
        log.push_str(&format!("load_failed err={err}\n"));
    }
    let line = encode_frame(&AcpResponse::Error {
        code: AcpErrorCode::BadFrame,
    })
    .expect("error encodes");
    log.push_str(&format!("emitted={}", String::from_utf8_lossy(&line)));

    // Pure by construction: module source touches no I/O or spawn surface.
    let source = include_str!("../src/acp_bridge.rs");
    for banned in [
        "std::fs",
        "std::net",
        "std::process",
        "Command",
        "tokio",
        "unsafe",
        "std::env",
        "println",
    ] {
        assert!(!source.contains(banned), "banned surface: {banned}");
    }
    assert_eq!(fs_calls.get(), 0);
    assert_eq!(net_calls.get(), 0);
    assert_eq!(spawn_calls.get(), 0);

    // Errors carry codes only, never frame content.
    assert!(!log.contains("SECRETABC"));
    assert!(!log.contains("SECRET-TEXT-xyz"));
    assert!(!log.contains("ses_SECRETABC"));

    // Fixture dir untouched apart from the sentinel.
    assert_eq!(std::fs::read(&sentinel).expect("reread"), b"sentinel");
    let entries: Vec<_> = std::fs::read_dir(dir.path()).expect("list").collect();
    assert_eq!(entries.len(), 1);
}
