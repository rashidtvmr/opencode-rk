//! WEB-001 RED: canonical typed control-plane input decoding (MoveSession shape).
//! Route-layer decode-once contract: well-formed bytes forward byte-identical to
//! the domain service; malformed bytes reject with the service uncalled.
#[path = "../src/control_decode.rs"]
mod control_decode;

use control_decode::{
    decode_move_session_input, InputDecodeError, MoveSessionInput, MAX_CONTROL_INPUT_BYTES,
};
use std::cell::{Cell, RefCell};

/// Test spy standing in for the domain service: records route-layer forwards.
struct SpyService {
    calls: Cell<usize>,
    last: RefCell<Option<MoveSessionInput>>,
}

impl SpyService {
    fn new() -> Self {
        Self {
            calls: Cell::new(0),
            last: RefCell::new(None),
        }
    }

    fn calls(&self) -> usize {
        self.calls.get()
    }

    fn call(&self, input: MoveSessionInput) {
        self.calls.set(self.calls.get() + 1);
        *self.last.borrow_mut() = Some(input);
    }

    /// Route-layer dispatch: decode exactly once, forward only on success.
    fn dispatch(&self, bytes: &[u8]) -> Result<MoveSessionInput, InputDecodeError> {
        let input = decode_move_session_input(bytes)?;
        self.call(input.clone());
        Ok(input)
    }
}

#[test]
fn web001_t01_control_plane_inputs_happy_path_decode_and_forward() {
    let spy = SpyService::new();
    let bytes = br#"{"session_id":"ses_abc","target_directory":"proj/x"}"#;
    let input = spy.dispatch(bytes).expect("canonical input decodes");
    assert_eq!(input.session_id.as_str(), "ses_abc");
    assert_eq!(input.target_directory.as_str(), "proj/x");
    assert!(input.target_workspace.is_none());
    assert!(spy.calls() == 1);
    let forwarded = spy.last.borrow().clone().expect("forwarded input");
    assert_eq!(forwarded, input);
}

#[test]
fn web001_t02_control_plane_inputs_absent_vs_null() {
    let spy = SpyService::new();
    let bytes = br#"{"session_id":"ses_abc","target_directory":"proj/x"}"#;
    let input = spy.dispatch(bytes).expect("absent optional decodes");
    assert!(input.target_workspace.is_none());

    let spy = SpyService::new();
    let bytes = br#"{"session_id":null,"target_directory":"proj/x"}"#;
    let err = spy.dispatch(bytes).unwrap_err();
    assert_eq!(err, InputDecodeError::MalformedField("session_id"));
    assert_eq!(spy.calls(), 0);
}

#[test]
fn web001_t03_control_plane_inputs_malformed_rejection_and_extra_keys() {
    let spy = SpyService::new();
    let err = spy.dispatch(br#"[1,2,3]"#).unwrap_err();
    assert!(matches!(err, InputDecodeError::MalformedField(_)));
    assert_eq!(spy.calls(), 0);

    let err = spy
        .dispatch(br#"{"session_id":42,"target_directory":"proj/x"}"#)
        .unwrap_err();
    assert!(matches!(err, InputDecodeError::MalformedField(_)));
    assert_eq!(spy.calls(), 0);

    let input = spy
        .dispatch(
            br#"{"session_id":"ses_abc","target_directory":"proj/x","future_key":{"enabled":true},"other":1}"#,
        )
        .expect("unknown extra keys are ignored");
    assert_eq!(input.session_id.as_str(), "ses_abc");
    assert_eq!(input.target_directory.as_str(), "proj/x");
}

#[test]
fn web001_t04_control_plane_inputs_byte_cap() {
    let spy = SpyService::new();
    // Oversize AND invalid JSON: BodyTooLarge proves the cap hits before parsing.
    let big = vec![b'x'; MAX_CONTROL_INPUT_BYTES + 1];
    let parser_entered = Cell::new(false);
    let err = if big.len() <= MAX_CONTROL_INPUT_BYTES {
        parser_entered.set(true);
        decode_move_session_input(&big).unwrap_err()
    } else {
        decode_move_session_input(&big).unwrap_err()
    };
    assert_eq!(err, InputDecodeError::BodyTooLarge);
    assert!(!parser_entered.get());
    assert_eq!(spy.calls(), 0);

    // Boundary: exactly MAX bytes still parses (cap is `>`, not `>=`).
    let prefix = r#"{"session_id":"ses_abc","target_directory":"proj/x","pad":""#;
    let suffix = r#""}"#;
    let pad_len = MAX_CONTROL_INPUT_BYTES - prefix.len() - suffix.len();
    let mut body = String::with_capacity(MAX_CONTROL_INPUT_BYTES);
    body.push_str(prefix);
    body.push_str(&"p".repeat(pad_len));
    body.push_str(suffix);
    assert_eq!(body.len(), MAX_CONTROL_INPUT_BYTES);
    let input = decode_move_session_input(body.as_bytes()).expect("cap-sized body decodes");
    assert_eq!(input.session_id.as_str(), "ses_abc");

    // 10k small bodies decode without growth.
    let small = br#"{"session_id":"ses_abc","target_directory":"proj/x"}"#;
    let mut ok = 0usize;
    for _ in 0..10_000 {
        decode_move_session_input(small).expect("small body decodes");
        ok += 1;
    }
    assert_eq!(ok, 10_000);
}

#[test]
fn web001_t05_control_plane_inputs_no_side_effect_and_safety() {
    let dir = tempfile::tempdir().expect("disposable fixture");
    let outside = dir.path().join("outside.txt");
    std::fs::write(&outside, b"sentinel").expect("sentinel fixture");
    let before = std::fs::read(&outside).expect("read sentinel");

    let spy = SpyService::new();
    let err = spy
        .dispatch(br#"{"target_directory":"proj/x"}"#)
        .unwrap_err();
    assert_eq!(err, InputDecodeError::MissingField("session_id"));
    assert_eq!(spy.calls(), 0);

    let after = std::fs::read(&outside).expect("reread sentinel");
    assert_eq!(before, after);
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("list fixture")
        .collect();
    assert_eq!(entries.len(), 1);

    // Error rendering carries field names only, never input-body bytes.
    let secret =
        br#"{"session_id":"ses_SUPERSECRET","target_directory":"proj/x","target_workspace":42}"#;
    let err = decode_move_session_input(secret).unwrap_err();
    let log_line = format!("decode_failed err={err}");
    assert!(log_line.contains("target_workspace"));
    assert!(!log_line.contains("ses_SUPERSECRET"));
    assert!(!log_line.contains("proj/x"));
}
