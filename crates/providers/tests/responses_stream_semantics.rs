//! RED lane: provider stream stop-reason semantics + function-call events.
//!
//! Contract (TDD, frozen once RED is observed):
//! - S1: `response.completed` yields `Completed { stop_reason: Completed }`.
//! - S2: `response.incomplete` with `incomplete_details.reason` yields
//!   `Completed { stop_reason: Incomplete { .. } }` — NOT an error, NOT
//!   silently swallowed as UnsupportedStreamEvent.
//! - S3: a `function_call` output item (delivered via `response.output_item.done`)
//!   yields a `FunctionCall` event with call_id/name/arguments.
//! - S4: `response.completed` with zero text but >=1 function call is valid
//!   (no EmptyOutput) — the turn may end in tool calls only.
//! - S5: after a terminal event the parser reports exhaustion (next → None).
#![forbid(unsafe_code)]

use opencode_rk_providers::responses::{
    ResponsesStopReason, ResponsesStreamEvent, ResponsesStreamParser,
};

fn parser() -> ResponsesStreamParser {
    ResponsesStreamParser::new()
}

#[test]
fn s1_completed_carries_completed_stop_reason() {
    let mut p = parser();
    let e = p
        .parse_event(
            None,
            r#"{"type":"response.completed","response":{"status":"completed"}}"#,
        )
        .expect("completed event parses");
    assert_eq!(
        e,
        Some(ResponsesStreamEvent::Completed {
            stop_reason: ResponsesStopReason::Completed,
        })
    );
}

#[test]
fn s2_incomplete_maps_to_incomplete_stop_reason_not_error() {
    let mut p = parser();
    let e = p
        .parse_event(
            None,
            r#"{"type":"response.incomplete","response":{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"}}}"#,
        )
        .expect("incomplete event parses");
    assert_eq!(
        e,
        Some(ResponsesStreamEvent::Completed {
            stop_reason: ResponsesStopReason::Incomplete {
                reason: Some("max_output_tokens".to_owned()),
            },
        })
    );
}

#[test]
fn s3_function_call_item_parses_with_id_name_arguments() {
    let mut p = parser();
    let e = p
        .parse_event(
            None,
            r#"{"type":"response.output_item.done","item":{"type":"function_call","id":"fc_1","call_id":"call_abc","name":"bash","arguments":"{\"command\":\"ls\"}"}}"#,
        )
        .expect("function_call item parses");
    assert_eq!(
        e,
        Some(ResponsesStreamEvent::FunctionCall {
            call_id: "call_abc".to_owned(),
            name: "bash".to_owned(),
            arguments: "{\"command\":\"ls\"}".to_owned(),
        })
    );
}

#[test]
fn s4_completed_with_tool_call_and_no_text_is_valid() {
    let mut p = parser();
    let first = p
        .parse_event(
            None,
            r#"{"type":"response.output_item.done","item":{"type":"function_call","call_id":"call_1","name":"bash","arguments":"{}"}}"#,
        )
        .expect("function call first");
    assert!(matches!(first, Some(ResponsesStreamEvent::FunctionCall { .. })));
    let e = p
        .parse_event(
            None,
            r#"{"type":"response.completed","response":{"status":"completed"}}"#,
        )
        .expect("completed after tool call");
    assert!(matches!(
        e,
        Some(ResponsesStreamEvent::Completed {
            stop_reason: ResponsesStopReason::Completed
        })
    ));
}

#[test]
fn s5_parser_is_exhausted_after_terminal_event() {
    let mut p = parser();
    let _ = p
        .parse_event(None, r#"{"type":"response.completed","response":{"status":"completed"}}"#)
        .expect("terminal");
    assert_eq!(p.parse_event(None, r#"{"type":"response.completed","response":{}}"#), Ok(None));
}

#[test]
fn s6_text_delta_still_flows_through_shared_parser() {
    let mut p = parser();
    let e = p
        .parse_event(None, r#"{"type":"response.output_text.delta","delta":"hi"}"#)
        .expect("delta");
    assert_eq!(e, Some(ResponsesStreamEvent::OutputTextDelta("hi".to_owned())));
}
