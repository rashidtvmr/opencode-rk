//! Disposable loopback HTTP coverage for persisted OpenAI OAuth dispatch.
use opencode_rk_providers::{config::ProviderConfig, persisted_auth::PersistedOAuth, responses::{OpenAiResponsesClient, ResponsesItem, ResponsesRole, ResponsesStreamEvent, ResponsesStopReason}};
use std::{sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::{SystemTime, UNIX_EPOCH}};

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener, sync::{oneshot, Mutex as TokioMutex}, time::{sleep, timeout, Duration}};

async fn peer(listener: TcpListener, count: Arc<AtomicUsize>, body: &'static str, status: &'static str, expected: &[&str]) {
    let (mut socket, _) = listener.accept().await.unwrap();
    count.fetch_add(1, Ordering::SeqCst);
    let mut request = vec![0; 32 * 1024];
    let n = socket.read(&mut request).await.unwrap();
    let request = String::from_utf8_lossy(&request[..n]);
    assert!(request.starts_with("POST "));
    for header in expected { assert!(request.lines().any(|line| line.eq_ignore_ascii_case(header)), "missing {header}: {request}"); }
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    socket.write_all(response.as_bytes()).await.unwrap();
}
fn config(url: String) -> ProviderConfig { ProviderConfig { provider_id: "openai".into(), base_url: url, api_key_env: "UNUSED_OAUTH_TEST_KEY".into(), timeout_secs: 3, max_tokens: 128, temperature: 0.0 } }
fn oauth(expires_at: u64) -> PersistedOAuth { PersistedOAuth { access_token: "old-access".into(), refresh_token: "old-refresh".into(), expires_at, account_id: Some("acct-loopback".into()), enterprise_url: None } }
fn now_ms() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().min(u128::from(u64::MAX)) as u64 }
fn input() -> [ResponsesItem; 1] { [ResponsesItem::Text { role: ResponsesRole::User, content: "hello".into() }] }

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_oauth_refreshes_and_dispatches_rotated_access_with_account_header() {
    let _env_guard = ENV_LOCK.lock().unwrap();
    let root = std::env::temp_dir().join(format!("opencode-oauth-{}", now_ms()));
    std::fs::create_dir_all(root.join("opencode")).unwrap(); std::env::set_var("XDG_DATA_HOME", &root);
    let refresh = TcpListener::bind("127.0.0.1:0").await.unwrap(); let model = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refresh_addr = refresh.local_addr().unwrap(); let model_addr = model.local_addr().unwrap();
    let refresh_count = Arc::new(AtomicUsize::new(0)); let model_count = Arc::new(AtomicUsize::new(0));
    tokio::spawn(peer(refresh, refresh_count.clone(), r#"{"access_token":"new-access","refresh_token":"rotated-refresh","expires_in":3600}"#, "200 OK", &[]));
    tokio::spawn(peer(model, model_count.clone(), r#"{"status":"completed","usage":{"input_tokens":12,"output_tokens":3},"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}]}"#, "200 OK", &["authorization: Bearer new-access", "chatgpt-account-id: acct-loopback"]));
    let client = OpenAiResponsesClient::from_oauth_config_with_endpoint(config(format!("http://{model_addr}")), oauth(1), format!("http://{refresh_addr}" )).unwrap();
    let completed = client.create_with_items_and_usage("fixture", "", &input()).await.unwrap();
    assert_eq!(completed.text, "ok");
    assert_eq!(completed.usage.map(|usage| (usage.input_tokens, usage.output_tokens)), Some((12, 3)));
    assert_eq!(refresh_count.load(Ordering::SeqCst), 1); assert_eq!(model_count.load(Ordering::SeqCst), 1);
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("opencode/auth.json")).unwrap()).unwrap();
    assert_eq!(saved["openai"]["access"], "new-access"); assert_eq!(saved["openai"]["refresh"], "rotated-refresh");
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn zero_expiry_openai_refreshes_and_dispatches_rotated_usage_response() {
    let _env_guard = ENV_LOCK.lock().unwrap();
    let root = std::env::temp_dir().join(format!("opencode-oauth-zero-{}", now_ms()));
    std::fs::create_dir_all(root.join("opencode")).unwrap();
    std::env::set_var("XDG_DATA_HOME", &root);
    let refresh = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let model = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refresh_addr = refresh.local_addr().unwrap();
    let model_addr = model.local_addr().unwrap();
    let refresh_count = Arc::new(AtomicUsize::new(0));
    let model_count = Arc::new(AtomicUsize::new(0));
    tokio::spawn(peer(
        refresh,
        refresh_count.clone(),
        r#"{"access_token":"zero-new-access","refresh_token":"zero-rotated-refresh","expires_in":3600}"#,
        "200 OK",
        &[],
    ));
    tokio::spawn(peer(
        model,
        model_count.clone(),
        r#"{"status":"completed","usage":{"input_tokens":5,"output_tokens":2},"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"zero-ok"}]}]}"#,
        "200 OK",
        &["authorization: Bearer zero-new-access", "chatgpt-account-id: acct-loopback"],
    ));
    let client = OpenAiResponsesClient::from_oauth_config_with_endpoint(
        config(format!("http://{model_addr}")),
        oauth(0),
        format!("http://{refresh_addr}"),
    )
    .unwrap();
    let completed = client
        .create_with_items_and_usage("fixture", "", &input())
        .await
        .unwrap();
    assert_eq!(completed.text, "zero-ok");
    assert_eq!(
        completed.usage.map(|usage| (usage.input_tokens, usage.output_tokens)),
        Some((5, 2))
    );
    assert_eq!(refresh_count.load(Ordering::SeqCst), 1);
    assert_eq!(model_count.load(Ordering::SeqCst), 1);
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("opencode/auth.json")).unwrap()).unwrap();
    assert_eq!(saved["openai"]["access"], "zero-new-access");
    assert_eq!(saved["openai"]["refresh"], "zero-rotated-refresh");
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn non_expired_oauth_does_not_refresh_and_api_behavior_has_no_account_header() {
    let refresh = TcpListener::bind("127.0.0.1:0").await.unwrap(); let refresh_addr = refresh.local_addr().unwrap();
    let model = TcpListener::bind("127.0.0.1:0").await.unwrap(); let model_addr = model.local_addr().unwrap();
    let refresh_count = Arc::new(AtomicUsize::new(0)); let model_count = Arc::new(AtomicUsize::new(0));
    tokio::spawn(peer(refresh, refresh_count.clone(), "{}", "500 Internal Server Error", &[]));
    tokio::spawn(peer(model, model_count.clone(), r#"{"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}]}"#, "200 OK", &["authorization: Bearer old-access"]));
    let client = OpenAiResponsesClient::from_oauth_config_with_endpoint(config(format!("http://{model_addr}")), oauth(now_ms() + 3_600_000), format!("http://{refresh_addr}" )).unwrap();
    client.create_with_items("fixture", "", &input()).await.unwrap();
    assert_eq!(refresh_count.load(Ordering::SeqCst), 0); assert_eq!(model_count.load(Ordering::SeqCst), 1);
    drop(refresh_addr);
}

async fn blocking_refresh_peer(
    listener: TcpListener,
    count: Arc<AtomicUsize>,
    started: oneshot::Sender<()>,
    release: oneshot::Receiver<()>,
) {
    let (mut socket, _) = timeout(Duration::from_secs(2), listener.accept()).await.unwrap().unwrap();
    count.fetch_add(1, Ordering::SeqCst);
    let mut request = vec![0; 32 * 1024];
    let n = timeout(Duration::from_secs(2), socket.read(&mut request)).await.unwrap().unwrap();
    let request = String::from_utf8_lossy(&request[..n]);
    assert!(request.starts_with("POST "));
    let _ = started.send(());
    timeout(Duration::from_secs(2), release).await.unwrap().unwrap();
    let body = r#"{"access_token":"rotated-access","expires_in":3600}"#;
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    socket.write_all(response.as_bytes()).await.unwrap();
}

async fn assert_no_request(listener: TcpListener, count: Arc<AtomicUsize>) {
    assert!(timeout(Duration::from_millis(500), listener.accept()).await.is_err());
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

async fn multi_peer(listener: TcpListener, count: Arc<AtomicUsize>, body: &'static str, status: &'static str, expected: &'static [&'static str]) {
    for _ in 0..2 {
        let (mut socket, _) = listener.accept().await.unwrap();
        count.fetch_add(1, Ordering::SeqCst);
        let mut request = vec![0; 32 * 1024];
        let n = socket.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..n]);
        for header in expected { assert!(request.lines().any(|line| line.eq_ignore_ascii_case(header)), "missing {header}: {request}"); }
        let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        socket.write_all(response.as_bytes()).await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_expired_calls_use_one_refresh_and_both_rotated_access_tokens() {
    let _env_guard = ENV_LOCK.lock().unwrap();
    let root = std::env::temp_dir().join(format!("opencode-oauth-concurrent-{}", now_ms()));
    std::fs::create_dir_all(root.join("opencode")).unwrap(); std::env::set_var("XDG_DATA_HOME", &root);
    let refresh = TcpListener::bind("127.0.0.1:0").await.unwrap(); let model = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refresh_addr = refresh.local_addr().unwrap(); let model_addr = model.local_addr().unwrap();
    let refresh_count = Arc::new(AtomicUsize::new(0)); let model_count = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    tokio::spawn(blocking_refresh_peer(refresh, refresh_count.clone(), started_tx, release_rx));
    tokio::spawn(multi_peer(model, model_count.clone(), r#"{"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}]}"#, "200 OK", &["authorization: Bearer rotated-access"]));
    let client = Arc::new(OpenAiResponsesClient::from_oauth_config_with_endpoint(config(format!("http://{model_addr}")), oauth(1), format!("http://{refresh_addr}" )).unwrap());
    let first = { let client = client.clone(); tokio::spawn(async move { client.create_with_items("fixture", "", &input()).await }) };
    timeout(Duration::from_secs(2), started_rx).await.unwrap().unwrap();
    let second = { let client = client.clone(); tokio::spawn(async move { client.create_with_items("fixture", "", &input()).await }) };
    sleep(Duration::from_millis(50)).await;
    assert_eq!(refresh_count.load(Ordering::SeqCst), 1, "overlapping calls must share the in-flight refresh");
    release_tx.send(()).unwrap();
    first.await.unwrap().unwrap(); second.await.unwrap().unwrap();
    assert_eq!(refresh_count.load(Ordering::SeqCst), 1); assert_eq!(model_count.load(Ordering::SeqCst), 2);
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("opencode/auth.json")).unwrap()).unwrap();
    assert_eq!(saved["openai"]["refresh"], "old-refresh");
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn failed_refresh_returns_oauth_error_without_model_dispatch_or_api_fallback() {
    let _env_guard = ENV_LOCK.lock().unwrap();
    let root = std::env::temp_dir().join(format!("opencode-oauth-failed-{}", now_ms()));
    std::fs::create_dir_all(root.join("opencode")).unwrap(); std::env::set_var("XDG_DATA_HOME", &root); std::env::set_var("UNUSED_OAUTH_TEST_KEY", "must-not-be-used");
    let refresh = TcpListener::bind("127.0.0.1:0").await.unwrap(); let model = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refresh_addr = refresh.local_addr().unwrap(); let model_addr = model.local_addr().unwrap();
    let refresh_count = Arc::new(AtomicUsize::new(0)); let model_count = Arc::new(AtomicUsize::new(0));
    tokio::spawn(peer(refresh, refresh_count.clone(), r#"{"error":"invalid_grant"}"#, "400 Bad Request", &[]));
    let model_task = tokio::spawn(assert_no_request(model, model_count.clone()));
    let client = OpenAiResponsesClient::from_oauth_config_with_endpoint(config(format!("http://{model_addr}")), oauth(1), format!("http://{refresh_addr}" )).unwrap();
    let error = client.create_with_items("fixture", "", &input()).await.unwrap_err();
    assert!(matches!(error, opencode_rk_providers::responses::ResponsesError::OAuthRefresh(_)));
    assert_eq!(refresh_count.load(Ordering::SeqCst), 1);
    model_task.await.unwrap();
    assert_eq!(model_count.load(Ordering::SeqCst), 0, "fallback/model listener observed an attempted request");
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn oauth_credential_is_used_by_compact_and_stream_dispatch() {
    let compact = TcpListener::bind("127.0.0.1:0").await.unwrap(); let stream = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let compact_addr = compact.local_addr().unwrap(); let stream_addr = stream.local_addr().unwrap();
    let compact_count = Arc::new(AtomicUsize::new(0)); let stream_count = Arc::new(AtomicUsize::new(0));
    let compact_body = r#"{"object":"response.compaction","id":"cmp","created_at":1700000000,"output":[{"type":"compaction","id":"cmp-item","encrypted_content":"opaque"}]}"#;
    tokio::spawn(peer(compact, compact_count.clone(), compact_body, "200 OK", &["authorization: Bearer old-access", "chatgpt-account-id: acct-loopback"]));
    let stream_body = "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\"}\n\n";
    tokio::spawn(peer(stream, stream_count.clone(), stream_body, "200 OK", &["authorization: Bearer old-access", "chatgpt-account-id: acct-loopback"]));
    let compact_client = OpenAiResponsesClient::from_oauth_config_with_endpoint(config(format!("http://{compact_addr}")), oauth(now_ms() + 3_600_000), "http://127.0.0.1:9").unwrap();
    compact_client.compact("fixture", &input()).await.unwrap();
    let stream_client = OpenAiResponsesClient::from_oauth_config_with_endpoint(config(format!("http://{stream_addr}")), oauth(now_ms() + 3_600_000), "http://127.0.0.1:9").unwrap();
    let mut events = stream_client.stream("fixture", "", &[opencode_rk_providers::responses::ResponsesInput::new(ResponsesRole::User, "hello")]).await.unwrap();
    let mut saw_text = false;
    let mut saw_completed = false;
    while let Some(event) = timeout(Duration::from_secs(2), events.next_event()).await.unwrap().unwrap() {
        match event {
            ResponsesStreamEvent::OutputTextDelta(text) => { saw_text |= text == "ok"; }
            ResponsesStreamEvent::Completed { stop_reason: ResponsesStopReason::Completed } => { saw_completed = true; }
            _ => {}
        }
    }
    assert!(saw_text);
    assert!(saw_completed, "stream fixture must be consumed through response.completed");
    assert_eq!(compact_count.load(Ordering::SeqCst), 1); assert_eq!(stream_count.load(Ordering::SeqCst), 1);
}
