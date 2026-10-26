//! Integration tests for `openai_device_auth` using real public API surface
//! and loopback HTTP stubs. No real OpenAI service or credentials involved.
//!
//! Exercise the provider crate's exported transport implementation.
#![forbid(unsafe_code)]

use opencode_rk_providers::openai_device_auth;

use openai_device_auth::{
    authorize_device, begin_device_authorization, exchange_authorization_code, refresh, AuthTransport,
    DeviceAuthorization, DeviceEndpoints, OpenAiCredential, OpenAiDeviceAuthError,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::watch;

// --- Helpers ---

async fn join_server(mut server: tokio::task::JoinHandle<()>) {
    match tokio::time::timeout(std::time::Duration::from_secs(25), &mut server).await {
        Ok(result) => result.expect("loopback server failed"),
        Err(_) => {
            server.abort();
            let _ = server.await;
            panic!("loopback server exceeded its deadline");
        }
    }
}

fn local_endpoints(port: u16) -> DeviceEndpoints {
    let base = format!("http://127.0.0.1:{port}");
    DeviceEndpoints::for_local_test(&base).expect("valid local base")
}

async fn read_http_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut raw = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&raw[..pos]).unwrap();
            let cl = headers
                .lines()
                .find_map(|l| {
                    let (n, v) = l.split_once(':')?;
                    if n.eq_ignore_ascii_case("content-length") {
                        v.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if raw.len() >= pos + 4 + cl {
                return String::from_utf8(raw).unwrap();
            }
        }
        let n = tokio::time::timeout(std::time::Duration::from_secs(5), socket.read(&mut buf))
            .await.expect("request read exceeded deadline").unwrap();
        assert!(n > 0, "request stream ended prematurely");
        raw.extend_from_slice(&buf[..n]);
        assert!(raw.len() <= 128 * 1024, "fixture request exceeded its byte budget");
    }
}

async fn write_response(socket: &mut tokio::net::TcpStream, status: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(resp.as_bytes()).await.unwrap();
    let _ = socket.shutdown().await;
}

fn request_path(raw: &str) -> &str {
    let first_line = raw.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1]
    } else {
        ""
    }
}

fn test_device(port: u16) -> DeviceAuthorization {
    DeviceAuthorization {
        device_auth_id: "dev-auth-123".to_string(),
        user_code: "UC-001".to_string(),
        verification_url: format!("http://127.0.0.1:{port}/codex/device"),
        interval: std::time::Duration::from_secs(1),
    }
}

// --- Test 1: begin_device_authorization success + Debug redaction ---

#[test]
fn local_endpoint_policy_rejects_external_hosts_and_non_root_urls() {
    for invalid in [
        "https://127.0.0.1:8080", "http://localhost:8080", "http://192.0.2.1:8080",
        "http://127.0.0.1:8080/other", "http://name@127.0.0.1:8080",
        "http://name:password@127.0.0.1:8080", "http://127.0.0.1:8080/?token=x",
        "http://127.0.0.1:8080/#fragment",
    ] {
        assert!(matches!(DeviceEndpoints::for_local_test(invalid), Err(OpenAiDeviceAuthError::EndpointPolicy)));
    }
    assert!(DeviceEndpoints::for_local_test("http://127.0.0.1:8080").is_ok());
    assert!(DeviceEndpoints::for_local_test("http://127.0.0.1:8080/").is_ok());
}

#[tokio::test]
async fn authorization_does_not_follow_redirects_to_another_origin() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoints = local_endpoints(listener.local_addr().unwrap().port());
    let location = format!("http://127.0.0.1:{}/capture", destination.local_addr().unwrap().port());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_http_request(&mut socket).await;
        let response = format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.shutdown().await.unwrap();
    });
    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);
    assert!(matches!(begin_device_authorization(&transport, &endpoints, &mut rx).await, Err(OpenAiDeviceAuthError::Request)));
    join_server(server).await;
    assert!(tokio::time::timeout(std::time::Duration::from_millis(100), destination.accept()).await.is_err(), "redirect destination must receive no request");
}

#[tokio::test]
async fn begin_auth_success_and_debug_redaction() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);

    let body = r#"{"device_auth_id":"dev-1234567890","user_code":"ABCD-1234","interval":"5"}"#;

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let raw = read_http_request(&mut socket).await;
        assert!(raw.contains("POST "), "expected POST");
        assert!(
            raw.contains("content-type: application/json"),
            "missing content-type header"
        );
        assert!(raw.contains("user-agent: opencode"), "missing user-agent");
        assert!(
            raw.contains(r#"{"client_id":"app_EMoamEEZ73f0CkXaXp7hrann"}"#),
            "missing client_id in body"
        );
        write_response(&mut socket, "200 OK", body).await;
    });

    let transport = AuthTransport::new().unwrap();
    let (tx, mut rx) = watch::channel(false);

    let result = begin_device_authorization(&transport, &endpoints, &mut rx).await;
    join_server(server).await;

    let auth = result.expect("authorization should succeed");
    assert_eq!(auth.device_auth_id, "dev-1234567890");
    assert_eq!(auth.user_code, "ABCD-1234");
    assert_eq!(auth.interval.as_secs(), 5);
    assert_eq!(
        auth.verification_url,
        format!("http://127.0.0.1:{port}/codex/device")
    );

    let dbg = format!("{:?}", auth);
    assert!(!dbg.contains("dev-1234567890"), "device_auth_id leaked");
    assert!(!dbg.contains("ABCD-1234"), "user_code leaked");
    assert!(dbg.contains("verification_url"), "missing field name");

    let _ = tx.send(true);
}

// --- Test 2: begin_device_authorization cancel before start ---

#[tokio::test]
async fn begin_auth_cancel_before_start() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(true);

    let result = begin_device_authorization(&transport, &local_endpoints(port), &mut rx).await;
    drop(listener);
    assert!(matches!(result, Err(OpenAiDeviceAuthError::Cancelled)));
}

// --- Test 3: begin_device_authorization HTTP error ---

#[tokio::test]
async fn begin_auth_http_error_returns_request_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let raw = read_http_request(&mut socket).await;
        assert!(raw.contains("POST "), "expected POST");
        write_response(&mut socket, "500 Internal Server Error", r#"{"error":"server"}"#).await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result = begin_device_authorization(&transport, &endpoints, &mut rx).await;
    join_server(server).await;

    assert!(matches!(result, Err(OpenAiDeviceAuthError::Request)));
}

// --- Test 4: begin_device_authorization invalid JSON ---

#[tokio::test]
async fn begin_auth_invalid_json_returns_invalid_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_http_request(&mut socket).await;
        write_response(&mut socket, "200 OK", "not-json{{{").await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result = begin_device_authorization(&transport, &endpoints, &mut rx).await;
    join_server(server).await;

    assert!(matches!(result, Err(OpenAiDeviceAuthError::InvalidResponse)));
}

// --- Test 5: begin_device_authorization oversized response ---

#[tokio::test]
async fn begin_auth_oversized_response_returns_body_too_large() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);

    let huge = "x".repeat(openai_device_auth::MAX_RESPONSE_BYTES + 1);
    let body = format!(
        r#"{{"device_auth_id":"{}","user_code":"ABCD-1234","interval":"5"}}"#,
        huge
    );

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_http_request(&mut socket).await;
        write_response(&mut socket, "200 OK", &body).await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result = begin_device_authorization(&transport, &endpoints, &mut rx).await;
    join_server(server).await;

    assert!(matches!(result, Err(OpenAiDeviceAuthError::BodyTooLarge)));
}

// --- Test 6: begin_device_authorization empty fields ---

#[tokio::test]
async fn begin_auth_empty_fields_return_invalid_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_http_request(&mut socket).await;
        write_response(
            &mut socket,
            "200 OK",
            r#"{"device_auth_id":"","user_code":"","interval":"5"}"#,
        )
        .await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result = begin_device_authorization(&transport, &endpoints, &mut rx).await;
    join_server(server).await;

    assert!(matches!(result, Err(OpenAiDeviceAuthError::InvalidResponse)));
}

// --- Test 7: authorize_device polling success (pending -> token -> exchange) ---

#[tokio::test]
async fn authorize_device_poll_pending_then_success() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);
    let device = test_device(port);

    let token_resp = r#"{"authorization_code":"auth-code-xyz","code_verifier":"verifier-abc"}"#;
    let token_json = r#"{"id_token":"h.jwt.payload","access_token":"access-tok-123","refresh_token":"refresh-tok-456","expires_in":3600}"#;

    let server = tokio::spawn(async move {
        // First poll: pending (403)
        let (mut socket, _) = listener.accept().await.unwrap();
        let raw = read_http_request(&mut socket).await;
        assert_eq!(request_path(&raw), "/api/accounts/deviceauth/token");
        write_response(&mut socket, "403 Forbidden", r#"{}"#).await;

        // Second poll: success with device token
        let (mut socket, _) = listener.accept().await.unwrap();
        let raw = read_http_request(&mut socket).await;
        assert_eq!(request_path(&raw), "/api/accounts/deviceauth/token");
        assert!(raw.contains("dev-auth-123"));
        assert!(raw.contains("UC-001"));
        write_response(&mut socket, "200 OK", token_resp).await;

        // Third request: oauth/token exchange
        let (mut socket, _) = listener.accept().await.unwrap();
        let raw = read_http_request(&mut socket).await;
        assert_eq!(request_path(&raw), "/oauth/token");
        assert!(raw.contains("grant_type=authorization_code"));
        assert!(raw.contains("code=auth-code-xyz"));
        assert!(raw.contains("code_verifier=verifier-abc"));
        assert!(raw.contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann"));
        assert!(raw.contains("redirect_uri="));
        write_response(&mut socket, "200 OK", token_json).await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result = authorize_device(&transport, &endpoints, &device, &mut rx).await;
    join_server(server).await;

    let cred = result.expect("authorization should succeed");
    assert_eq!(cred.access, "access-tok-123");
    assert_eq!(cred.refresh, "refresh-tok-456");

    // Debug redaction
    let dbg = format!("{:?}", cred);
    assert!(!dbg.contains("access-tok-123"), "access token leaked");
    assert!(!dbg.contains("refresh-tok-456"), "refresh token leaked");

    // ExpiresAtMs computation (expires_in=3600 seconds)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert!(cred.expires_at_ms > now);
    let expected_min = now + 3_599_000;
    let expected_max = now + 3_601_000;
    assert!(
        cred.expires_at_ms >= expected_min && cred.expires_at_ms <= expected_max,
        "expires_at_ms {} not in [{}, {}]",
        cred.expires_at_ms,
        expected_min,
        expected_max
    );

    // account_id should be None for a simple token without JWT claims
    assert_eq!(cred.account_id, None);
}

// --- Test 8: authorize_device cancel during pending polls ---

#[tokio::test]
async fn authorize_device_cancel_during_pending_poll() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);
    let device = test_device(port);

    let (tx, mut rx) = watch::channel(false);

    // Cancel only after the real polling request has reached the fixture.
    // This handshake is independent of scheduler timing and host load.
    let server = tokio::spawn(async move {
        // First: pending poll
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_http_request(&mut socket).await;
        write_response(&mut socket, "403 Forbidden", r#"{}"#).await;
        tx.send(true).unwrap();
    });

    let transport = AuthTransport::new().unwrap();
    let result = authorize_device(&transport, &endpoints, &device, &mut rx).await;
    join_server(server).await;

    assert!(matches!(result, Err(OpenAiDeviceAuthError::Cancelled)));
}

// --- Test 9: authorize_device denied on non-403/non-404 status ---

#[tokio::test]
async fn authorize_device_denied_on_non_pending_status() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);
    let device = test_device(port);

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_http_request(&mut socket).await;
        write_response(&mut socket, "401 Unauthorized", r#"{}"#).await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result = authorize_device(&transport, &endpoints, &device, &mut rx).await;
    join_server(server).await;

    assert!(matches!(result, Err(OpenAiDeviceAuthError::Denied)));
}

// --- Test 10: authorize_device invalid token response (empty code) ---

#[tokio::test]
async fn authorize_device_invalid_token_response_empty_code() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);
    let device = test_device(port);

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_http_request(&mut socket).await;
        write_response(
            &mut socket,
            "200 OK",
            r#"{"authorization_code":"","code_verifier":"verifier-abc"}"#,
        )
        .await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result = authorize_device(&transport, &endpoints, &device, &mut rx).await;
    join_server(server).await;

    assert!(matches!(result, Err(OpenAiDeviceAuthError::InvalidResponse)));
}

// --- Test 11: authorize_device oversized token endpoint response ---

#[tokio::test]
async fn authorize_device_oversized_token_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);
    let device = test_device(port);

    let huge = "x".repeat(openai_device_auth::MAX_RESPONSE_BYTES + 1);
    let body = format!(
        r#"{{"authorization_code":"auth-code-xyz","code_verifier":"verifier-abc","extra":"{huge}"}}"#
    );

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_http_request(&mut socket).await;
        write_response(&mut socket, "200 OK", &body).await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result = authorize_device(&transport, &endpoints, &device, &mut rx).await;
    join_server(server).await;

    assert!(matches!(result, Err(OpenAiDeviceAuthError::BodyTooLarge)));
}

// --- Test 12: exchange_authorization_code success ---

#[tokio::test]
async fn exchange_authorization_code_success() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);

    let token_json = r#"{"id_token":"h.eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhY3QtbG9jYWwifQ.sig","access_token":"access-tok-789","refresh_token":"refresh-tok-012","expires_in":7200}"#;

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let raw = read_http_request(&mut socket).await;
        assert_eq!(request_path(&raw), "/oauth/token");
        assert!(raw.contains("grant_type=authorization_code"));
        assert!(raw.contains("code=my-auth-code"));
        assert!(raw.contains("code_verifier=my-verifier"));
        assert!(raw.contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann"));
        write_response(&mut socket, "200 OK", token_json).await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result =
        exchange_authorization_code(&transport, &endpoints, "my-auth-code", "my-verifier", &mut rx)
            .await;
    join_server(server).await;

    let cred = result.expect("exchange should succeed");
    assert_eq!(cred.access, "access-tok-789");
    assert_eq!(cred.refresh, "refresh-tok-012");

    // Debug redaction
    let dbg = format!("{:?}", cred);
    assert!(!dbg.contains("access-tok-789"), "access token leaked");
    assert!(!dbg.contains("refresh-tok-012"), "refresh token leaked");

    // account_id extraction from JWT (chatgpt_account_id claim in payload)
    assert_eq!(cred.account_id, Some("act-local".to_string()));

    // expires_at_ms (expires_in=7200)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert!(cred.expires_at_ms > now);
    let expected_min = now + 7_199_000;
    let expected_max = now + 7_201_000;
    assert!(
        cred.expires_at_ms >= expected_min && cred.expires_at_ms <= expected_max,
        "expires_at_ms {} not in [{}, {}]",
        cred.expires_at_ms,
        expected_min,
        expected_max
    );
}

// --- Test 13: exchange_authorization_code HTTP error ---

#[tokio::test]
async fn exchange_authorization_code_http_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_http_request(&mut socket).await;
        write_response(&mut socket, "400 Bad Request", r#"{"error":"invalid_grant"}"#).await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result =
        exchange_authorization_code(&transport, &endpoints, "bad-code", "bad-verifier", &mut rx)
            .await;
    join_server(server).await;

    assert!(matches!(result, Err(OpenAiDeviceAuthError::Request)));
}

// --- Test 14: exchange_authorization_code invalid expiry ---

#[tokio::test]
async fn exchange_authorization_code_invalid_expiry() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = read_http_request(&mut socket).await;
        write_response(
            &mut socket,
            "200 OK",
            r#"{"id_token":"h.jwt.payload","access_token":"access-tok-789","refresh_token":"refresh-tok-012","expires_in":999999999}"#,
        )
        .await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result =
        exchange_authorization_code(&transport, &endpoints, "my-code", "my-verifier", &mut rx).await;
    join_server(server).await;

    assert!(matches!(result, Err(OpenAiDeviceAuthError::InvalidExpiry)));
}

// --- Test 15: refresh token success ---

#[tokio::test]
async fn refresh_token_success() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);

    let token_json = r#"{"id_token":"h.eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhY3QtcmVmIn0.sig","access_token":"new-access","refresh_token":"new-refresh","expires_in":1800}"#;

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let raw = read_http_request(&mut socket).await;
        assert_eq!(request_path(&raw), "/oauth/token");
        assert!(raw.contains("grant_type=refresh_token"));
        assert!(raw.contains("refresh_token=old-refresh-token"));
        assert!(raw.contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann"));
        write_response(&mut socket, "200 OK", token_json).await;
    });

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result = refresh(&transport, &endpoints, "old-refresh-token", &mut rx).await;
    join_server(server).await;

    let cred = result.expect("refresh should succeed");
    assert_eq!(cred.access, "new-access");
    assert_eq!(cred.refresh, "new-refresh");

    let dbg = format!("{:?}", cred);
    assert!(!dbg.contains("new-access"), "new access token leaked");
    assert!(!dbg.contains("new-refresh"), "new refresh token leaked");

    // account_id extraction from JWT
    assert_eq!(cred.account_id, Some("act-ref".to_string()));
}

// --- Test 16: refresh empty token returns error ---

#[tokio::test]
async fn refresh_empty_token_returns_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoints = local_endpoints(port);

    let transport = AuthTransport::new().unwrap();
    let (_tx, mut rx) = watch::channel(false);

    let result = refresh(&transport, &endpoints, "", &mut rx).await;
    drop(listener);

    assert!(matches!(result, Err(OpenAiDeviceAuthError::InvalidResponse)));
}

// --- Test 17: credential serialize redacts secrets ---

#[test]
fn credential_serialize_redacts_secrets() {
    let cred = OpenAiCredential {
        access: "secret-access-tok".to_string(),
        refresh: "secret-refresh-tok".to_string(),
        expires_at_ms: 999_999_999,
        account_id: Some("acct-123".to_string()),
    };

    let json = serde_json::to_string(&cred).expect("serialize");
    let value: serde_json::Value = serde_json::from_str(&json).expect("parse json");

    assert_eq!(value["expires_at_ms"], 999_999_999);
    assert!(
        !json.contains("secret-access-tok"),
        "access token in serialized output"
    );
    assert!(
        !json.contains("secret-refresh-tok"),
        "refresh token in serialized output"
    );
    assert!(
        !json.contains("acct-123"),
        "account_id in serialized output"
    );
}
