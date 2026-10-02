//! S1 diagnostic: prove whether one reusable Responses client can make two
//! sequential streaming turns after a `connection: close` tool turn.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use opencode_rk_providers::responses::{
    OpenAiResponsesClient, ResponsesItem, ResponsesRole, ResponsesStreamEvent,
};
use serde_json::Value;
use tokio::time::timeout;

static ENV_LOCK: Mutex<()> = Mutex::new(());

const CALL_ID: &str = "call_s1";
const FIRST_BODY: &str = concat!(
    "event: response.output_item.done\n",
    "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_s1\",\"name\":\"bash\",\"arguments\":\"{\\\"command\\\":\\\"printf ok\\\"}\"}}\n\n",
    "event: response.completed\n",
    "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n"
);
const SECOND_BODY: &str = concat!(
    "event: response.output_text.delta\n",
    "data: {\"type\":\"response.output_text.delta\",\"delta\":\"done\"}\n\n",
    "event: response.completed\n",
    "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n"
);

struct Fixture {
    base_url: String,
    requests: mpsc::Receiver<(usize, Value, Instant)>,
    join: Option<thread::JoinHandle<()>>,
}

fn fixture() -> Fixture {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback fixture");
    listener
        .set_nonblocking(true)
        .expect("set nonblocking listener");
    let address = listener.local_addr().expect("fixture address");
    let (send, requests) = mpsc::channel();
    let join = thread::spawn(move || {
        let started = Instant::now();
        for number in 1..=2 {
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if started.elapsed() > Duration::from_secs(5) {
                            return;
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => return,
                }
            };
            let (body, stream) = read_request(stream).expect("bounded fixture request");
            let request: Value = serde_json::from_slice(&body).expect("request JSON");
            send.send((number, request, Instant::now()))
                .expect("record request");
            write_response(stream, if number == 1 { FIRST_BODY } else { SECOND_BODY });
        }
    });
    Fixture {
        base_url: format!("http://{address}"),
        requests,
        join: Some(join),
    }
}

fn read_request(mut stream: TcpStream) -> std::io::Result<(Vec<u8>, TcpStream)> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut bytes = Vec::with_capacity(4096);
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "headers",
            ));
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > 256 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request cap",
            ));
        }
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let length = headers
        .lines()
        .find_map(|line| {
            let lower = line.to_ascii_lowercase();
            lower
                .strip_prefix("content-length:")
                .map(str::trim)
                .map(str::to_owned)
        })
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "content length"))?;
    if length > 256 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "body cap",
        ));
    }
    while bytes.len() < header_end + length {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "body",
            ));
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok((bytes[header_end..header_end + length].to_vec(), stream))
}

fn write_response(mut stream: TcpStream, body: &str) {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .expect("fixture headers");
    stream.write_all(body.as_bytes()).expect("fixture body");
    stream.flush().expect("fixture flush");
}

fn configure(base_url: &str) -> OpenAiResponsesClient {
    unsafe {
        std::env::set_var("OPENAI_BASE_URL", base_url);
        std::env::set_var("OPENAI_API_KEY", "diagnostic-only");
        std::env::set_var("OPENAI_TIMEOUT_SECS", "2");
    }
    OpenAiResponsesClient::from_env().expect("disposable provider config")
}

async fn first_turn(
    client: &OpenAiResponsesClient,
) -> opencode_rk_providers::responses::OpenAiResponsesStream {
    let mut stream = timeout(
        Duration::from_secs(2),
        client.stream(
            "gpt-diagnostic",
            "low",
            &[opencode_rk_providers::responses::ResponsesInput::new(
                ResponsesRole::User,
                "call tool",
            )],
        ),
    )
    .await
    .expect("first request deadline")
    .expect("first request");
    assert!(
        matches!(timeout(Duration::from_secs(2), stream.next_event()).await.expect("function deadline").expect("function event"), Some(ResponsesStreamEvent::FunctionCall { call_id, .. }) if call_id == CALL_ID)
    );
    assert!(matches!(
        timeout(Duration::from_secs(2), stream.next_event())
            .await
            .expect("completion deadline")
            .expect("completion event"),
        Some(ResponsesStreamEvent::Completed { .. })
    ));
    stream
}

async fn second_turn(client: &OpenAiResponsesClient) -> String {
    let items = vec![
        ResponsesItem::FunctionCall {
            call_id: CALL_ID.into(),
            name: "bash".into(),
            arguments: r#"{"command":"printf ok"}"#.into(),
        },
        ResponsesItem::FunctionCallOutput {
            call_id: CALL_ID.into(),
            output: "ok".into(),
        },
    ];
    let mut stream = timeout(
        Duration::from_secs(2),
        client.stream_with_tools("gpt-diagnostic", "low", &items, &[]),
    )
    .await
    .expect("second request deadline")
    .expect("second request");
    let mut text = String::new();
    loop {
        match timeout(Duration::from_secs(2), stream.next_event())
            .await
            .expect("second stream deadline")
            .expect("second stream event")
        {
            Some(ResponsesStreamEvent::OutputTextDelta(delta)) => text.push_str(&delta),
            Some(ResponsesStreamEvent::Completed { .. }) => return text,
            Some(_) => {}
            None => panic!("second stream ended before completion"),
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn reusable_client_two_rounds_after_closed_tool_stream() {
    let _lock = ENV_LOCK.lock().expect("environment lock");
    let fixture = fixture();
    let client = configure(&fixture.base_url);
    let first = first_turn(&client).await;
    drop(first);
    let text = second_turn(&client).await;
    assert_eq!(text, "done");
    let first_request = fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("request 1");
    let second_request = fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("request 2");
    assert_eq!(first_request.0, 1);
    assert_eq!(second_request.0, 2);
    assert_eq!(second_request.1["input"][0]["type"], "function_call");
    assert_eq!(second_request.1["input"][0]["call_id"], CALL_ID);
    assert_eq!(second_request.1["input"][1]["type"], "function_call_output");
    assert_eq!(second_request.1["input"][1]["call_id"], CALL_ID);
    eprintln!(
        "S1 provider diagnostic: ACCEPT/request1 at {:?}, ACCEPT/request2 at {:?}",
        first_request.2, second_request.2
    );
    if let Some(join) = fixture.join {
        join.join().expect("fixture thread");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn completed_first_stream_can_remain_alive_while_second_opens() {
    let _lock = ENV_LOCK.lock().expect("environment lock");
    let fixture = fixture();
    let client = configure(&fixture.base_url);
    let first = first_turn(&client).await;
    let text = second_turn(&client).await;
    assert_eq!(text, "done");
    drop(first);
    let _ = fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("request 1");
    let second = fixture
        .requests
        .recv_timeout(Duration::from_secs(1))
        .expect("request 2");
    assert_eq!(second.1["input"][1]["type"], "function_call_output");
    if let Some(join) = fixture.join {
        join.join().expect("fixture thread");
    }
}
