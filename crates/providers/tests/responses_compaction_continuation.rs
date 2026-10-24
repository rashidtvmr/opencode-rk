//! Lossless continuation from standalone OpenAI Responses compaction.
#![forbid(unsafe_code)]

use opencode_rk_providers::{
    config::ProviderConfig,
    responses::{
        MAX_RESPONSES_INPUT_BYTES, MAX_RESPONSES_INPUT_MESSAGES, OpenAiResponsesClient,
        ResponsesCompactionWindow, ResponsesError, ResponsesItem, ResponsesRole, ResponsesTool,
    },
};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

struct Fixture {
    address: String,
    requests: tokio::task::JoinHandle<Vec<(String, Value)>>,
}

async fn spawn_fixture(
    compacted: Value,
    next_response: Vec<u8>,
    next_content_type: &str,
) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let next_content_type = next_content_type.to_owned();
    let requests = tokio::spawn(async move {
        let mut captured = Vec::new();
        for index in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_request(&mut socket).await;
            let (headers, body) = request.split_once("\r\n\r\n").unwrap();
            let request_line = headers.lines().next().unwrap().to_owned();
            captured.push((request_line, serde_json::from_str(body).unwrap()));

            let (content_type, response_body) = if index == 0 {
                ("application/json", serde_json::to_vec(&compacted).unwrap())
            } else {
                (next_content_type.as_str(), next_response.clone())
            };
            let response_headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response_body.len()
            );
            socket.write_all(response_headers.as_bytes()).await.unwrap();
            socket.write_all(&response_body).await.unwrap();
            let _ = socket.shutdown().await;
        }
        captured
    });
    Fixture { address, requests }
}

async fn read_request(socket: &mut TcpStream) -> String {
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        if let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&raw[..header_end]).unwrap();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if raw.len() >= header_end + 4 + content_length {
                return String::from_utf8(raw).unwrap();
            }
        }
        let read = socket.read(&mut chunk).await.unwrap();
        assert_ne!(read, 0, "request ended before its complete body");
        raw.extend_from_slice(&chunk[..read]);
    }
}

fn fixture_client(address: &str) -> OpenAiResponsesClient {
    OpenAiResponsesClient::from_config(
        ProviderConfig {
            provider_id: "openai".into(),
            base_url: format!("http://{address}/v1"),
            api_key_env: "UNUSED_FIXTURE_KEY".into(),
            timeout_secs: 3,
            max_tokens: 64,
            temperature: 0.0,
        },
        Some("fixture-secret".into()),
    )
    .unwrap()
}

fn compacted_response() -> Value {
    json!({
        "id": "resp_compact_1",
        "object": "response.compaction",
        "created_at": 1764967971,
        "usage": {"input_tokens": 80, "output_tokens": 0},
        "output": [
            {
                "id": "msg_keep",
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "phase": "commentary",
                "content": [{
                    "type": "output_text",
                    "text": "retained message",
                    "annotations": [{"type":"url_citation","url":"https://example.test","start_index":0,"end_index":3,"title":"source"}]
                }],
                "future_message_metadata": {"keep": [true, 7]}
            },
            {
                "id": "fc_item_keep",
                "type": "function_call",
                "call_id": "call_keep",
                "name": "lookup",
                "arguments": "{\"key\":\"value\"}",
                "status": "completed",
                "future_tool_metadata": {"origin": "provider"}
            },
            {
                "type": "compaction",
                "id": "cmp_keep",
                "encrypted_content": "opaque+/=bytes-雪",
                "future_compaction_metadata": "must survive"
            }
        ]
    })
}

fn expected_tail() -> ResponsesItem {
    ResponsesItem::Text {
        role: ResponsesRole::User,
        content: "continue with the next step".into(),
    }
}

fn canonical_window(output: Vec<Value>) -> ResponsesCompactionWindow {
    ResponsesCompactionWindow {
        id: "resp_fixture".into(),
        created_at: 1,
        usage: None,
        output,
    }
}

fn captured_input(request: &(String, Value), expected_path: &str) -> Vec<Value> {
    assert_eq!(request.0, format!("POST {expected_path} HTTP/1.1"));
    request.1["input"].as_array().unwrap().clone()
}

#[tokio::test(flavor = "current_thread")]
async fn compact_then_create_forwards_canonical_items_and_unknown_fields_unchanged() {
    let compacted = compacted_response();
    let expected_output = compacted["output"].as_array().unwrap().clone();
    let fixture = spawn_fixture(
        compacted,
        br#"{"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"continued"}]}]}"#.to_vec(),
        "application/json",
    )
    .await;
    let client = fixture_client(&fixture.address);

    let window = client
        .compact("fixture-model", &[])
        .await
        .expect("compact window parses");
    let tail = [expected_tail()];
    assert_eq!(
        client
            .create_with_compaction_window("fixture-model", "medium", &window, &tail)
            .await
            .expect("canonical window continues"),
        "continued"
    );

    let requests = fixture.requests.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].0, "POST /v1/responses/compact HTTP/1.1");
    let next_input = captured_input(&requests[1], "/v1/responses");
    assert_eq!(
        &next_input[..expected_output.len()],
        expected_output.as_slice()
    );
    assert_eq!(
        next_input.last().unwrap(),
        &json!({"role":"user","content":"continue with the next step"})
    );
    assert_eq!(requests[1].1["reasoning"]["effort"], "medium");
    assert!(requests[1].1.get("stream").is_none());
    assert!(requests[1].1.get("tools").is_none());
    assert_eq!(requests[1].1["max_output_tokens"], 64);
    assert_eq!(
        window.output, expected_output,
        "continuation leaves the window immutable"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn compact_then_stream_forwards_canonical_items_with_tools_and_streaming() {
    let compacted = compacted_response();
    let expected_output = compacted["output"].as_array().unwrap().clone();
    let fixture = spawn_fixture(
        compacted,
        b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n"
            .to_vec(),
        "text/event-stream",
    )
    .await;
    let client = fixture_client(&fixture.address);

    let window = client
        .compact("fixture-model", &[])
        .await
        .expect("compact window parses");
    let tail = [expected_tail()];
    let tools = [ResponsesTool::function(
        "lookup",
        "Find an item",
        json!({"type":"object","properties":{"key":{"type":"string"}},"required":["key"]}),
    )];
    let mut stream = client
        .stream_with_compaction_window("fixture-model", "high", &window, &tail, &tools)
        .await
        .expect("canonical window streams");
    assert!(matches!(
        stream.next_event().await.unwrap(),
        Some(opencode_rk_providers::responses::ResponsesStreamEvent::Completed { .. })
    ));

    let requests = fixture.requests.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].0, "POST /v1/responses/compact HTTP/1.1");
    let next_input = captured_input(&requests[1], "/v1/responses");
    assert_eq!(
        &next_input[..expected_output.len()],
        expected_output.as_slice()
    );
    assert_eq!(
        next_input.last().unwrap(),
        &json!({"role":"user","content":"continue with the next step"})
    );
    assert_eq!(requests[1].1["reasoning"]["effort"], "high");
    assert_eq!(requests[1].1["tools"][0]["type"], "function");
    assert_eq!(requests[1].1["tools"][0]["name"], "lookup");
    assert_eq!(requests[1].1["stream"], true);
    assert_eq!(requests[1].1["reasoning"]["summary"], "auto");
    assert_eq!(requests[1].1["max_output_tokens"], 64);
    assert_eq!(
        window.output, expected_output,
        "continuation leaves the window immutable"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn continuation_rejects_a_window_without_valid_compaction_before_network_io() {
    let client = fixture_client("127.0.0.1:1");
    for output in [
        vec![json!({"type":"message","role":"assistant"})],
        vec![json!({"type":"compaction","id":"cmp_missing_content"})],
    ] {
        let result = client
            .create_with_compaction_window("fixture-model", "", &canonical_window(output), &[])
            .await;
        assert!(matches!(result, Err(ResponsesError::InvalidJson(_))));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn continuation_rejects_count_overflow_before_network_io() {
    let mut output =
        vec![json!({"type":"message","role":"assistant"}); MAX_RESPONSES_INPUT_MESSAGES];
    output[MAX_RESPONSES_INPUT_MESSAGES - 1] = json!({
        "type":"compaction","id":"cmp_keep","encrypted_content":"opaque"
    });
    let client = fixture_client("127.0.0.1:1");
    let result = client
        .create_with_compaction_window(
            "fixture-model",
            "",
            &canonical_window(output),
            &[expected_tail()],
        )
        .await;
    assert!(matches!(
        result,
        Err(ResponsesError::TooManyMessages {
            max: MAX_RESPONSES_INPUT_MESSAGES,
            actual
        }) if actual == MAX_RESPONSES_INPUT_MESSAGES + 1
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn continuation_rejects_serialized_byte_overflow_before_network_io() {
    let output = vec![
        json!({"type":"message","future_payload":"x".repeat(MAX_RESPONSES_INPUT_BYTES)}),
        json!({"type":"compaction","id":"cmp_keep","encrypted_content":"opaque"}),
    ];
    let client = fixture_client("127.0.0.1:1");
    let result = client
        .create_with_compaction_window("fixture-model", "", &canonical_window(output), &[])
        .await;
    assert!(matches!(
        result,
        Err(ResponsesError::InputTooLarge {
            max: MAX_RESPONSES_INPUT_BYTES,
            actual
        }) if actual > MAX_RESPONSES_INPUT_BYTES
    ));
}
