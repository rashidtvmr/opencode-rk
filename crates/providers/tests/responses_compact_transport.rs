//! Local transport contract for the native OpenAI Responses compact endpoint.
#![forbid(unsafe_code)]

use std::time::Duration;

use opencode_rk_providers::{
    config::ProviderConfig,
    responses::{
        OpenAiResponsesClient, ResponsesError, ResponsesItem, ResponsesRole,
        MAX_RESPONSES_BODY_BYTES, MAX_RESPONSES_INPUT_BYTES, MAX_RESPONSES_INPUT_MESSAGES,
    },
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct Fixture {
    address: String,
    server: tokio::task::JoinHandle<String>,
}

async fn spawn_fixture(status: &str, body: &[u8], content_length: Option<usize>) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let status = status.to_owned();
    let body = body.to_vec();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read_request(&mut socket).await;
        let length = content_length
            .map(|length| format!("Content-Length: {length}\r\n"))
            .unwrap_or_default();
        let headers = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{length}Connection: close\r\n\r\n"
        );
        socket.write_all(headers.as_bytes()).await.unwrap();
        socket.write_all(&body).await.unwrap();
        let _ = socket.shutdown().await;
        request
    });
    Fixture { address, server }
}

async fn read_request(socket: &mut TcpStream) -> String {
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let read = socket.read(&mut chunk).await.unwrap();
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read]);
        let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let content_length = String::from_utf8_lossy(&raw[..header_end])
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if raw.len() >= header_end + 4 + content_length {
            break;
        }
    }
    String::from_utf8(raw).unwrap()
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

fn input_history() -> Vec<ResponsesItem> {
    vec![
        ResponsesItem::Text {
            role: ResponsesRole::User,
            content: "continue".into(),
        },
        ResponsesItem::FunctionCall {
            call_id: "call-1".into(),
            name: "lookup".into(),
            arguments: r#"{"key":"value"}"#.into(),
        },
        ResponsesItem::FunctionCallOutput {
            call_id: "call-1".into(),
            output: "result".into(),
        },
    ]
}

fn compacted_response() -> serde_json::Value {
    serde_json::json!({
        "id": "resp_compact_1",
        "object": "response.compaction",
        "created_at": 1764967971,
        "usage": {"input_tokens": 80, "output_tokens": 0},
        "output": [
            {
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "phase": "commentary",
                "content": [{
                    "type": "output_text",
                    "text": "retained message",
                    "annotations": [{"type": "url_citation", "url": "https://example.test", "start_index": 0, "end_index": 3, "title": "source"}]
                }]
            },
            {
                "id": "fc_item_1",
                "type": "function_call",
                "call_id": "call_2",
                "name": "lookup",
                "arguments": "{\"key\":\"value\"}",
                "status": "completed"
            },
            {
                "type": "compaction",
                "id": "cmp_1",
                "encrypted_content": "opaque+/=bytes-雪"
            }
        ]
    })
}

fn split_request(request: &str) -> (&str, &str) {
    let (headers, body) = request.split_once("\r\n\r\n").expect("HTTP body separator");
    (headers, body)
}

#[tokio::test(flavor = "current_thread")]
async fn compact_posts_bounded_typed_history_and_preserves_raw_output() {
    let response = compacted_response();
    let response_body = serde_json::to_vec(&response).unwrap();
    let fixture = spawn_fixture("200 OK", &response_body, None).await;
    let client = fixture_client(&fixture.address);
    let history = input_history();

    let window = tokio::time::timeout(
        Duration::from_secs(5),
        client.compact("fixture-model", &history),
    )
    .await
    .expect("compact request stays bounded")
    .expect("documented response.compaction accepted");
    let request = fixture.server.await.unwrap();
    let (headers, body) = split_request(&request);
    let first_line = headers.lines().next().unwrap();
    assert_eq!(first_line, "POST /v1/responses/compact HTTP/1.1");
    assert!(headers.lines().any(|line| line.eq_ignore_ascii_case("authorization: Bearer fixture-secret")));
    let payload: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(payload["model"], "fixture-model");
    assert_eq!(payload["input"][0], serde_json::json!({"role":"user","content":"continue"}));
    assert_eq!(payload["input"][1]["type"], "function_call");
    assert_eq!(payload["input"][2]["type"], "function_call_output");
    assert_eq!(payload.as_object().unwrap().len(), 2, "compact sends only model and input");

    assert_eq!(window.id, "resp_compact_1");
    assert_eq!(window.created_at, 1764967971);
    assert_eq!(window.usage, Some(response["usage"].clone()));
    assert_eq!(window.output, response["output"].as_array().unwrap().clone());
    assert_eq!(window.output[0]["id"], "msg_1");
    assert_eq!(window.output[0]["content"][0]["annotations"][0]["url"], "https://example.test");
    assert_eq!(window.output[1]["call_id"], "call_2");
    assert_eq!(window.output[2]["encrypted_content"], "opaque+/=bytes-雪");
}

#[tokio::test(flavor = "current_thread")]
async fn compact_reports_http_errors_without_accepting_error_bodies() {
    let body = br#"{"error":{"message":"bad compact input"}}"#;
    let fixture = spawn_fixture("400 Bad Request", body, None).await;
    let result = fixture_client(&fixture.address)
        .compact("fixture-model", &input_history())
        .await;
    let _ = fixture.server.await.unwrap();
    assert!(matches!(
        result,
        Err(ResponsesError::Upstream { status: 400, ref message }) if message == "bad compact input"
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn compact_rejects_wrong_response_object_without_requiring_status() {
    let body = br#"{"id":"resp_1","object":"response","created_at":1,"output":[]}"#;
    let fixture = spawn_fixture("200 OK", body, None).await;
    let result = fixture_client(&fixture.address)
        .compact("fixture-model", &input_history())
        .await;
    let _ = fixture.server.await.unwrap();
    assert!(matches!(result, Err(ResponsesError::InvalidJson(_))));
}

#[tokio::test(flavor = "current_thread")]
async fn compact_rejects_oversized_response_from_content_length_before_reading() {
    let fixture = spawn_fixture(
        "200 OK",
        b"",
        Some(MAX_RESPONSES_BODY_BYTES + 1),
    )
    .await;
    let result = fixture_client(&fixture.address)
        .compact("fixture-model", &input_history())
        .await;
    let _ = fixture.server.await.unwrap();
    assert!(matches!(
        result,
        Err(ResponsesError::ResponseTooLarge { max: MAX_RESPONSES_BODY_BYTES })
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn compact_rejects_malformed_compaction_envelope() {
    let body = br#"{"id":"resp_1","object":"response.compaction","created_at":"later","output":[]}"#;
    let fixture = spawn_fixture("200 OK", body, None).await;
    let result = fixture_client(&fixture.address)
        .compact("fixture-model", &input_history())
        .await;
    let _ = fixture.server.await.unwrap();
    assert!(matches!(result, Err(ResponsesError::InvalidJson(_))));
}

#[tokio::test(flavor = "current_thread")]
async fn compact_enforces_input_count_and_byte_bounds_before_network_io() {
    let mut too_many = Vec::with_capacity(MAX_RESPONSES_INPUT_MESSAGES + 1);
    too_many.resize_with(MAX_RESPONSES_INPUT_MESSAGES + 1, || ResponsesItem::Text {
        role: ResponsesRole::User,
        content: "x".into(),
    });
    let client = fixture_client("127.0.0.1:1");
    assert!(matches!(
        client.compact("fixture-model", &too_many).await,
        Err(ResponsesError::TooManyMessages { .. })
    ));

    let too_large = vec![ResponsesItem::Text {
        role: ResponsesRole::User,
        content: "x".repeat(MAX_RESPONSES_INPUT_BYTES + 1),
    }];
    assert!(matches!(
        client.compact("fixture-model", &too_large).await,
        Err(ResponsesError::InputTooLarge { .. })
    ));
}
