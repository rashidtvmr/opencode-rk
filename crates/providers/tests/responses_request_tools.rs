//! RED lane: provider request-side tool support (typed items + tool schemas).
//!
//! Contract (TDD, frozen once RED is observed):
//! - R1: the request payload carries a Responses `tools` array of `function`
//!   schemas (type/name/description/parameters) when tools are supplied, and
//!   no `tools` key when the list is empty (back-compat shape).
//! - R2: transcript items serialize to the Responses wire format:
//!   text as `{role, content}`, `FunctionCall` as
//!   `{type:"function_call", call_id, name, arguments}`, and
//!   `FunctionCallOutput` as `{type:"function_call_output", call_id, output}`.
//! - R3: the legacy text-only input list produces the exact same payload as
//!   the equivalent typed item list (no behavior change for existing turns).
//! - R4: payload validation still applies (empty model rejected; count/byte
//!   bounds enforced over the typed items).
#![forbid(unsafe_code)]

use opencode_rk_providers::responses::{
    responses_request_payload, ResponsesItem, ResponsesRole, ResponsesTool,
};

fn bash_tool() -> ResponsesTool {
    ResponsesTool::function(
        "bash",
        "Run a shell command and return stdout",
        serde_json::json!({
            "type": "object",
            "properties": { "command": { "type": "string" } },
            "required": ["command"]
        }),
    )
}

#[test]
fn r1_payload_carries_function_tool_schemas() {
    let items = vec![ResponsesItem::Text {
        role: ResponsesRole::User,
        content: "list files".to_owned(),
    }];
    let payload =
        responses_request_payload("gpt-test", "low", &items, std::slice::from_ref(&bash_tool()), true)
            .expect("payload builds");
    let tools = payload["tools"].as_array().expect("tools array present");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["name"], "bash");
    assert_eq!(tools[0]["description"], "Run a shell command and return stdout");
    assert_eq!(tools[0]["parameters"]["properties"]["command"]["type"], "string");
    assert_eq!(payload["stream"], true);
}

#[test]
fn r1b_no_tools_omits_the_key_entirely() {
    let items = vec![ResponsesItem::Text {
        role: ResponsesRole::User,
        content: "hi".to_owned(),
    }];
    let payload =
        responses_request_payload("gpt-test", "low", &items, &[], false).expect("payload builds");
    assert!(payload.get("tools").is_none(), "no tools key when empty");
    assert_eq!(payload.get("stream"), None);
}

#[test]
fn r2_items_serialize_to_responses_wire_shapes() {
    let call = ResponsesItem::FunctionCall {
        call_id: "call_1".to_owned(),
        name: "bash".to_owned(),
        arguments: "{\"command\":\"ls\"}".to_owned(),
    };
    let output = ResponsesItem::FunctionCallOutput {
        call_id: "call_1".to_owned(),
        output: "file-a\nfile-b".to_owned(),
    };
    let payload = responses_request_payload(
        "gpt-test",
        "low",
        &[call, output],
        std::slice::from_ref(&bash_tool()),
        true,
    )
    .expect("payload builds");
    let input = payload["input"].as_array().expect("input array");
    assert_eq!(input.len(), 2);
    assert_eq!(input[0]["type"], "function_call");
    assert_eq!(input[0]["call_id"], "call_1");
    assert_eq!(input[0]["name"], "bash");
    assert_eq!(input[0]["arguments"], "{\"command\":\"ls\"}");
    assert_eq!(input[1]["type"], "function_call_output");
    assert_eq!(input[1]["call_id"], "call_1");
    assert_eq!(input[1]["output"], "file-a\nfile-b");
}

#[test]
fn r3_legacy_text_input_and_typed_text_items_are_equivalent() {
    use opencode_rk_providers::responses::ResponsesInput;
    let legacy = vec![ResponsesInput::new(ResponsesRole::User, "hello")];
    let typed = vec![ResponsesItem::Text {
        role: ResponsesRole::User,
        content: "hello".to_owned(),
    }];
    let a = responses_request_payload("m", "low", &legacy, &[], true).expect("legacy payload");
    let b = responses_request_payload("m", "low", &typed, &[], true).expect("typed payload");
    assert_eq!(a["input"], b["input"]);
}

#[test]
fn r4_validation_still_applies_to_typed_items() {
    let items = vec![ResponsesItem::FunctionCall {
        call_id: "c".to_owned(),
        name: "bash".to_owned(),
        arguments: "{}".to_owned(),
    }];
    assert!(responses_request_payload("", "low", &items, &[], true).is_err());
    assert!(responses_request_payload("m", "bogus-effort", &items, &[], true).is_err());
}
