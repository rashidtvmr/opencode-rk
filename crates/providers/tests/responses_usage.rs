//! Usage semantics of the Responses client: provider-authoritative counts.
//!
//! Contract under test (`crates/providers/src/responses.rs`):
//! - `usage` is reported only when the provider reports it; a missing,
//!   null or malformed `input_tokens`/`output_tokens` withholds the whole
//!   usage (`None`), never zeroes it.
//! - `cached_tokens`/`reasoning_tokens` are `Option<u64>`: `Some(count)`
//!   only when the provider reported the number verbatim (explicit zero
//!   preserved as `Some(0)`); absent, null or malformed details stay
//!   `None` ("unreported"), never an invented zero. Reported numbers
//!   pass through verbatim even when inconsistent.
//! - Usage never accumulates inside one parser: it is the terminal event's
//!   per-turn provider statement. Cross-turn accumulation belongs to the
//!   caller (cost counters), and tool-loop payloads keep the exact
//!   call/output pairs that make the provider's next per-turn usage
//!   billable and attributable.

use opencode_rk_providers::responses::{
    responses_request_payload, BufferedResponsesBody, ReportedUsage, ResponsesItem,
    ResponsesRole, ResponsesStopReason, ResponsesStreamEvent, ResponsesStreamParser,
    ResponsesTool,
};
use serde_json::{json, Value};

fn usage_object(
    input: Option<Value>,
    output: Option<Value>,
    cached: Option<Value>,
    reasoning: Option<Value>,
) -> Value {
    let mut map = serde_json::Map::new();
    if let Some(value) = input {
        map.insert("input_tokens".into(), value);
    }
    if let Some(value) = output {
        map.insert("output_tokens".into(), value);
    }
    if let Some(cached) = cached {
        map.insert("input_tokens_details".into(), json!({ "cached_tokens": cached }));
    }
    if let Some(reasoning) = reasoning {
        map.insert(
            "output_tokens_details".into(),
            json!({ "reasoning_tokens": reasoning }),
        );
    }
    map.into()
}

fn feed(parser: &mut ResponsesStreamParser, data: &str) -> Option<ResponsesStreamEvent> {
    parser
        .parse_event(None, &format!("data: {data}\n\n"))
        .unwrap()
}

fn full_usage(
    input: u64,
    output: u64,
    cached: Option<u64>,
    reasoning: Option<u64>,
) -> Option<ReportedUsage> {
    Some(ReportedUsage {
        input_tokens: input,
        output_tokens: output,
        cached_tokens: cached,
        reasoning_tokens: reasoning,
    })
}

fn usage(input: Option<Value>, output: Option<Value>, cached: Option<Value>, reasoning: Option<Value>) -> Value {
    usage_object(input, output, cached, reasoning)
}

#[test]
fn stream_completed_reports_usage_verbatim() {
    let mut parser = ResponsesStreamParser::new();
    let completed = json!({
        "type": "response.completed",
        "response": { "usage": usage(Some(json!(120)), Some(json!(30)), Some(json!(40)), Some(json!(10))) }
    })
    .to_string();
    let terminal = feed(&mut parser, &completed);
    assert!(matches!(
        terminal,
        Some(ResponsesStreamEvent::Completed {
            stop_reason: ResponsesStopReason::Completed
        })
    ));
    assert_eq!(parser.usage(), full_usage(120, 30, Some(40), Some(10)));
}

#[test]
fn stream_incomplete_reports_usage_with_stop_reason() {
    let mut parser = ResponsesStreamParser::new();
    let incomplete = json!({
        "type": "response.incomplete",
        "response": {
            "incomplete_details": { "reason": "max_output_tokens" },
            "usage": usage(Some(json!(70)), Some(json!(50)), None, Some(json!(50)))
        }
    })
    .to_string();
    let terminal = feed(&mut parser, &incomplete);
    assert!(matches!(
        terminal,
        Some(ResponsesStreamEvent::Completed {
            stop_reason: ResponsesStopReason::Incomplete { ref reason }
        }) if reason.as_deref() == Some("max_output_tokens")
    ));
    // Details object entirely absent: cached stays unknown (`None`), the
    // provider-reported reasoning count passes through verbatim.
    assert_eq!(parser.usage(), full_usage(70, 50, None, Some(50)));
}

#[test]
fn terminal_events_without_usage_stay_unreported_not_zero() {
    for response in [
        json!({}),                                   // standard: no usage key
        json!({ "usage": Value::Null }),             // compatible providers
    ] {
        let mut parser = ResponsesStreamParser::new();
        let completed = json!({ "type": "response.completed", "response": response }).to_string();
        feed(&mut parser, &completed);
        assert_eq!(
            parser.usage(),
            None,
            "absent usage must stay unreported, not become zeros"
        );
    }
}

#[test]
fn required_counts_missing_malformed_or_fractional_withhold_whole_usage() {
    // Input missing while output present: no invented zero, no partial usage.
    let cases = vec![
        usage(None, Some(json!(30)), None, None),
        usage(Some(json!("120")), None, None, None), // string, not a number
        usage(Some(json!(120.5)), None, None, None), // fractional truncation
        usage(Some(Value::Null), None, None, None),  // explicit null
        usage(Some(json!(-5)), None, None, None),    // negative claim
        json!(5),                                    // usage is not an object
    ];
    for bad in cases {
        let mut parser = ResponsesStreamParser::new();
        let completed =
            json!({ "type": "response.completed", "response": { "usage": bad } }).to_string();
        feed(&mut parser, &completed);
        assert_eq!(parser.usage(), None, "case {bad} must withhold usage");
    }
}

#[test]
fn missing_null_and_malformed_subsets_stay_unknown_not_zero() {
    // Every unreported subset stays `None` (unknown), never an invented 0
    // that would conflate "provider reported exactly zero" with "not
    // reported". Malformed detail fields degrade to unknown.
    let cases = [
        // details object absent entirely
        json!({ "input_tokens": 10, "output_tokens": 5 }),
        // details object explicitly null
        json!({ "input_tokens": 10, "output_tokens": 5, "input_tokens_details": Value::Null }),
        // subset field explicitly null
        json!({ "input_tokens": 10, "output_tokens": 5,
                "input_tokens_details": { "cached_tokens": Value::Null } }),
        // subset field malformed (string, not a number)
        json!({ "input_tokens": 10, "output_tokens": 5,
                "input_tokens_details": { "cached_tokens": "40" } }),
        // subset field negative claim
        json!({ "input_tokens": 10, "output_tokens": 5,
                "output_tokens_details": { "reasoning_tokens": -1 } }),
        // subset field fractional claim
        json!({ "input_tokens": 10, "output_tokens": 5,
                "output_tokens_details": { "reasoning_tokens": 2.5 } }),
        // details object wrong type (string, not an object)
        json!({ "input_tokens": 10, "output_tokens": 5, "input_tokens_details": "no" }),
    ];
    for bad in cases {
        let mut parser = ResponsesStreamParser::new();
        let completed =
            json!({ "type": "response.completed", "response": { "usage": bad } }).to_string();
        feed(&mut parser, &completed);
        let reported = parser.usage().expect("input/output present: usage reported");
        assert_eq!(
            reported.cached_tokens,
            None,
            "case {bad}: unreported cached must stay unknown, not 0"
        );
        assert_eq!(
            reported.reasoning_tokens,
            None,
            "case {bad}: unreported reasoning must stay unknown, not 0"
        );
    }
}

#[test]
fn explicit_zero_subsets_stay_reported_zeros() {
    // An explicit provider-asserted zero is a real report and must be
    // preserved as `Some(0)`, the distinct fact from `None` above.
    let mut parser = ResponsesStreamParser::new();
    let completed = json!({
        "type": "response.completed",
        "response": { "usage": {
            "input_tokens": 30,
            "output_tokens": 20,
            "input_tokens_details": { "cached_tokens": 0 },
            "output_tokens_details": { "reasoning_tokens": 0 },
        } }
    })
    .to_string();
    feed(&mut parser, &completed);
    assert_eq!(
        parser.usage(),
        Some(ReportedUsage {
            input_tokens: 30,
            output_tokens: 20,
            cached_tokens: Some(0),
            reasoning_tokens: Some(0),
        })
    );
}

#[test]
fn integral_float_counts_and_subset_details_parse() {
    let mut parser = ResponsesStreamParser::new();
    let completed = json!({
        "type": "response.completed",
        "response": {
            // JS-style integral floats plus only one details object.
            "usage": usage(
                Some(json!(120.0)),
                Some(json!(30.0)),
                Some(json!(Value::String("40".into()))), // malformed detail
                None,
            )
        }
    })
    .to_string();
    feed(&mut parser, &completed);
    // Malformed cached detail is unknown, not zero; the reasoning details
    // object is entirely absent and also stays unknown.
    assert_eq!(parser.usage(), full_usage(120, 30, None, None));
}

#[test]
fn provider_counts_pass_through_verbatim_even_when_inconsistent() {
    // cached_tokens exceeding input_tokens, reasoning-only details, an
    // explicitly reported zero: the client passes them through untouched.
    let mut parser = ResponsesStreamParser::new();
    let completed = json!({
        "type": "response.completed",
        "response": {
            "usage": usage(Some(json!(30)), Some(json!(20)), Some(json!(40)), Some(json!(0)))
        }
    })
    .to_string();
    feed(&mut parser, &completed);
    assert_eq!(parser.usage(), full_usage(30, 20, Some(40), Some(0)));
}

fn buffered_envelope(status: &str, output: Value, usage: Option<Value>, reason: Option<&str>) -> String {
    let mut envelope = json!({ "status": status, "output": output });
    if let Some(reason) = reason {
        envelope["incomplete_details"] = json!({ "reason": reason });
    }
    if let Some(usage) = usage {
        envelope["usage"] = usage;
    }
    envelope.to_string()
}

fn assistant_text(text: &str) -> Value {
    json!([
        { "type": "message", "role": "assistant",
          "content": [{ "type": "output_text", "text": text }] }
    ])
}

#[test]
fn buffered_completed_reports_usage_and_missing_usage_stays_none() {
    let body = buffered_envelope(
        "completed",
        assistant_text("hi"),
        Some(usage(Some(json!(10)), Some(json!(5)), Some(json!(2)), Some(json!(3)))),
        None,
    );
    let buffered = BufferedResponsesBody::parse(&body).unwrap();
    assert_eq!(buffered.usage(), full_usage(10, 5, Some(2), Some(3)));

    let no_usage = buffered_envelope("completed", assistant_text("hi"), None, None);
    let buffered = BufferedResponsesBody::parse(&no_usage).unwrap();
    assert_eq!(buffered.usage(), None);
}

#[test]
fn buffered_incomplete_reports_usage_and_suppresses_partial_tool_calls() {
    let body = buffered_envelope(
        "incomplete",
        json!([
            { "type": "function_call", "call_id": "call_1", "name": "tool", "arguments": "{}" },
            { "type": "message", "role": "assistant",
              "content": [{ "type": "output_text", "text": "partial" }] }
        ]),
        Some(usage(Some(json!(7)), Some(json!(2)), None, None)),
        Some("max_output_tokens"),
    );
    let buffered = BufferedResponsesBody::parse(&body).unwrap();
    assert_eq!(
        buffered.usage(),
        full_usage(7, 2, None, None),
        "incomplete turns are still billable usage"
    );
    let events = buffered.into_events();
    assert!(!events
        .iter()
        .any(|event| matches!(event, ResponsesStreamEvent::FunctionCall { .. })));
    assert!(matches!(
        events.last(),
        Some(ResponsesStreamEvent::Completed {
            stop_reason: ResponsesStopReason::Incomplete { reason: Some(reason) }
        }) if reason == "max_output_tokens"
    ));
}

#[test]
fn usage_is_per_turn_and_tool_loop_payload_preserves_call_output_pairs() {
    // Turn 1: the model requests one tool call; usage is the completed
    // turn's provider statement, not accumulated state.
    let mut turn1 = ResponsesStreamParser::new();
    let call_event = json!({
        "type": "response.output_item.done",
        "item": { "type": "function_call", "call_id": "call_1", "name": "read_file",
                  "arguments": r#"{"path":"a.rs"}"# }
    })
    .to_string();
    assert!(matches!(
        feed(&mut turn1, &call_event),
        Some(ResponsesStreamEvent::FunctionCall { call_id, name, .. })
            if call_id == "call_1" && name == "read_file"
    ));
    let completed1 = json!({
        "type": "response.completed",
        "response": { "usage": usage(Some(json!(120)), Some(json!(30)), Some(json!(40)), Some(json!(10))) }
    })
    .to_string();
    feed(&mut turn1, &completed1);
    assert_eq!(turn1.usage(), full_usage(120, 30, Some(40), Some(10)));

    // Turn 2: a fresh parser sees only turn 2's usage. Aggregation across
    // turns is the caller's job (cost counters), never the parser's.
    let mut turn2 = ResponsesStreamParser::new();
    let completed2 = json!({
        "type": "response.completed",
        "response": { "usage": usage(Some(json!(70)), Some(json!(50)), None, Some(json!(50))) }
    })
    .to_string();
    feed(&mut turn2, &completed2);
    assert_eq!(turn2.usage(), full_usage(70, 50, None, Some(50)));
    assert_ne!(turn2.usage(), turn1.usage());

    // The next request carries the exact call/output pair so the provider
    // can attribute and bill turn 2's usage to the agentic loop.
    let items = vec![
        ResponsesItem::FunctionCall {
            call_id: "call_1".into(),
            name: "read_file".into(),
            arguments: r#"{"path":"a.rs"}"#.into(),
        },
        ResponsesItem::FunctionCallOutput {
            call_id: "call_1".into(),
            output: "fn a() {}".into(),
        },
        ResponsesItem::Text {
            role: ResponsesRole::User,
            content: "done".into(),
        },
    ];
    let payload = responses_request_payload(
        "gpt-5",
        "low",
        &items,
        &[ResponsesTool::function(
            "read_file",
            "Read a file",
            json!({ "type": "object", "properties": { "path": { "type": "string" } } }),
        )],
        true,
    )
    .unwrap();
    assert_eq!(payload["input"][0]["type"], "function_call");
    assert_eq!(payload["input"][0]["call_id"], "call_1");
    assert_eq!(payload["input"][1]["type"], "function_call_output");
    assert_eq!(payload["input"][1]["output"], "fn a() {}");
    assert_eq!(payload["input"][2]["role"], "user");
    assert_eq!(payload["tools"][0]["name"], "read_file");
}

#[test]
fn duplicate_completed_event_cannot_overwrite_usage() {
    let mut parser = ResponsesStreamParser::new();
    let first =
        json!({ "type": "response.completed", "response": { "usage": usage(Some(json!(9)), Some(json!(4)), None, None) } })
            .to_string();
    feed(&mut parser, &first);
    assert_eq!(parser.usage(), full_usage(9, 4, None, None));

    let forged =
        json!({ "type": "response.completed", "response": { "usage": usage(Some(json!(999)), Some(json!(999)), None, None) } })
            .to_string();
    assert!(parser.parse_event(None, &forged).unwrap().is_none());
    assert_eq!(parser.usage(), full_usage(9, 4, None, None));
}
#[test]
fn openai_responses_inclusive_input_does_not_double_count_cache_subset() {
    let mut parser = ResponsesStreamParser::new();
    let completed = json!({
        "type":"response.completed",
        "response":{"usage":{"input_tokens":125000,"output_tokens":32,
        "input_tokens_details":{"cached_tokens":120000}}}
    })
    .to_string();
    feed(&mut parser, &completed);
    let usage = parser.usage().expect("provider usage");
    assert_eq!(usage.input_tokens, 125000);
    assert_eq!(usage.cached_tokens, Some(120000));
}
