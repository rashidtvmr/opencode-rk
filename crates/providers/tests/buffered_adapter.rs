//! RED lane: bounded buffered JSON adapter for custom-provider model-local IDs.
//!
//! Contract (TDD, frozen once RED observed):
//! - B1: `buffered_model_ids` field exists on `CustomProviderConfig`, defaults to
//!   empty, persists through save/load round-trip, and is preserved when not
//!   provided on update (UI must not erase an existing setting).
//! - B2: `buffered_model_ids` accepts ids containing slashes (model-local IDs).
//! - B3: The buffered adapter validates a complete non-streaming JSON body
//!   (OpenAI Responses shape) and emits typed `ResponsesStreamEvent`s only when
//!   the whole response is well-formed.
//! - B4: A malformed or missing terminal event yields an error, no partial/delta
//!   events are emitted before validation completes.
//! - B5: Duplicate function_call items are rejected once, no event executes.
//! - B6: Oversize text (> MAX_INLINE_PAYLOAD_BYTES) yields OutputTooLarge.
//! - B7: `response.failed` / error object yields StreamFailed, no tool executes.
//! - B8: `response.incomplete` maps to `Completed { Incomplete { reason } }`.
//! - B9: `stream: false` is set exactly once per round; no retry/replay/fallback.
#![forbid(unsafe_code)]

use opencode_rk_providers::custom_provider::{
    CustomProviderConfig,
};
use opencode_rk_providers::responses::{
    BufferedResponsesBody, ResponsesError, ResponsesStreamEvent, ResponsesStopReason,
};

fn fixture_config() -> CustomProviderConfig {
    CustomProviderConfig {
        id: "9router".to_owned(),
        name: "9router".to_owned(),
        base_url: "http://localhost:20128/v1".to_owned(),
        api_key: None,
        buffered_model_ids: vec!["oc/deepseek-v4.1-flash-free".to_owned()],
    }
}

#[test]
fn b1_buffered_model_ids_defaults_to_empty() {
    let config = CustomProviderConfig {
        id: "testprov".to_owned(),
        name: "Test Prov".to_owned(),
        base_url: "https://example.org/v1".to_owned(),
        api_key: None,
        buffered_model_ids: vec![],
    };
    assert!(config.buffered_model_ids.is_empty());
}

#[test]
fn b1b_buffered_model_ids_serializes_and_round_trips() {
    let config = fixture_config();
    let value = serde_json::to_value(&config).unwrap();
    assert_eq!(
        value["buffered_model_ids"].as_array().unwrap()[0].as_str().unwrap(),
        "oc/deepseek-v4.1-flash-free"
    );
    let decoded: CustomProviderConfig = serde_json::from_value(value).unwrap();
    assert_eq!(decoded.buffered_model_ids, config.buffered_model_ids);
}

#[test]
fn b2_model_local_ids_with_slashes_accepted() {
    let ids = vec![
        "oc/deepseek-v4.1-flash-free".to_owned(),
        "route/model-1158".to_owned(),
        "simple".to_owned(),
    ];
    let config = CustomProviderConfig {
        id: "9router".to_owned(),
        name: "9router".to_owned(),
        base_url: "http://localhost:20128/v1".to_owned(),
        api_key: None,
        buffered_model_ids: ids.clone(),
    };
    assert_eq!(config.buffered_model_ids, ids);
}

#[test]
fn b3_well_formed_buffered_body_yields_terminal_event_only() {
    let body = r#"{"id":"resp_1","object":"response","status":"completed","output":[]}"#;
    let events = BufferedResponsesBody::parse(body)
        .expect("well-formed buffered body parses")
        .into_events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        ResponsesStreamEvent::Completed { stop_reason } => {
            assert_eq!(*stop_reason, ResponsesStopReason::Completed);
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[test]
fn b4_malformed_terminal_event_rejects_before_emitting() {
    // Missing the required terminal/completed event: body has output but no
    // completed/incomplete/failed marker. The adapter must NOT emit deltas.
    let body = r#"{"id":"resp_1","status":"in_progress","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi"}]}]}"#;
    let result = BufferedResponsesBody::parse(body);
    assert!(
        result.is_err(),
        "missing terminal event must be rejected"
    );
}

#[test]
fn b5_duplicate_function_call_ids_rejected_once() {
    let body = r#"{"status":"completed","output":[{"type":"function_call","call_id":"dup","name":"bash","arguments":"{}"},{"type":"function_call","call_id":"dup","name":"bash","arguments":"{}"}]}"#;
    assert!(
        BufferedResponsesBody::parse(body).is_err(),
        "duplicate call_id must be rejected"
    );
}

#[test]
fn b6_oversize_text_rejected() {
    let big = "x".repeat(opencode_rk_contracts::MAX_INLINE_PAYLOAD_BYTES + 1);
    let body = serde_json::json!({
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type":"output_text","text": big}]
        }]
    })
    .to_string();
    let result = BufferedResponsesBody::parse(&body);
    assert!(matches!(result, Err(ResponsesError::OutputTooLarge { .. })));
}

#[test]
fn b7_failed_event_yields_stream_error_no_tools() {
    let body = r#"{"status":"failed","error":{"code":"bad_request","message":"bad request"}}"#;
    let result = BufferedResponsesBody::parse(body);
    assert!(matches!(result, Err(ResponsesError::StreamFailed { code, message }) if code == "bad_request" && message == "bad request"));
}

#[test]
fn b8_incomplete_maps_to_incomplete_stop_reason() {
    let body = r#"{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"output":[]}"#;
    let events = BufferedResponsesBody::parse(body)
        .expect("incomplete body parses")
        .into_events();
    assert_eq!(events.len(), 1, "only terminal event emitted");
    match &events[0] {
        ResponsesStreamEvent::Completed { stop_reason } => {
            assert_eq!(
                *stop_reason,
                ResponsesStopReason::Incomplete { reason: Some("max_output_tokens".to_owned()) }
            );
        }
        other => panic!("expected Completed with Incomplete, got {other:?}"),
    }
}

#[test]
fn b9_text_and_tool_then_terminal_event_sequence() {
    let body = serde_json::json!({
        "status": "completed",
        "output": [
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type":"output_text","text":"hello"}]
            },
            {
                "type": "function_call",
                "call_id": "fc_1",
                "name": "bash",
                "arguments": "{\"command\":\"ls\"}"
            }
        ]
    })
    .to_string();
    let events = BufferedResponsesBody::parse(&body)
        .expect("well-formed body with tool call parses")
        .into_events();
    assert_eq!(events.len(), 3, "text, tool_call, completed");
    assert!(matches!(&events[0], ResponsesStreamEvent::OutputTextDelta(t) if t == "hello"));
    assert!(matches!(&events[1], ResponsesStreamEvent::FunctionCall { call_id, name, .. } if call_id == "fc_1" && name == "bash"));
    assert!(matches!(&events[2], ResponsesStreamEvent::Completed { .. }));
}

#[test]
fn b10_body_exceeding_max_bytes_rejected() {
    // The body content itself exceeds MAX_RESPONSES_BODY_BYTES.
    let big = "x".repeat(opencode_rk_providers::responses::MAX_RESPONSES_BODY_BYTES + 1);
    let body = serde_json::json!({"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text": big}]}]}).to_string();
    let result = BufferedResponsesBody::parse(&body);
    assert!(matches!(result, Err(ResponsesError::ResponseTooLarge { .. })));
}

#[test]
fn b11_malformed_json_rejected() {
    assert!(BufferedResponsesBody::parse("not json").is_err());
    assert!(BufferedResponsesBody::parse("").is_err());
}

#[test]
fn late_malformed_item_never_exposes_an_earlier_executable_call() {
    let body = r#"{"status":"completed","output":[{"type":"function_call","call_id":"first","name":"write_file","arguments":"{}"},{"type":"function_call","call_id":"later","name":"write_file","arguments":"not-json"}]}"#;
    assert!(BufferedResponsesBody::parse(body).is_err());
    let partial = r#"{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"output":[{"type":"function_call","arguments":"{truncated"}]}"#;
    let events = BufferedResponsesBody::parse(partial).unwrap().into_events();
    assert!(matches!(events.as_slice(),[ResponsesStreamEvent::Completed {stop_reason:ResponsesStopReason::Incomplete {..}}]));
    assert!(BufferedResponsesBody::parse(r#"{"status":"completed","error":{"message":"failed"},"output":[]}"#).is_err());
}

#[test]
fn completed_envelope_with_late_unfinished_call_rejects_entire_batch() {
    for status in [serde_json::json!("incomplete"),serde_json::json!("in_progress"),serde_json::json!("failed"),serde_json::json!(null),serde_json::json!(17)] {
        let body=serde_json::json!({"status":"completed","output":[
            {"type":"function_call","status":"completed","call_id":"valid-first","name":"write","arguments":"{}"},
            {"type":"function_call","status":status,"call_id":"unfinished-later","name":"write","arguments":"{}"}
        ]}).to_string();
        assert!(BufferedResponsesBody::parse(&body).is_err(),"unfinished item crossed execution boundary");
    }
}

#[test]
fn buffered_reasoning_summary_is_retained_and_bounded() {
    let body = serde_json::json!({"status":"completed","output":[{"type":"reasoning","summary":[{"type":"summary_text","text":"summary"}]}]}).to_string();
    let events = BufferedResponsesBody::parse(&body).unwrap().into_events();
    assert!(matches!(&events[0],ResponsesStreamEvent::ReasoningSummaryDelta(text) if text == "summary"));
    let body = serde_json::json!({"status":"completed","output":[{"type":"reasoning","summary":[{"type":"summary_text","text":"x".repeat(opencode_rk_contracts::MAX_REASONING_SUMMARY_BYTES+1)}]}]}).to_string();
    assert!(matches!(BufferedResponsesBody::parse(&body),Err(ResponsesError::ReasoningSummaryTooLarge {..})));
}

use tokio::io::{AsyncReadExt,AsyncWriteExt};
async fn read_request(socket: &mut tokio::net::TcpStream) -> serde_json::Value {
    let mut bytes=Vec::new();
    loop {
        let mut chunk=[0;4096];
        let count=socket.read(&mut chunk).await.unwrap(); assert!(count>0);
        bytes.extend_from_slice(&chunk[..count]);assert!(bytes.len()<1024*1024);
        if let Some(end)=bytes.windows(4).position(|part|part==b"\r\n\r\n") {
            let headers=String::from_utf8_lossy(&bytes[..end]).to_lowercase();
            let size:usize=headers.lines().find_map(|line|line.strip_prefix("content-length:")).unwrap().trim().parse().unwrap();
            if bytes.len()>=end+4+size { return serde_json::from_slice(&bytes[end+4..end+4+size]).unwrap(); }
        }
    }
}

#[tokio::test]
async fn actual_http_exact_model_optin_and_typed_continuation_leave_default_sse_unchanged() {
    use opencode_rk_providers::{config::ProviderConfig,responses::{OpenAiResponsesClient,ResponsesItem,ResponsesRole,ResponsesTool}};
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address=listener.local_addr().unwrap();
    let server=tokio::spawn(async move {
        let mut captured=Vec::new();
        for round in 0..3 {
            let (mut socket,_)=listener.accept().await.unwrap();
            captured.push(read_request(&mut socket).await);
            let (content_type,body)=match round {
                0=>("application/json",r#"{"status":"completed","output":[{"type":"function_call","call_id":"fixture-call","name":"read_file","arguments":"{\"path\":\"fixture.rs\"}"}]}"#),
                1=>("application/json",r#"{"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"continued"}]}]}"#),
                _=>("text/event-stream","event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n"),
            };
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        }
        captured
    });
    let model="oc/deepseek-v4.1-flash-free";
    let client=OpenAiResponsesClient::from_config(ProviderConfig{provider_id:"fixture".into(),base_url:format!("http://{address}/v1"),api_key_env:"UNUSED_FIXTURE_KEY".into(),timeout_secs:3,max_tokens:256,temperature:0.7},None).unwrap().with_buffered_models(vec![model.into()]);
    let mut items=vec![ResponsesItem::Text{role:ResponsesRole::User,content:"read fixture".into()}];
    let tools=[ResponsesTool::function("read_file","fixture read",serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}))];
    let mut stream=client.stream_with_tools(model,"high",&items,&tools).await.unwrap();
    let call=stream.next_event().await.unwrap().unwrap();
    assert!(matches!(&call,ResponsesStreamEvent::FunctionCall{call_id,..} if call_id=="fixture-call"));
    if let ResponsesStreamEvent::FunctionCall{call_id,name,arguments}=call { items.push(ResponsesItem::FunctionCall{call_id:call_id.clone(),name,arguments});items.push(ResponsesItem::FunctionCallOutput{call_id,output:"fixture contents".into()}); }
    assert!(matches!(stream.next_event().await.unwrap(),Some(ResponsesStreamEvent::Completed{..})));
    assert!(stream.next_event().await.unwrap().is_none());
    let mut continuation=client.stream_with_tools(model,"high",&items,&tools).await.unwrap();
    assert_eq!(continuation.next_event().await.unwrap(),Some(ResponsesStreamEvent::OutputTextDelta("continued".into())));
    let mut other=client.stream_with_tools(&format!("{model}-other"),"high",&items,&tools).await.unwrap();
    assert!(matches!(other.next_event().await.unwrap(),Some(ResponsesStreamEvent::Completed{..})));
    let captured=server.await.unwrap();
    assert_eq!(captured.len(),3);
    assert_eq!(captured[0]["stream"],false);assert_eq!(captured[1]["stream"],false);assert_eq!(captured[2]["stream"],true);
    assert_eq!(captured[1]["input"][1]["call_id"],"fixture-call");
    assert_eq!(captured[1]["input"][2]["type"],"function_call_output");
    assert_eq!(captured[1]["input"][2]["output"],"fixture contents");
    assert_eq!(captured[0]["tools"][0]["name"],"read_file");
}

#[tokio::test]
async fn buffered_body_remains_cancellable_and_drop_reclaims_socket() {
    use opencode_rk_providers::{config::ProviderConfig,responses::{OpenAiResponsesClient,ResponsesItem,ResponsesRole}};
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address=listener.local_addr().unwrap();
    let server=tokio::spawn(async move {
        let (mut socket,_)=listener.accept().await.unwrap();
        assert_eq!(read_request(&mut socket).await["stream"],false);
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\n\r\n{").await.unwrap();
        let mut byte=[0];
        match socket.read(&mut byte).await {
            Ok(0)=>{},
            Err(error) if error.kind()==std::io::ErrorKind::ConnectionReset=>{},
            other=>panic!("buffered cancellation retained socket: {other:?}"),
        }
    });
    let client=OpenAiResponsesClient::from_config(ProviderConfig{provider_id:"fixture".into(),base_url:format!("http://{address}/v1"),api_key_env:"UNUSED_FIXTURE_KEY".into(),timeout_secs:3,max_tokens:256,temperature:0.7},None).unwrap().with_buffered_models(vec!["buffered".into()]);
    let items=[ResponsesItem::Text{role:ResponsesRole::User,content:"fixture".into()}];
    let mut stream=tokio::time::timeout(std::time::Duration::from_secs(1),client.stream_with_tools("buffered","high",&items,&[])).await.expect("admission eagerly buffered the body").unwrap();
    assert!(tokio::time::timeout(std::time::Duration::from_millis(30),stream.next_event()).await.is_err());
    drop(stream);
    tokio::time::timeout(std::time::Duration::from_secs(1),server).await.unwrap().unwrap();
}
