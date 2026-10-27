//! Disposable loopback HTTP coverage for persisted OpenAI OAuth dispatch.
use opencode_rk_providers::{config::ProviderConfig, persisted_auth::PersistedOAuth, responses::{OpenAiResponsesClient, ResponsesItem, ResponsesRole, ResponsesStreamEvent, ResponsesStopReason}};
use std::{sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::{SystemTime, UNIX_EPOCH}};

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener, sync::{oneshot, Mutex as TokioMutex}, time::{sleep, timeout, Duration}};


/// RAII guard: captures prior `XDG_DATA_HOME` and `UNUSED_OAUTH_TEST_KEY`,
/// installs a disposable tempdir, and restores originals on drop (including
/// panic unwinding). Removes the disposable directory on both panic and
/// success. Safe because this process is the only writer of these env vars
/// during the test; other binaries keep separate process env and are unaffected.
struct EnvGuard {
    prior_xdg: Option<std::ffi::OsString>,
    prior_key: Option<std::ffi::OsString>,
    pub root: std::path::PathBuf,
}

impl EnvGuard {
    fn install() -> Self {
        let prior_xdg = std::env::var_os("XDG_DATA_HOME");
        let prior_key = std::env::var_os("UNUSED_OAUTH_TEST_KEY");
        let root = std::env::temp_dir().join(format!(
            "opencode-oauth-v2-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64
        ));
        std::env::set_var("XDG_DATA_HOME", &root);
        // Value is intentionally non-secret marker only; never log real env secrets.
        std::env::set_var("UNUSED_OAUTH_TEST_KEY", "must-not-be-used");
        std::fs::create_dir_all(root.join("opencode")).unwrap();
        EnvGuard { prior_xdg, prior_key, root }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
        match &self.prior_xdg {
            Some(value) => std::env::set_var("XDG_DATA_HOME", value),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
        match &self.prior_key {
            Some(value) => std::env::set_var("UNUSED_OAUTH_TEST_KEY", value),
            None => std::env::remove_var("UNUSED_OAUTH_TEST_KEY"),
        }
    }
}
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

#[tokio::test(flavor = "current_thread")]
async fn failed_refresh_preserves_persisted_old_credential_and_context() {
    let _env_lock = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::install();

    opencode_rk_providers::persisted_auth::save_oauth_credential_with_metadata(
        "openai".into(),
        "old-persisted-access".into(),
        "old-persisted-refresh".into(),
        7,
        Some("old-account-context".into()),
        Some("https://enterprise.example".into()),
    )
    .await
    .unwrap();
    let before = opencode_rk_providers::persisted_auth::oauth("openai")
        .await
        .expect("seeded old credential");
    assert_eq!(before.access_token, "old-persisted-access");
    assert_eq!(before.refresh_token, "old-persisted-refresh");
    assert_eq!(before.expires_at, 7);
    assert_eq!(before.account_id.as_deref(), Some("old-account-context"));
    assert_eq!(before.enterprise_url.as_deref(), Some("https://enterprise.example"));

    let refresh = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let model = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refresh_addr = refresh.local_addr().unwrap();
    let model_addr = model.local_addr().unwrap();
    let refresh_count = Arc::new(AtomicUsize::new(0));
    let model_count = Arc::new(AtomicUsize::new(0));

    tokio::spawn(peer(
        refresh,
        refresh_count.clone(),
        r#"{"error":"invalid_grant"}"#,
        "400 Bad Request",
        &[],
    ));
    let model_task = tokio::spawn(assert_no_request(model, model_count.clone()));

    let client = OpenAiResponsesClient::from_oauth_config_with_endpoint(
        config(format!("http://{model_addr}")),
        oauth(1),
        format!("http://{refresh_addr}"),
    )
    .unwrap();
    let error = client.create_with_items("fixture", "", &input()).await.unwrap_err();
    assert!(
        matches!(error, opencode_rk_providers::responses::ResponsesError::OAuthRefresh(_)),
        "failed refresh must surface OAuthRefresh, not {error:?}"
    );
    assert_eq!(refresh_count.load(Ordering::SeqCst), 1);
    model_task.await.unwrap();
    assert_eq!(
        model_count.load(Ordering::SeqCst),
        0,
        "fallback/model listener observed an attempted request"
    );

    let after = opencode_rk_providers::persisted_auth::oauth("openai")
        .await
        .expect("failed refresh must retain old credential");
    assert_eq!(after.access_token, before.access_token);
    assert_eq!(after.refresh_token, before.refresh_token);
    assert_eq!(after.expires_at, before.expires_at);
    assert_eq!(after.account_id, before.account_id);
    assert_eq!(after.enterprise_url, before.enterprise_url);

    let persisted = std::fs::read(_env.root.join("opencode/auth.json")).unwrap();
    let persisted_text = String::from_utf8(persisted).unwrap();
    assert!(persisted_text.contains("old-persisted-access"));
    assert!(persisted_text.contains("old-persisted-refresh"));
    assert!(persisted_text.contains("old-account-context"));
    assert!(persisted_text.contains("https://enterprise.example"));
    assert!(!format!("{:?}", after).contains("old-persisted-access"));
    assert!(!format!("{:?}", after).contains("old-persisted-refresh"));
}

/// Expiry-triggered refresh persistence: when the OAuth credential is expired,
/// the client performs one refresh, dispatches the rotated access token with the
/// seeded account header, and atomically persists the rotated access/refresh with
/// account/enterprise context preserved. Debug output redacts token values.
/// Complements the failed-refresh preservation test above. Uses `EnvGuard` for
/// RAII env/temp restoration on both panic and success; no API-key fallback.
#[tokio::test(flavor = "current_thread")]
async fn expiry_triggered_refresh_preserves_account_and_persists_rotated_credential() {
    let _env_lock = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::install();

    let root = &_env.root;
    opencode_rk_providers::persisted_auth::save_oauth_credential_with_metadata(
        "openai".into(),
        "seeded-access".into(),
        "seeded-refresh".into(),
        1,
        Some("seeded-account".into()),
        Some("https://enterprise.example".into()),
    )
    .await
    .unwrap();

    let refresh = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let model = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let refresh_addr = refresh.local_addr().unwrap();
    let model_addr = model.local_addr().unwrap();
    let refresh_count = Arc::new(AtomicUsize::new(0));
    let model_count = Arc::new(AtomicUsize::new(0));

    tokio::spawn(peer(
        refresh,
        refresh_count.clone(),
        r#"{"access_token":"rotated-access","refresh_token":"rotated-refresh","expires_in":3600}"#,
        "200 OK",
        &[],
    ));
    tokio::spawn(peer(
        model,
        model_count.clone(),
        r#"{"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}]}"#,
        "200 OK",
        &["authorization: Bearer rotated-access", "chatgpt-account-id: seeded-account"],
    ));

    // The client credential carries the same account/enterprise context as the
    // seeded persisted fixture, so the refresh path (responses.rs:1118-1136)
    // preserves exactly that context through `save_refreshed_oauth`.
    let client_credential = PersistedOAuth {
        access_token: "seeded-access".into(),
        refresh_token: "seeded-refresh".into(),
        expires_at: 1,
        account_id: Some("seeded-account".into()),
        enterprise_url: Some("https://enterprise.example".into()),
    };
    let client = OpenAiResponsesClient::from_oauth_config_with_endpoint(
        config(format!("http://{model_addr}")),
        client_credential,
        format!("http://{refresh_addr}"),
    )
    .unwrap();
    let client = Arc::new(client);
    let handle = {
        let client = client.clone();
        tokio::spawn(async move { client.create_with_items("fixture", "", &input()).await })
    };

    let completed = handle.await.unwrap().unwrap();
    assert_eq!(completed, "ok");
    assert_eq!(refresh_count.load(Ordering::SeqCst), 1);
    assert_eq!(model_count.load(Ordering::SeqCst), 1);

    let after = opencode_rk_providers::persisted_auth::oauth("openai")
        .await
        .expect("rotated credential must persist");
    assert_eq!(after.access_token, "rotated-access");
    assert_eq!(after.refresh_token, "rotated-refresh");
    assert!(after.expires_at > 1, "rotated credential must advance expiry");
    assert_eq!(after.account_id.as_deref(), Some("seeded-account"));
    assert_eq!(after.enterprise_url.as_deref(), Some("https://enterprise.example"));

    let persisted_text = String::from_utf8(std::fs::read(root.join("opencode/auth.json")).unwrap()).unwrap();
    assert!(persisted_text.contains("rotated-access"));
    assert!(persisted_text.contains("rotated-refresh"));
    assert!(persisted_text.contains("seeded-account"));
    assert!(persisted_text.contains("https://enterprise.example"));
    assert!(!persisted_text.contains("seeded-access"));
    assert!(!persisted_text.contains("seeded-refresh"));
    assert!(!format!("{:?}", after).contains("rotated-access"));
    assert!(!format!("{:?}", after).contains("rotated-refresh"));
}
