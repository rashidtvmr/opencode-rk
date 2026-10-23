#![forbid(unsafe_code)]

use opencode_rk_providers::responses::{
    BufferedResponsesBody, ResponsesError, ResponsesStreamParser,
};

fn parse_sse_failure(payload: &str) -> ResponsesError {
    let mut parser = ResponsesStreamParser::new();
    match parser.parse_event(None, &format!("data: {payload}\n\n")) {
        Err(error) => error,
        Ok(event) => panic!("expected terminal stream error, got {event:?}"),
    }
}

fn parse_buffered_failure(body: &str) -> ResponsesError {
    match BufferedResponsesBody::parse(body) {
        Err(error) => error,
        Ok(_) => panic!("expected buffered stream error"),
    }
}

#[test]
fn sse_failure_preserves_transient_bad_gateway_code() {
    assert_eq!(
        parse_sse_failure(
            r#"{"type":"response.failed","response":{"error":{"code":"bad_gateway","message":"upstream unavailable"}}}"#
        ),
        ResponsesError::StreamFailed {
            code: "bad_gateway".to_owned(),
            message: "upstream unavailable".to_owned(),
        }
    );
}

#[test]
fn sse_failure_preserves_permanent_bad_request_code() {
    assert_eq!(
        parse_sse_failure(
            r#"{"type":"response.failed","response":{"error":{"code":"bad_request","message":"invalid input"}}}"#
        ),
        ResponsesError::StreamFailed {
            code: "bad_request".to_owned(),
            message: "invalid input".to_owned(),
        }
    );
}

#[test]
fn sse_failure_without_code_uses_unknown() {
    assert_eq!(
        parse_sse_failure(
            r#"{"type":"response.failed","response":{"error":{"message":"provider failed"}}}"#
        ),
        ResponsesError::StreamFailed {
            code: "unknown".to_owned(),
            message: "provider failed".to_owned(),
        }
    );
}

#[test]
fn sse_failure_null_or_non_string_code_uses_unknown() {
    for code in [serde_json::Value::Null, serde_json::json!(503)] {
        let payload = serde_json::json!({
            "type": "response.failed",
            "response": { "error": { "code": code, "message": "provider failed" } }
        })
        .to_string();

        assert_eq!(
            parse_sse_failure(&payload),
            ResponsesError::StreamFailed {
                code: "unknown".to_owned(),
                message: "provider failed".to_owned(),
            }
        );
    }
}

#[test]
fn sse_failure_message_is_bounded_without_splitting_utf8() {
    let original = "界".repeat(1025);
    let payload = serde_json::json!({
        "type": "response.failed",
        "response": { "error": { "code": "bad_gateway", "message": original } }
    })
    .to_string();

    assert_eq!(
        parse_sse_failure(&payload),
        ResponsesError::StreamFailed {
            code: "bad_gateway".to_owned(),
            message: "界".repeat(1024),
        }
    );
}

#[test]
fn buffered_failure_matches_sse_error_code_and_message() {
    let sse = parse_sse_failure(
        r#"{"type":"response.failed","response":{"error":{"code":"bad_gateway","message":"upstream unavailable"}}}"#,
    );
    let buffered = parse_buffered_failure(
        r#"{"status":"failed","error":{"code":"bad_gateway","message":"upstream unavailable"}}"#,
    );

    assert_eq!(buffered, sse);
}
