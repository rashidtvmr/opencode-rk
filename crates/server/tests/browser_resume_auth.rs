//! G6 browser-resume auth contract over a real loopback HTTP listener.
#![forbid(unsafe_code)]

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use opencode_rk_catalog::Catalog;
use opencode_rk_server::{AppState, daemon_auth::DaemonAuth, router_with_auth};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use tempfile::TempDir;
use tokio::{
    net::TcpListener,
    task::JoinHandle,
    time::{Duration, sleep},
};

const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const RESUME_SCHEME: &str = "BrowserResume";
const MAX_LEASE_MS: u64 = 15 * 60 * 1000;

struct Server {
    address: SocketAddr,
    task: JoinHandle<()>,
    _storage: TempDir,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    async fn start(auth: DaemonAuth) -> Self {
        Self::start_at(auth, "127.0.0.1:0".parse().unwrap()).await
    }

    async fn start_at(auth: DaemonAuth, address: SocketAddr) -> Self {
        let storage_dir = tempfile::tempdir().expect("temporary storage directory");
        let storage = Storage::open_in_memory(storage_dir.path().join("blobs")).expect("storage");
        let app = router_with_auth(
            AppState {
                sessions: SessionService::new(Arc::new(storage)),
                catalog: Arc::new(Catalog::default()),
            },
            Some(auth),
        );
        let listener = TcpListener::bind(address).await.expect("loopback bind");
        let address = listener.local_addr().expect("listener address");
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test router");
        });
        Self {
            address,
            task,
            _storage: storage_dir,
        }
    }

    fn origin(&self) -> String {
        format!("http://{}", self.address)
    }

    async fn stop(&mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}

struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

async fn request(
    address: SocketAddr,
    method: &str,
    path: &str,
    authorization: Option<&str>,
    origin: Option<&str>,
    body: &[u8],
) -> HttpResponse {
    let address_copy = address;
    let method = method.to_owned();
    let path = path.to_owned();
    let authorization = authorization.map(str::to_owned);
    let origin = origin.map(str::to_owned);
    let body = body.to_vec();
    tokio::task::spawn_blocking(move || {
        request_blocking(
            address_copy,
            &method,
            &path,
            authorization.as_deref(),
            origin.as_deref(),
            &body,
        )
    })
    .await
    .expect("bounded loopback request worker")
}

fn request_blocking(
    address: SocketAddr,
    method: &str,
    path: &str,
    authorization: Option<&str>,
    origin: Option<&str>,
    body: &[u8],
) -> HttpResponse {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .expect("connect loopback server");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .expect("write timeout");
    let mut wire = format!(
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(value) = authorization {
        wire.push_str(&format!("Authorization: {value}\r\n"));
    }
    if let Some(value) = origin {
        wire.push_str(&format!("Origin: {value}\r\n"));
    }
    if !body.is_empty() {
        wire.push_str("Content-Type: application/json\r\n");
    }
    wire.push_str("\r\n");
    stream
        .write_all(wire.as_bytes())
        .expect("write request headers");
    stream.write_all(body).expect("write request body");
    // Content-Length completes the request body; avoid a TCP half-close that
    // makes the HTTP/1 server observe EOF before sending its response.
    let mut response = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).expect("read response");
        if read == 0 {
            break;
        }
        assert!(
            response.len() + read <= MAX_RESPONSE_BYTES,
            "bounded HTTP response exceeded"
        );
        response.extend_from_slice(&chunk[..read]);
    }
    let split = response
        .windows(4)
        .position(|value| value == b"\r\n\r\n")
        .expect("HTTP headers");
    let raw_headers = String::from_utf8_lossy(&response[..split]);
    let status = raw_headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .expect("HTTP status");
    let raw_body = &response[split + 4..];
    let body = if raw_headers
        .lines()
        .any(|line| line.eq_ignore_ascii_case("transfer-encoding: chunked"))
    {
        decode_chunked(raw_body)
    } else {
        raw_body.to_vec()
    };
    HttpResponse { status, body }
}

fn decode_chunked(mut input: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    loop {
        let end = input
            .windows(2)
            .position(|value| value == b"\r\n")
            .expect("chunk size");
        let size = usize::from_str_radix(
            std::str::from_utf8(&input[..end])
                .expect("ASCII chunk size")
                .split(';')
                .next()
                .unwrap(),
            16,
        )
        .expect("valid chunk size");
        input = &input[end + 2..];
        if size == 0 {
            break;
        }
        assert!(
            output.len() + size <= MAX_RESPONSE_BYTES,
            "bounded chunked body exceeded"
        );
        assert!(input.len() >= size + 2, "complete response chunk");
        output.extend_from_slice(&input[..size]);
        assert_eq!(&input[size..size + 2], b"\r\n");
        input = &input[size + 2..];
    }
    output
}

fn json(response: &HttpResponse) -> serde_json::Value {
    serde_json::from_slice(&response.body).expect("JSON response")
}
fn resume_auth(token: &str) -> String {
    format!("{RESUME_SCHEME} {token}")
}
fn capability(response: &HttpResponse) -> String {
    json(response)["browser_capability"]
        .as_str()
        .expect("separate browser capability")
        .to_owned()
}
async fn issue(server: &Server, native: &str) -> HttpResponse {
    request(
        server.address,
        "POST",
        "/auth/browser-resume",
        Some(&format!("Bearer {native}")),
        Some(&server.origin()),
        b"",
    )
    .await
}
async fn issue_with_ttl(server: &Server, native: &str, ttl_ms: u64) -> HttpResponse {
    let body = format!("{{\"ttl_ms\":{ttl_ms}}}");
    request(
        server.address,
        "POST",
        "/auth/browser-resume",
        Some(&format!("Bearer {native}")),
        Some(&server.origin()),
        body.as_bytes(),
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn g6_resume_capability_exchanges_after_reload_preserving_server_session_history() {
    let native = DaemonAuth::mint().expect("native daemon auth");
    let native_token = native.token().to_owned();
    let mut server = Server::start(native).await;
    let origin = server.origin();
    let created = request(
        server.address,
        "POST",
        "/api/sessions",
        Some(&format!("Bearer {native_token}")),
        Some(&origin),
        br#"{"title":"survives browser reload"}"#,
    )
    .await;
    assert_eq!(created.status, 201, "native authenticated session creation");
    let session_id = json(&created)["session"]["id"]
        .as_str()
        .expect("session ID")
        .to_owned();
    let appended = request(
        server.address,
        "POST",
        &format!("/api/sessions/{session_id}/messages"),
        Some(&format!("Bearer {native_token}")),
        Some(&origin),
        br#"{"text":"history survives reload"}"#,
    )
    .await;
    assert_eq!(
        appended.status, 201,
        "create session history under daemon auth"
    );

    let issued = issue(&server, &native_token).await;
    assert_eq!(
        issued.status, 201,
        "authenticated launch mints one browser capability"
    );
    let issued_json = json(&issued);
    let first = capability(&issued);
    let expiry = issued_json["expires_at_unix_ms"]
        .as_u64()
        .expect("absolute expiry");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_millis() as u64;
    assert!(
        expiry > now && expiry <= now + MAX_LEASE_MS,
        "absolute lease <= 15 minutes"
    );
    assert_ne!(
        first, native_token,
        "browser capability differs from native master bearer"
    );
    assert_eq!(first.len(), 43, "opaque 32-byte base64url capability");
    assert!(
        first
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    );
    assert_eq!(
        issued_json.as_object().expect("bounded response").len(),
        2,
        "response contains no native credential or session authority"
    );
    let resumed = request(
        server.address,
        "GET",
        "/api/sessions",
        Some(&resume_auth(&first)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(
        resumed.status, 200,
        "browser capability serves existing web session authority"
    );
    assert!(
        json(&resumed)["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["id"] == session_id)
    );
    let messages = request(
        server.address,
        "GET",
        &format!("/api/sessions/{session_id}/messages"),
        Some(&resume_auth(&first)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(
        messages.status, 200,
        "browser capability reads preserved session history"
    );
    assert!(
        json(&messages)["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["body"]["text"] == "history survives reload")
    );

    let exchanged = request(
        server.address,
        "POST",
        "/auth/browser-resume/exchange",
        Some(&resume_auth(&first)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(
        exchanged.status, 200,
        "fresh document exchanges tab capability before session API calls"
    );
    let second = capability(&exchanged);
    assert_ne!(first, second, "exchange rotates capability");
    assert_eq!(
        json(&exchanged)["expires_at_unix_ms"],
        expiry,
        "exchange never extends absolute lifetime"
    );
    let replay = request(
        server.address,
        "GET",
        "/api/sessions",
        Some(&resume_auth(&first)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(replay.status, 403, "exchanged capability cannot replay");
    let resumed_again = request(
        server.address,
        "GET",
        "/api/sessions",
        Some(&resume_auth(&second)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(resumed_again.status, 200);
    assert!(
        json(&resumed_again)["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["id"] == session_id)
    );

    let privileged = request(
        server.address,
        "PUT",
        "/auth/openai",
        Some(&resume_auth(&second)),
        Some(&origin),
        br#"{"type":"api","key":"must-not-reach-handler"}"#,
    )
    .await;
    assert_eq!(
        privileged.status, 403,
        "browser lease cannot authorize native auth/control routes"
    );
    // Permission replies remain operation-scoped and are covered by the
    // frozen S1 broker/HTTP gate. This contract must not blanket-deny the
    // permission route: an authenticated browser UI must be able to answer
    // its actual pending approval, while native auth/control routes remain
    // privileged below.
    let cross_origin_mutation = request(
        server.address,
        "POST",
        "/api/sessions",
        Some(&resume_auth(&second)),
        Some("http://attacker.invalid"),
        br#"{"title":"must not be created"}"#,
    )
    .await;
    assert_eq!(
        cross_origin_mutation.status, 403,
        "origin mismatch rejects before side effects"
    );
    let still_one = request(
        server.address,
        "GET",
        "/api/sessions",
        Some(&resume_auth(&second)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(
        json(&still_one)["sessions"].as_array().unwrap().len(),
        1,
        "denied cross-origin write creates no session"
    );
    let revoked = request(
        server.address,
        "DELETE",
        "/auth/browser-resume",
        Some(&resume_auth(&second)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(revoked.status, 204, "explicit revoke accepted");
    let after_revoke = request(
        server.address,
        "GET",
        "/api/sessions",
        Some(&resume_auth(&second)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(
        after_revoke.status, 403,
        "revoked capability no longer authorizes web API"
    );
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn g6_resume_rejects_missing_malformed_wrong_origin_and_restart_credentials() {
    let native = DaemonAuth::mint().expect("native daemon auth");
    let native_token = native.token().to_owned();
    let mut server = Server::start(native).await;
    let origin = server.origin();
    let missing_native = request(
        server.address,
        "POST",
        "/auth/browser-resume",
        None,
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(
        missing_native.status, 401,
        "missing trusted launch credential rejected"
    );
    let missing_origin_launch = request(
        server.address,
        "POST",
        "/auth/browser-resume",
        Some(&format!("Bearer {native_token}")),
        None,
        b"",
    )
    .await;
    assert_eq!(
        missing_origin_launch.status, 403,
        "missing browser origin cannot mint capability"
    );
    let wrong_launch_origin = request(
        server.address,
        "POST",
        "/auth/browser-resume",
        Some(&format!("Bearer {native_token}")),
        Some("http://attacker.invalid"),
        b"",
    )
    .await;
    assert_eq!(
        wrong_launch_origin.status, 403,
        "wrong-origin mint rejected"
    );
    let malformed = request(
        server.address,
        "POST",
        "/auth/browser-resume/exchange",
        Some(&resume_auth("not-a-capability")),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(malformed.status, 403, "malformed capability rejected");
    let issued = issue(&server, &native_token).await;
    assert_eq!(issued.status, 201);
    let old_capability = capability(&issued);
    let missing_origin_exchange = request(
        server.address,
        "POST",
        "/auth/browser-resume/exchange",
        Some(&resume_auth(&old_capability)),
        None,
        b"",
    )
    .await;
    assert_eq!(
        missing_origin_exchange.status, 403,
        "missing origin cannot exchange capability"
    );
    let wrong_exchange_origin = request(
        server.address,
        "POST",
        "/auth/browser-resume/exchange",
        Some(&resume_auth(&old_capability)),
        Some("http://attacker.invalid"),
        b"",
    )
    .await;
    assert_eq!(
        wrong_exchange_origin.status, 403,
        "wrong-origin exchange rejected"
    );
    let wrong_origin_api = request(
        server.address,
        "GET",
        "/api/sessions",
        Some(&resume_auth(&old_capability)),
        Some("http://attacker.invalid"),
        b"",
    )
    .await;
    assert_eq!(wrong_origin_api.status, 403, "wrong-origin API rejected");
    let valid_after_denials = request(
        server.address,
        "GET",
        "/api/sessions",
        Some(&resume_auth(&old_capability)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(
        valid_after_denials.status, 200,
        "denials do not consume the rightful lease"
    );
    let retained_address = server.address;
    server.stop().await;
    let restarted_auth = DaemonAuth::mint().expect("fresh daemon generation");
    let restarted_native = restarted_auth.token().to_owned();
    let mut restarted = Server::start_at(restarted_auth, retained_address).await;
    let prior_generation = request(
        restarted.address,
        "GET",
        "/api/sessions",
        Some(&resume_auth(&old_capability)),
        Some(&restarted.origin()),
        b"",
    )
    .await;
    assert_eq!(
        prior_generation.status, 403,
        "daemon generation restart invalidates old browser capability"
    );
    let new_generation_native = request(
        restarted.address,
        "GET",
        "/api/sessions",
        Some(&format!("Bearer {restarted_native}")),
        Some(&restarted.origin()),
        b"",
    )
    .await;
    assert_eq!(
        new_generation_native.status, 200,
        "new native bearer remains authoritative"
    );
    restarted.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn g6_resume_lease_count_is_bounded() {
    let native = DaemonAuth::mint().expect("native daemon auth");
    let token = native.token().to_owned();
    let mut server = Server::start(native).await;
    let mut capabilities = std::collections::HashSet::new();
    for index in 0..128 {
        let response = issue(&server, &token).await;
        assert_eq!(
            response.status, 201,
            "lease {index} below documented capacity"
        );
        assert!(response.body.len() <= 1024, "bounded capability response");
        assert!(
            capabilities.insert(capability(&response)),
            "lease {index} must be unique"
        );
    }
    let over_capacity = issue(&server, &token).await;
    assert_eq!(
        over_capacity.status, 429,
        "capacity exhaustion is bounded and explicit"
    );
    let first = capabilities.iter().next().unwrap().clone();
    let revoked = request(
        server.address,
        "DELETE",
        "/auth/browser-resume",
        Some(&resume_auth(&first)),
        Some(&server.origin()),
        b"",
    )
    .await;
    assert_eq!(revoked.status, 204, "revocation succeeds at capacity");
    assert_eq!(
        issue(&server, &token).await.status,
        201,
        "revocation frees one bounded lease slot"
    );
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn g6_resume_expiry_is_enforced_and_invalid_ttl_does_not_issue() {
    let native = DaemonAuth::mint().expect("native daemon auth");
    let token = native.token().to_owned();
    let mut server = Server::start(native).await;
    let origin = server.origin();
    for invalid in [0, MAX_LEASE_MS + 1] {
        let response = issue_with_ttl(&server, &token, invalid).await;
        assert_eq!(
            response.status, 400,
            "invalid requested TTL rejected before issuance"
        );
    }
    let issued = issue_with_ttl(&server, &token, 1_000).await;
    assert_eq!(
        issued.status, 201,
        "small explicit TTL accepted for deterministic expiry contract"
    );
    let expiry = json(&issued)["expires_at_unix_ms"]
        .as_u64()
        .expect("absolute expiry");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert!(
        expiry > now && expiry <= now + 1_000,
        "the explicit lifetime is bounded, not merely reported"
    );
    let cap = capability(&issued);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let current = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        if current > expiry {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "expiry observation deadline"
        );
        sleep(Duration::from_millis(25)).await;
    }
    let expired = request(
        server.address,
        "GET",
        "/api/sessions",
        Some(&resume_auth(&cap)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(
        expired.status, 403,
        "server denies use after actual wall-clock expiry"
    );
    let revoke_expired = request(
        server.address,
        "DELETE",
        "/auth/browser-resume",
        Some(&resume_auth(&cap)),
        Some(&origin),
        b"",
    )
    .await;
    assert_eq!(
        revoke_expired.status, 403,
        "expired capability cannot be revoked as if live"
    );
    server.stop().await;
}
