//! OBS-NATIVE-EVENT-TRANSPORT: authenticated `GET /api/event` (+ `/events`
//! alias) SSE transport over the daemon event bus.
//!
//! Envelope is upstream-derived (`/api/event`, `{id, type, properties}`
//! payloads, `server.connected` first, `server.heartbeat` keepalive,
//! terminating `server.instance.disposed`). Auth precedes subscription:
//! missing/wrong bearer rejects before any subscriber is attached. Frames are
//! bounded; a slow consumer overflows into an EOF disconnect; dropping the
//! connection reclaims the subscriber. Buses are router-owned instances:
//! two routers never share streams.
#![forbid(unsafe_code)]

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::Response,
    Router,
};
use futures_util::StreamExt;
use opencode_rk_contracts::{MessageId, SessionId};
use opencode_rk_server::{
    daemon_auth::DaemonAuth,
    event_bus::{BusError, EventBus, ServerEvent},
    protocol_api::{check_protocol_coverage, route_kind, served_routes, Method, RouteKind},
    route_table::{ProtocolRoute, PROTOCOL_ROUTES},
    AppState,
};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn bus() -> EventBus {
    EventBus::new(256)
}

fn auth() -> DaemonAuth {
    DaemonAuth::from_published(TOKEN).expect("valid token")
}

fn app_with_events(sessions: SessionService, auth: Option<DaemonAuth>, events: EventBus) -> Router {
    let state = AppState {
        sessions,
        catalog: Arc::new(opencode_rk_catalog::Catalog::default()),
    };
    opencode_rk_server::router_with_events(state, auth, events)
}

fn in_memory_sessions() -> SessionService {
    let storage_dir = tempfile::tempdir().expect("temporary storage directory");
    let storage = Storage::open_in_memory(storage_dir.path().join("blobs")).expect("storage");
    SessionService::new(Arc::new(storage))
}

async fn get_event(app: Router, auth_header: Option<&str>, url: &str) -> Response<Body> {
    let mut builder = Request::builder().uri(url).method("GET");
    if let Some(bearer) = auth_header {
        builder = builder.header(header::AUTHORIZATION, bearer);
    }
    let request = builder.body(Body::empty()).unwrap();
    app.oneshot(request).await.unwrap()
}

async fn first_frame(
    mut stream: impl futures_util::Stream<Item = Result<axum::body::Bytes, axum::Error>> + Unpin,
) -> String {
    let frame = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("stream must emit within 5s")
        .expect("stream must not end before first frame")
        .expect("frame bytes");
    String::from_utf8(frame.to_vec()).expect("utf-8 frame")
}

/// First SSE `data:` payload of one frame (the upstream `event: message`
/// header may precede it).
fn data_line(frame: &str) -> Option<&str> {
    frame.lines().find_map(|line| line.strip_prefix("data: "))
}

/// Production router path: `router_with_auth` builds and caps its own bus.
fn production_app() -> Router {
    let state = AppState {
        sessions: in_memory_sessions(),
        catalog: Arc::new(opencode_rk_catalog::Catalog::default()),
    };
    opencode_rk_server::router_with_auth(state, Some(auth()))
}

/// Poll `subscriber_count()` until it equals `expected` within `within`
/// (failing loudly instead of silently waiting for the 15s heartbeat).
async fn wait_for_subscriber_count(events: &EventBus, expected: usize, within: Duration) {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        let seen = events.subscriber_count();
        if seen == expected {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "subscriber_count={seen}, expected {expected} within {within:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn missing_bearer_rejected_before_any_subscriber() {
    let events = bus();
    let res = get_event(
        app_with_events(in_memory_sessions(), Some(auth()), events.clone()),
        None,
        "/api/event",
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        events.subscriber_count(),
        0,
        "rejected request must not attach a subscriber"
    );
}

#[tokio::test]
async fn wrong_bearer_rejected_before_any_subscriber() {
    let events = bus();
    let res = get_event(
        app_with_events(in_memory_sessions(), Some(auth()), events.clone()),
        Some("Bearer ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
        "/api/event",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        events.subscriber_count(),
        0,
        "wrong bearer must not attach a subscriber"
    );
}

#[tokio::test]
async fn authenticated_sse_emits_server_connected_then_message_appended() {
    let events = bus();
    let res = get_event(
        app_with_events(in_memory_sessions(), Some(auth()), events.clone()),
        Some(&format!("Bearer {TOKEN}")),
        "/api/event",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );
    let mut stream = res.into_body().into_data_stream();
    let connected = first_frame(&mut stream).await;
    // Correction: pinned upstream `eventData` legitimately emits the
    // `event: message` header before the `data:` line; accept it, then parse
    // the data JSON strictly.
    assert!(
        connected.starts_with("event: message\ndata: "),
        "first frame must carry the upstream event: message header: {connected:?}"
    );
    let connected_payload: serde_json::Value =
        serde_json::from_str(data_line(&connected).expect("connected data line"))
            .expect("connected data line is strict JSON");
    assert_eq!(
        connected_payload
            .get("type")
            .and_then(serde_json::Value::as_str),
        Some("server.connected"),
        "first frame must be server.connected: {connected:?}"
    );
    let session = SessionId::new();
    // Fixture maintenance (wire schema upgrade): the durable type requires
    // the canonical persisted UUID — never a fabricated `seq`.
    let message_id = MessageId::new();
    events
        .publish(ServerEvent::MessageStored {
            session,
            message_id,
        })
        .unwrap();
    let next = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("frame within 5s")
        .expect("stream must stay open")
        .expect("bytes");
    let frame = String::from_utf8(next.to_vec()).unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(data_line(&frame).expect("event data line"))
            .expect("event data line is strict JSON");
    assert_eq!(
        payload.get("type").and_then(serde_json::Value::as_str),
        Some("message.appended"),
        "strict type parse: {frame}"
    );
    assert_eq!(
        payload
            .pointer("/properties/session")
            .and_then(serde_json::Value::as_str),
        Some(session.to_string().as_str()),
        "strict session parse: {frame}"
    );
    assert_eq!(
        payload
            .pointer("/properties/message_id")
            .and_then(serde_json::Value::as_str),
        Some(message_id.to_string().as_str()),
        "durable event carries the canonical persisted UUID: {frame}"
    );
    // id is present and never the cursor-zero sentinel alone.
    assert!(
        !frame.replace(' ', "").contains("\"id\":\"0\""),
        "no seq-0 cursor claim: {frame}"
    );
}

#[tokio::test]
async fn unrelated_session_identity_preserved() {
    let events = bus();
    let res = get_event(
        app_with_events(in_memory_sessions(), Some(auth()), events.clone()),
        Some(&format!("Bearer {TOKEN}")),
        "/api/event",
    )
    .await;
    let mut stream = res.into_body().into_data_stream();
    let _connected = first_frame(&mut stream).await;
    let other = SessionId::new();
    events
        .publish(ServerEvent::MessageAppended {
            session: other.clone(),
            seq: events.next_seq(),
        })
        .unwrap();
    let frame = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("frame")
        .expect("open")
        .expect("bytes");
    let text = String::from_utf8(frame.to_vec()).unwrap();
    assert!(
        text.contains(&format!("\"session\":\"{other}\"")),
        "payload must preserve session identity: {text}"
    );
}

#[tokio::test]
async fn two_routers_have_independent_buses() {
    let bus_a = bus();
    let bus_b = bus();
    let auth = auth();
    let token = format!("Bearer {TOKEN}");
    let sessions_a = in_memory_sessions();
    let sessions_b = in_memory_sessions();
    let mut stream_a = get_event(
        app_with_events(sessions_a, Some(auth.clone()), bus_a.clone()),
        Some(&token),
        "/api/event",
    )
    .await
    .into_body()
    .into_data_stream();
    let mut stream_b = get_event(
        app_with_events(sessions_b, Some(auth), bus_b.clone()),
        Some(&token),
        "/api/event",
    )
    .await
    .into_body()
    .into_data_stream();
    let _a = first_frame(&mut stream_a).await;
    let _b = first_frame(&mut stream_b).await;
    // Fixture maintenance (wire schema upgrade): transport-isolation intent
    // unchanged, durable type now requires the canonical persisted UUID.
    bus_a
        .publish(ServerEvent::MessageStored {
            session: SessionId::new(),
            message_id: MessageId::new(),
        })
        .unwrap();
    let seen = tokio::time::timeout(Duration::from_secs(5), stream_a.next())
        .await
        .expect("stream A must receive bus A event")
        .expect("stream A open")
        .expect("bytes");
    assert!(String::from_utf8_lossy(&seen.to_vec()).contains("message.appended"));
    let silence = tokio::time::timeout(Duration::from_millis(300), stream_b.next()).await;
    assert!(
        silence.is_err(),
        "stream B must not receive bus A events (independent routers)"
    );
}

#[tokio::test]
async fn oversize_frame_dropped_but_connection_kept_alive() {
    let events = bus();
    let res = get_event(
        app_with_events(in_memory_sessions(), Some(auth()), events.clone()),
        Some(&format!("Bearer {TOKEN}")),
        "/api/event",
    )
    .await;
    let mut stream = res.into_body().into_data_stream();
    let _connected = first_frame(&mut stream).await;
    let huge = "h".repeat(2 * 1024 * 1024);
    events
        .publish(ServerEvent::ToolExecuted {
            name: huge,
            duration_ms: 0,
        })
        .unwrap();
    let session = SessionId::new();
    // Fixture maintenance (wire schema upgrade): durable type uses the
    // canonical persisted UUID; bounds assertions unchanged.
    events
        .publish(ServerEvent::MessageStored {
            session,
            message_id: MessageId::new(),
        })
        .unwrap();
    let frame = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("small frame after oversize drop")
        .expect("connection stays open")
        .expect("bytes");
    let text = String::from_utf8(frame.to_vec()).unwrap();
    assert!(
        text.contains("\"type\":\"message.appended\"")
            && text.contains(&format!("\"session\":\"{session}\"")),
        "small event must arrive after an oversize frame was dropped: {text}"
    );
}

#[tokio::test]
async fn events_alias_is_authenticated_sse_not_html() {
    let events = bus();
    let unauthorized = get_event(
        app_with_events(in_memory_sessions(), Some(auth()), events.clone()),
        None,
        "/events",
    )
    .await;
    assert_eq!(
        unauthorized.status(),
        StatusCode::UNAUTHORIZED,
        "/events must not be an HTML fallback"
    );
    let res = get_event(
        app_with_events(in_memory_sessions(), Some(auth()), events),
        Some(&format!("Bearer {TOKEN}")),
        "/events",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );
    let mut stream = res.into_body().into_data_stream();
    let connected = first_frame(&mut stream).await;
    assert!(connected.contains("server.connected"), "{connected:?}");
}

#[tokio::test]
async fn slow_consumer_overflow_disconnects_subscriber() {
    let events = EventBus::new(4);
    let res = get_event(
        app_with_events(in_memory_sessions(), Some(auth()), events.clone()),
        Some(&format!("Bearer {TOKEN}")),
        "/api/event",
    )
    .await;
    let mut stream = res.into_body().into_data_stream();
    let _connected = first_frame(&mut stream).await;
    // Flood the bus beyond the per-subscriber channel (cap 4) without reading;
    // publish keeps reporting Full and the entry survives (pre-existing
    // semantics), while the HTTP pump detects the overflow and ends the
    // stream (EOF disconnect) instead of silently dropping frames forever.
    for _ in 0..8 {
        let _ = events.publish(ServerEvent::MessageAppended {
            session: SessionId::new(),
            seq: events.next_seq(),
        });
    }
    assert_eq!(
        events.subscriber_count(),
        1,
        "bus keeps the entry; overflow is reported, not destructive"
    );
    let mut frames_received = 0;
    let end = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match stream.next().await {
                Some(Ok(_)) => frames_received += 1,
                Some(Err(_)) | None => break,
            }
        }
    })
    .await
    .expect("slow consumer must be disconnected (EOF) within 5s");
    let _ = end;
    assert!(
        frames_received <= 8,
        "overflowing consumer is cut off: {frames_received} frames"
    );
}

#[tokio::test]
async fn bus_overflow_preserves_existing_semantics() {
    let bus = EventBus::new(2);
    let sub = bus.subscribe(None);
    bus.publish(ServerEvent::Shutdown).unwrap();
    bus.publish(ServerEvent::Shutdown).unwrap();
    assert_eq!(
        bus.publish(ServerEvent::Shutdown),
        Err(BusError::Full),
        "overflow must surface as Full"
    );
    assert_eq!(
        bus.subscriber_count(),
        1,
        "overflow does not remove the entry"
    );
    assert!(
        sub.has_overflowed(),
        "overflow must be observable by the consumer"
    );
}

#[tokio::test]
async fn route_tables_declare_event_streams_consistently() {
    let served = served_routes();
    assert!(served.contains(&(Method::Get, "/api/event")), "{served:?}");
    assert!(served.contains(&(Method::Get, "/events")), "{served:?}");
    assert_eq!(route_kind("/api/event"), RouteKind::Sse);
    assert_eq!(route_kind("/events"), RouteKind::Sse);
    // Both entries exist once in the authoritative table.
    let mut events_entries = PROTOCOL_ROUTES
        .iter()
        .filter(|r: &&ProtocolRoute| r.path == "/api/event" || r.path == "/events")
        .collect::<Vec<_>>();
    assert_eq!(events_entries.len(), 2);
    events_entries.sort_by_key(|r| r.path);
    assert!(events_entries
        .iter()
        .all(|r| r.handler == "handle_events_sse"));
    check_protocol_coverage().expect("coverage must hold");
}

#[tokio::test]
async fn idle_body_drop_reclaims_subscriber_before_heartbeat() {
    let events = bus();
    let res = get_event(
        app_with_events(in_memory_sessions(), Some(auth()), events.clone()),
        Some(&format!("Bearer {TOKEN}")),
        "/api/event",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let mut stream = res.into_body().into_data_stream();
    let connected = first_frame(&mut stream).await;
    assert!(connected.contains("server.connected"), "{connected:?}");
    assert_eq!(events.subscriber_count(), 1);
    // Drop the response body while the pump is idle: `tx.closed()` must be
    // watched so the subscriber is released well before the 15s heartbeat.
    drop(stream);
    wait_for_subscriber_count(&events, 0, Duration::from_secs(3)).await;
}

#[tokio::test]
async fn full_output_queue_body_drop_reclaims_subscriber() {
    let events = bus();
    let res = get_event(
        app_with_events(in_memory_sessions(), Some(auth()), events.clone()),
        Some(&format!("Bearer {TOKEN}")),
        "/api/event",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let mut stream = res.into_body().into_data_stream();
    let _connected = first_frame(&mut stream).await;
    // Overfill the bounded per-connection output queue (64 frames) without
    // reading any of it: the pump ends up parked inside an outbound send.
    for _ in 0..96 {
        let _ = events.publish(ServerEvent::MessageAppended {
            session: SessionId::new(),
            seq: events.next_seq(),
        });
    }
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(
        events.subscriber_count(),
        1,
        "pump still attached while the output queue is full"
    );
    // Cancellation-aware send: a full output queue must not defer the
    // cancel/overflow/shutdown checks past the connection drop.
    drop(stream);
    wait_for_subscriber_count(&events, 0, Duration::from_secs(3)).await;
}

#[tokio::test]
async fn production_router_refuses_sse_beyond_finite_admission_cap() {
    let cap = opencode_rk_server::event_http::MAX_HTTP_SUBSCRIBERS;
    assert_eq!(cap, 64, "production admission cap stays finite");
    let app = production_app();
    let token = format!("Bearer {TOKEN}");
    let mut held = Vec::with_capacity(cap);
    for admitted in 0..cap {
        let res = get_event(app.clone(), Some(&token), "/api/event").await;
        assert_eq!(res.status(), StatusCode::OK, "admission #{admitted}");
        assert_eq!(
            res.headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/event-stream")
        );
        held.push(res.into_body());
    }
    // N+1: explicit refusal before any pump or `server.connected` frame.
    let refused = get_event(app.clone(), Some(&token), "/api/event").await;
    assert_eq!(refused.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_ne!(
        refused
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream"),
        "refusal must not open a stream"
    );
    // Dropping one connection releases its admission slot: a replacement is
    // admitted (release on drop, not a permanent leak).
    held.pop().expect("held stream");
    let mut replacement = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let res = get_event(app.clone(), Some(&token), "/api/event").await;
        if res.status() == StatusCode::OK {
            replacement = Some(res);
            break;
        }
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            tokio::time::Instant::now() < deadline,
            "dropped connection must release its admission slot within 3s"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let mut stream = replacement
        .expect("replacement response")
        .into_body()
        .into_data_stream();
    let connected = first_frame(&mut stream).await;
    assert!(connected.contains("server.connected"), "{connected:?}");
    // Routers are independent: a second production router has its own bus
    // and its own admission ceiling.
    let other = production_app();
    let res_other = get_event(other, Some(&token), "/api/event").await;
    assert_eq!(
        res_other.status(),
        StatusCode::OK,
        "second router admits independently"
    );
}

#[tokio::test]
async fn concurrent_sse_admission_never_exceeds_cap() {
    let cap = opencode_rk_server::event_http::MAX_HTTP_SUBSCRIBERS;
    let app = production_app();
    let token = format!("Bearer {TOKEN}");
    let attempts = cap + 32;
    let calls = (0..attempts).map(|_| {
        let app = app.clone();
        let token = token.clone();
        async move { get_event(app, Some(&token), "/api/event").await.status() }
    });
    let statuses: Vec<StatusCode> = futures_util::future::join_all(calls).await;
    let admitted = statuses
        .iter()
        .filter(|status| **status == StatusCode::OK)
        .count();
    assert_eq!(
        admitted, cap,
        "concurrent admission is atomic and exactly capped"
    );
    assert!(
        statuses
            .iter()
            .all(|status| *status == StatusCode::OK || *status == StatusCode::SERVICE_UNAVAILABLE),
        "only admission statuses appear: {statuses:?}"
    );
}

// ---------------------------------------------------------------------------
// Durable publication identity (OBS-NATIVE-EVENT-TRANSPORT-INSTALLED)
// ---------------------------------------------------------------------------

/// Issue one route call with an optional bearer and JSON body.
async fn call_route(
    app: Router,
    method: &str,
    url: &str,
    auth_header: Option<&str>,
    body: Option<&str>,
) -> Response<Body> {
    call_route_headers(app, method, url, auth_header, &[], body).await
}

/// Like [`call_route`], with extra explicit request headers (e.g. the
/// `Origin`/`Host` pair required by origin-bound BrowserResume admission).
async fn call_route_headers(
    app: Router,
    method: &str,
    url: &str,
    auth_header: Option<&str>,
    extra_headers: &[(&str, &str)],
    body: Option<&str>,
) -> Response<Body> {
    let mut builder = Request::builder().uri(url).method(method);
    if let Some(bearer) = auth_header {
        builder = builder.header(header::AUTHORIZATION, bearer);
    }
    for (name, value) in extra_headers {
        builder = builder.header(*name, *value);
    }
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    let request = builder
        .body(match body {
            Some(text) => Body::from(text.to_owned()),
            None => Body::empty(),
        })
        .unwrap();
    app.oneshot(request).await.unwrap()
}

/// `GET` with fully explicit headers (native or BrowserResume authorization,
/// optional `Origin`/`Host` binding).
async fn get_event_resume(
    app: Router,
    authorization: Option<&str>,
    origin: Option<&str>,
    host: Option<&str>,
    url: &str,
) -> Response<Body> {
    let mut builder = Request::builder().uri(url).method("GET");
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    if let Some(value) = origin {
        builder = builder.header(header::ORIGIN, value);
    }
    if let Some(value) = host {
        builder = builder.header(header::HOST, value);
    }
    let request = builder.body(Body::empty()).unwrap();
    app.oneshot(request).await.unwrap()
}

/// `GET` with zero or more Authorization field values appended **in order**
/// via `HeaderMap::append`, preserving multiple distinct entries (duplicate
/// and mixed-scheme cardinality probes).
async fn get_event_multi_auth(
    app: Router,
    auths: &[&str],
    origin: Option<&str>,
    host: Option<&str>,
    url: &str,
) -> Response<Body> {
    let mut builder = Request::builder().uri(url).method("GET");
    if let Some(value) = origin {
        builder = builder.header(header::ORIGIN, value);
    }
    if let Some(value) = host {
        builder = builder.header(header::HOST, value);
    }
    let mut request = builder.body(Body::empty()).unwrap();
    for value in auths {
        request.headers_mut().append(
            header::AUTHORIZATION,
            value.parse().expect("static authorization value"),
        );
    }
    app.oneshot(request).await.unwrap()
}

/// Issue an origin-bound capability through the real
/// `POST /auth/browser-resume` route (native bearer + Origin/Host binding).
async fn issue_capability(app: &Router, origin: &str) -> String {
    issue_capability_with_expiry(app, origin, 60_000).await.0
}

/// Issue a capability with an explicit lease TTL through the same real
/// route; returns the capability and the absolute expiry the actual
/// registry response records (`ttl_ms` is the registry's supported lease
/// seam: `1..=MAX_LEASE_MS`).
async fn issue_capability_with_expiry(app: &Router, origin: &str, ttl_ms: u64) -> (String, u64) {
    let host = origin.trim_start_matches("http://");
    let res = call_route_headers(
        app.clone(),
        "POST",
        "/auth/browser-resume",
        Some(&format!("Bearer {TOKEN}")),
        &[("Origin", origin), ("Host", host)],
        Some(&format!("{{\"ttl_ms\":{ttl_ms}}}")),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED, "capability issue");
    let value: serde_json::Value =
        serde_json::from_str(&body_string(res).await).expect("issue JSON");
    let capability = value
        .get("browser_capability")
        .and_then(serde_json::Value::as_str)
        .expect("issued capability")
        .to_owned();
    let expires_at_unix_ms = value
        .get("expires_at_unix_ms")
        .and_then(serde_json::Value::as_u64)
        .expect("absolute expiry");
    (capability, expires_at_unix_ms)
}

/// Collect one complete response body (5s per-chunk deadline).
async fn body_string(res: Response<Body>) -> String {
    let mut stream = res.into_body().into_data_stream();
    let mut out = String::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(Ok(chunk))) => out.push_str(&String::from_utf8_lossy(&chunk)),
            Ok(Some(Err(error))) => panic!("response body error: {error}"),
            Ok(None) => break,
            Err(_) => panic!("response body read timed out"),
        }
    }
    out
}

/// Strict JSON envelope of one SSE frame.
fn envelope(frame: &str) -> serde_json::Value {
    serde_json::from_str(data_line(frame).expect("frame data line")).expect("frame payload JSON")
}

/// Assert no frame and no EOF arrives on this stream within `within`.
async fn assert_silence(
    stream: &mut (impl futures_util::Stream<Item = Result<axum::body::Bytes, axum::Error>> + Unpin),
    within: Duration,
    context: &str,
) {
    match tokio::time::timeout(within, stream.next()).await {
        Err(_) => {}
        Ok(None) => panic!("stream ended unexpectedly: {context}"),
        Ok(Some(Ok(bytes))) => panic!(
            "unexpected frame {}: {context}",
            String::from_utf8_lossy(&bytes)
        ),
        Ok(Some(Err(error))) => panic!("stream error {error}: {context}"),
    }
}

/// The bus allocates ONE stable publication identity before fanout: every
/// subscription observes the identical id/payload for one publication, the
/// identity is fresh per publication (never a reused durable cursor), and
/// the legacy `recv()` API keeps working unchanged.
#[tokio::test]
async fn bus_allocates_one_stable_publication_identity_across_subscriptions() {
    let events = bus();
    let mut a = events.subscribe(None);
    let mut b = events.subscribe(None);
    let session = SessionId::new();
    let message_id = MessageId::new();
    events
        .publish(ServerEvent::MessageStored {
            session,
            message_id,
        })
        .expect("publish");
    let pa = a.recv_published().await.expect("publication for a");
    let pb = b.recv_published().await.expect("publication for b");
    assert_eq!(
        pa.id, pb.id,
        "one identity allocated before fanout, identical for both subscribers"
    );
    assert_eq!(pa, pb, "identical envelope (id + payload) for both");
    assert!(matches!(pa.event, ServerEvent::MessageStored { .. }));

    events.publish(ServerEvent::Shutdown).expect("publish");
    let pa2 = a.recv_published().await.expect("second publication for a");
    let pb2 = b.recv_published().await.expect("second publication for b");
    assert_eq!(pa2.id, pb2.id, "shared identity again");
    assert_ne!(
        pa.id, pa2.id,
        "publication identity is fresh, not a reused replay cursor"
    );

    // Legacy receive API: unchanged, still yields the bare event.
    let mut legacy = events.subscribe(None);
    events.publish(ServerEvent::Shutdown).expect("publish");
    assert_eq!(legacy.recv().await, Some(ServerEvent::Shutdown));
    assert!(legacy.try_recv_published().is_err(), "queue drained");
}

/// Real production router: a durable `POST /api/sessions/{id}/messages`
/// append publishes the persisted `MessageRecord.id` with one shared
/// publication id to two authenticated SSE clients; durable history carries
/// the same identity; failed/unauthorized/foreign-router appends publish
/// nothing; reconnect resnapshots durable history without bus replay.
#[tokio::test]
async fn durable_append_publishes_shared_identity_to_two_sse_clients() {
    let token = format!("Bearer {TOKEN}");
    let app = production_app();

    // Durable session create over the production route (published before the
    // SSE clients attach, so their first frame after subscribing is the
    // durable append).
    let res = call_route(
        app.clone(),
        "POST",
        "/api/sessions",
        Some(&token),
        Some(r#"{"title":"durable-publication"}"#),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created: serde_json::Value =
        serde_json::from_str(&body_string(res).await).expect("session create JSON");
    let session = created
        .pointer("/session/id")
        .and_then(serde_json::Value::as_str)
        .expect("durable session id")
        .to_owned();

    // Two authenticated SSE clients on the same production router.
    let res_a = get_event(app.clone(), Some(&token), "/api/event").await;
    assert_eq!(res_a.status(), StatusCode::OK);
    let mut sse_a = res_a.into_body().into_data_stream();
    let connected_a = first_frame(&mut sse_a).await;
    assert!(connected_a.contains("server.connected"), "{connected_a:?}");
    let res_b = get_event(app.clone(), Some(&token), "/api/event").await;
    assert_eq!(res_b.status(), StatusCode::OK);
    let mut sse_b = res_b.into_body().into_data_stream();
    let connected_b = first_frame(&mut sse_b).await;
    assert!(connected_b.contains("server.connected"), "{connected_b:?}");

    // Durable append through the real route.
    let res = call_route(
        app.clone(),
        "POST",
        &format!("/api/sessions/{session}/messages"),
        Some(&token),
        Some(r#"{"text":"persisted durable payload"}"#),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let posted: serde_json::Value =
        serde_json::from_str(&body_string(res).await).expect("append JSON");
    let message_id = posted
        .pointer("/message/id")
        .and_then(serde_json::Value::as_str)
        .expect("persisted MessageRecord.id")
        .to_owned();
    assert_eq!(
        message_id.len(),
        36,
        "UUID-shaped durable message id: {message_id}"
    );

    // Both clients receive ONE publication with an identical shared id and
    // the actual durable identity (not seq0 / per-client counter).
    let frame_a = first_frame(&mut sse_a).await;
    let frame_b = first_frame(&mut sse_b).await;
    let env_a = envelope(&frame_a);
    let env_b = envelope(&frame_b);
    assert_eq!(env_a["type"], "message.appended", "{frame_a}");
    assert_eq!(env_b["type"], "message.appended", "{frame_b}");
    assert_eq!(
        env_a["id"], env_b["id"],
        "identical shared publication id: {frame_a} vs {frame_b}"
    );
    assert_eq!(
        env_a
            .pointer("/properties/session")
            .and_then(serde_json::Value::as_str),
        Some(session.as_str()),
        "durable session in envelope: {frame_a}"
    );
    assert_eq!(
        env_b
            .pointer("/properties/session")
            .and_then(serde_json::Value::as_str),
        Some(session.as_str()),
        "durable session in envelope: {frame_b}"
    );
    assert_eq!(
        env_a
            .pointer("/properties/message_id")
            .and_then(serde_json::Value::as_str),
        Some(message_id.as_str()),
        "actual persisted MessageRecord.id in envelope: {frame_a}"
    );
    assert_eq!(
        env_b
            .pointer("/properties/message_id")
            .and_then(serde_json::Value::as_str),
        Some(message_id.as_str()),
        "actual persisted MessageRecord.id in envelope: {frame_b}"
    );

    // Durable history rows carry the same identity (event -> storage).
    let res = call_route(
        app.clone(),
        "GET",
        &format!("/api/sessions/{session}/history"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let history: serde_json::Value =
        serde_json::from_str(&body_string(res).await).expect("history JSON");
    let ids: Vec<&str> = history["messages"]
        .as_array()
        .expect("history rows")
        .iter()
        .filter_map(|row| row.get("id").and_then(serde_json::Value::as_str))
        .collect();
    assert!(
        ids.contains(&message_id.as_str()),
        "history rows {ids:?} must contain the published id {message_id}"
    );

    // Negative: missing/wrong bearer never publishes.
    let res = call_route(
        app.clone(),
        "POST",
        &format!("/api/sessions/{session}/messages"),
        None,
        Some(r#"{"text":"unauthorized"}"#),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let res = call_route(
        app.clone(),
        "POST",
        &format!("/api/sessions/{session}/messages"),
        Some("Bearer ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
        Some(r#"{"text":"forbidden"}"#),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_silence(
        &mut sse_a,
        Duration::from_millis(400),
        "missing/wrong bearer must not publish (client A)",
    )
    .await;
    assert_silence(
        &mut sse_b,
        Duration::from_millis(400),
        "missing/wrong bearer must not publish (client B)",
    )
    .await;

    // Negative: a failed durable append (absent session) publishes nothing.
    let absent = "00000000-0000-4000-8000-000000000000";
    let res = call_route(
        app.clone(),
        "POST",
        &format!("/api/sessions/{absent}/messages"),
        Some(&token),
        Some(r#"{"text":"orphan append"}"#),
    )
    .await;
    assert!(
        res.status() != StatusCode::CREATED,
        "durable failure must reject: {}",
        res.status()
    );
    assert_silence(
        &mut sse_a,
        Duration::from_millis(400),
        "failed durable append must not publish (client A)",
    )
    .await;
    assert_silence(
        &mut sse_b,
        Duration::from_millis(400),
        "failed durable append must not publish (client B)",
    )
    .await;

    // Negative: another router's durable append never reaches these streams.
    let other = production_app();
    let res = call_route(
        other.clone(),
        "POST",
        "/api/sessions",
        Some(&token),
        Some(r#"{"title":"other-router"}"#),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let other_created: serde_json::Value =
        serde_json::from_str(&body_string(res).await).expect("other session JSON");
    let other_session = other_created
        .pointer("/session/id")
        .and_then(serde_json::Value::as_str)
        .expect("other session id")
        .to_owned();
    let res = call_route(
        other,
        "POST",
        &format!("/api/sessions/{other_session}/messages"),
        Some(&token),
        Some(r#"{"text":"other router payload"}"#),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    assert_silence(
        &mut sse_a,
        Duration::from_millis(400),
        "cross-router durable append must not reach this router (client A)",
    )
    .await;
    assert_silence(
        &mut sse_b,
        Duration::from_millis(400),
        "cross-router durable append must not reach this router (client B)",
    )
    .await;

    // Reconnect: fresh subscription + durable history snapshot. The bus
    // counter is never a durable cursor: no publication is replayed, while
    // the durable rows retain the actual IDs.
    drop(sse_a);
    let res = get_event(app.clone(), Some(&token), "/api/event").await;
    assert_eq!(res.status(), StatusCode::OK);
    let mut fresh = res.into_body().into_data_stream();
    let fresh_connected = first_frame(&mut fresh).await;
    assert!(
        fresh_connected.contains("server.connected"),
        "{fresh_connected:?}"
    );
    assert_silence(
        &mut fresh,
        Duration::from_millis(400),
        "reconnect must not replay bus publications (no seq-based side effects)",
    )
    .await;
    let res = call_route(
        app,
        "GET",
        &format!("/api/sessions/{session}/history"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let history: serde_json::Value =
        serde_json::from_str(&body_string(res).await).expect("reconnect history JSON");
    let ids: Vec<&str> = history["messages"]
        .as_array()
        .expect("history rows")
        .iter()
        .filter_map(|row| row.get("id").and_then(serde_json::Value::as_str))
        .collect();
    assert!(
        ids.contains(&message_id.as_str()),
        "reconnect snapshot retains durable identity: {ids:?}"
    );
}

// ---------------------------------------------------------------------------
// Security repair (OBS-NATIVE-EVENT-TRANSPORT-INSTALLED, P1 findings)
// ---------------------------------------------------------------------------

/// P1-1: both event aliases authenticate through the SAME bearer middleware
/// as every approved API read. Origin-bound `BrowserResume` is admitted for
/// `GET /api/event` and `/events` (exact path membership, no blanket
/// whitelist, no origin bypass); wrong/missing origin, missing/wrong/revoked/
/// malformed capability, and wrong native bearer all reject **before** any
/// subscriber attaches; native bearer keeps working.
#[tokio::test]
async fn browser_resume_admits_both_event_aliases_origin_bound() {
    let events = bus();
    let app = app_with_events(in_memory_sessions(), Some(auth()), events.clone());
    let origin = "http://127.0.0.1:54321";
    let host = "127.0.0.1:54321";
    let other_origin = "http://127.0.0.1:54999";
    let other_host = "127.0.0.1:54999";
    let capability = issue_capability(&app, origin).await;
    let browser = format!("BrowserResume {capability}");

    // --- negative controls: nothing attaches before successful auth ---
    // missing credential (either alias)
    let res = get_event_resume(app.clone(), None, Some(origin), Some(host), "/events").await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "missing credential");
    // wrong native bearer
    let res = get_event_resume(
        app.clone(),
        Some("Bearer ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
        Some(origin),
        Some(host),
        "/events",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "wrong native bearer");
    // malformed capability (scheme without token)
    let res = get_event_resume(
        app.clone(),
        Some("BrowserResume"),
        Some(origin),
        Some(host),
        "/api/event",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "malformed capability");
    // missing origin/host binding: capability never admits unbound
    let res = get_event_resume(app.clone(), Some(&browser), None, None, "/api/event").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "no origin binding");
    // wrong origin: capability is origin-bound
    let res = get_event_resume(
        app.clone(),
        Some(&browser),
        Some(other_origin),
        Some(other_host),
        "/api/event",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "wrong origin");
    // revoked capability
    let revoked = issue_capability(&app, origin).await;
    let res = call_route_headers(
        app.clone(),
        "DELETE",
        "/auth/browser-resume",
        Some(&format!("BrowserResume {revoked}")),
        &[("Origin", origin), ("Host", host)],
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT, "capability revoke");
    let res = get_event_resume(
        app.clone(),
        Some(&format!("BrowserResume {revoked}")),
        Some(origin),
        Some(host),
        "/api/event",
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "revoked capability");

    assert_eq!(
        events.subscriber_count(),
        0,
        "subscriber count unchanged before successful authentication"
    );

    // --- positive: origin-bound BrowserResume on BOTH aliases ---
    let res = get_event_resume(
        app.clone(),
        Some(&browser),
        Some(origin),
        Some(host),
        "/api/event",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK, "BrowserResume on /api/event");
    let mut stream_a = res.into_body().into_data_stream();
    assert!(
        first_frame(&mut stream_a)
            .await
            .contains("server.connected"),
        "admitted BrowserResume stream connects"
    );
    let res = get_event_resume(
        app.clone(),
        Some(&browser),
        Some(origin),
        Some(host),
        "/events",
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK, "BrowserResume on /events");
    let mut stream_b = res.into_body().into_data_stream();
    assert!(
        first_frame(&mut stream_b)
            .await
            .contains("server.connected"),
        "admitted BrowserResume stream connects on alias"
    );
    assert_eq!(
        events.subscriber_count(),
        2,
        "both aliases admitted through the origin-bound middleware"
    );

    // --- native bearer control still authenticates ---
    let token = format!("Bearer {TOKEN}");
    let res = get_event(app.clone(), Some(&token), "/api/event").await;
    assert_eq!(res.status(), StatusCode::OK, "native bearer control");
    assert_eq!(events.subscriber_count(), 3);
}

/// P1-2: a legacy `MessageAppended` publication (e.g. the shared engine's
/// `seq: 0` replay) must NOT surface as the durable `message.appended` wire
/// type: it is classified as an explicit resync hint without any durable
/// cursor or fabricated message identity. The typed `MessageStored` event
/// keeps the durable type with the canonical UUID, shared by both
/// subscribers. This classification is a documented local contract upgrade
/// requiring independent test adjudication (CONTRACT_NOTE.md).
#[tokio::test]
async fn legacy_message_appended_emits_resync_hint_not_durable_type() {
    let events = bus();
    let app = app_with_events(in_memory_sessions(), Some(auth()), events.clone());
    let token = format!("Bearer {TOKEN}");

    let res_a = get_event(app.clone(), Some(&token), "/api/event").await;
    assert_eq!(res_a.status(), StatusCode::OK);
    let mut stream_a = res_a.into_body().into_data_stream();
    let _connected_a = first_frame(&mut stream_a).await;
    let res_b = get_event(app.clone(), Some(&token), "/events").await;
    assert_eq!(res_b.status(), StatusCode::OK);
    let mut stream_b = res_b.into_body().into_data_stream();
    let _connected_b = first_frame(&mut stream_b).await;

    // seq-0 legacy replay: no durable identity behind it.
    let session = SessionId::new();
    events
        .publish(ServerEvent::MessageAppended {
            session: session.clone(),
            seq: 0,
        })
        .unwrap();
    let frame_a = first_frame(&mut stream_a).await;
    let frame_b = first_frame(&mut stream_b).await;
    let payload = envelope(&frame_a);
    assert_eq!(
        payload.get("type").and_then(serde_json::Value::as_str),
        Some("session.resync_required"),
        "legacy publication classified as resync hint: {frame_a}"
    );
    assert_eq!(
        payload
            .pointer("/properties/session")
            .and_then(serde_json::Value::as_str),
        Some(session.to_string().as_str()),
        "resync hint preserves session identity: {frame_a}"
    );
    assert_eq!(
        payload
            .pointer("/properties/reason")
            .and_then(serde_json::Value::as_str),
        Some("legacy_publication"),
        "resync hint states its legacy reason: {frame_a}"
    );
    assert!(
        payload.pointer("/properties/seq").is_none(),
        "legacy seq must never appear as a durable cursor: {frame_a}"
    );
    assert!(
        !frame_a.contains("message.appended"),
        "legacy publication must not claim the durable type: {frame_a}"
    );
    let payload_b = envelope(&frame_b);
    assert_eq!(
        payload_b.get("type").and_then(serde_json::Value::as_str),
        Some("session.resync_required"),
        "both subscribers observe the resync classification: {frame_b}"
    );
    assert_eq!(
        payload.get("id"),
        payload_b.get("id"),
        "one shared publication identity for the resync envelope"
    );

    // Typed durable event still reaches both subscribers as message.appended
    // with the canonical persisted UUID and the same publication id.
    let message_id = MessageId::new();
    events
        .publish(ServerEvent::MessageStored {
            session: session.clone(),
            message_id,
        })
        .unwrap();
    let durable_a = first_frame(&mut stream_a).await;
    let durable_b = first_frame(&mut stream_b).await;
    let durable_payload = envelope(&durable_a);
    assert_eq!(
        durable_payload
            .get("type")
            .and_then(serde_json::Value::as_str),
        Some("message.appended"),
        "typed durable event keeps the durable wire type: {durable_a}"
    );
    assert_eq!(
        durable_payload
            .pointer("/properties/message_id")
            .and_then(serde_json::Value::as_str),
        Some(message_id.to_string().as_str()),
        "durable event carries the canonical persisted UUID: {durable_a}"
    );
    assert_eq!(
        durable_payload.get("id"),
        envelope(&durable_b).get("id"),
        "both subscribers share one durable publication identity"
    );
}

/// P1-3: the HTTP admission cap is enforced inside the authoritative
/// `router_with_events` constructor — an injected unlimited bus is lowered to
/// the HTTP cap (never widened beyond a preconfigured smaller cap), the N+1
/// request is refused with 503 before subscribing, a dropped subscriber is
/// reclaimed, and routers stay isolated.
#[tokio::test]
async fn router_with_events_caps_injected_unlimited_bus() {
    let cap = opencode_rk_server::event_http::MAX_HTTP_SUBSCRIBERS;
    let injected = bus();
    assert_eq!(
        injected.max_subscribers(),
        usize::MAX,
        "fresh injected bus starts unlimited"
    );

    let app = app_with_events(in_memory_sessions(), Some(auth()), injected.clone());
    assert_eq!(
        injected.max_subscribers(),
        cap,
        "router_with_events lowers an unlimited injected bus to the HTTP cap"
    );

    let token = format!("Bearer {TOKEN}");
    let mut open = Vec::with_capacity(cap);
    for _ in 0..cap {
        let res = get_event(app.clone(), Some(&token), "/api/event").await;
        assert_eq!(res.status(), StatusCode::OK, "within cap admits");
        open.push(res);
    }
    assert_eq!(injected.subscriber_count(), cap, "cap subscribers attached");

    // N+1: refused before any subscription attaches.
    let overflow = get_event(app.clone(), Some(&token), "/api/event").await;
    assert_eq!(
        overflow.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "request beyond the cap is refused"
    );
    assert_eq!(
        injected.subscriber_count(),
        cap,
        "refusal attaches no subscriber"
    );

    // Alias refusal: the same authoritative constructor caps `/events` too,
    // so the route-level cap claim holds for BOTH aliases (test-review
    // recommendation; assertion intent of the original test preserved).
    let overflow_alias = get_event(app.clone(), Some(&token), "/events").await;
    assert_eq!(
        overflow_alias.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "/events beyond the cap is refused through the same constructor"
    );
    assert_eq!(
        injected.subscriber_count(),
        cap,
        "alias refusal attaches no subscriber"
    );

    // Reclaim: dropping one admitted response frees exactly one slot.
    drop(open.pop());
    wait_for_subscriber_count(&injected, cap - 1, Duration::from_secs(5)).await;
    let readmitted = get_event(app.clone(), Some(&token), "/api/event").await;
    assert_eq!(readmitted.status(), StatusCode::OK, "reclaimed slot admits");
    open.push(readmitted);

    // Isolation: a second injected router gets its own capped bus.
    let other_bus = bus();
    let _other = app_with_events(in_memory_sessions(), Some(auth()), other_bus.clone());
    assert_eq!(
        other_bus.max_subscribers(),
        cap,
        "second router independently capped"
    );
    assert_eq!(other_bus.subscriber_count(), 0, "isolated subscriber state");

    // No widening: a preconfigured smaller cap survives router construction.
    let lower = bus();
    lower.set_max_subscribers(8);
    let _lower_app = app_with_events(in_memory_sessions(), Some(auth()), lower.clone());
    assert_eq!(
        lower.max_subscribers(),
        8,
        "constructor never widens a lower preconfigured cap"
    );
}

// ---------------------------------------------------------------------------
// Security repair v6: duplicate Authorization cardinality + alias negatives
// + deterministic expiry (OBS-NATIVE-EVENT-TRANSPORT-INSTALLED)
// ---------------------------------------------------------------------------

/// Duplicate/mixed Authorization fields are refused on BOTH event aliases
/// before any credential verification. Every probe uses a VALID native
/// bearer and a VALID issued, origin-bound capability with correct
/// Origin/Host, so the only attributable failure reason is field
/// cardinality (400). Identical duplicates and mixed schemes are rejected
/// in either order; a single comma-combined value is one field and is
/// rejected by the existing strict parsers (403), not by cardinality;
/// subscriber count never moves and credentials are never echoed.
#[tokio::test]
async fn duplicate_authorization_fields_rejected_on_both_event_aliases() {
    let events = bus();
    let app = app_with_events(in_memory_sessions(), Some(auth()), events.clone());
    let origin = "http://127.0.0.1:54321";
    let host = "127.0.0.1:54321";
    let native = format!("Bearer {TOKEN}");
    let capability = issue_capability(&app, origin).await;
    let browser = format!("BrowserResume {capability}");

    for url in ["/api/event", "/events"] {
        // Duplicate identical native fields (valid token, twice).
        let res = get_event_multi_auth(
            app.clone(),
            &[&native, &native],
            Some(origin),
            Some(host),
            url,
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::BAD_REQUEST,
            "duplicate native fields on {url}"
        );
        let body = body_string(res).await;
        assert!(
            !body.contains(TOKEN),
            "native credential must not be echoed: {body}"
        );

        // Duplicate identical BrowserResume fields (valid capability, twice).
        let res = get_event_multi_auth(
            app.clone(),
            &[&browser, &browser],
            Some(origin),
            Some(host),
            url,
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::BAD_REQUEST,
            "duplicate capability fields on {url}"
        );
        let body = body_string(res).await;
        assert!(
            !body.contains(&capability),
            "capability must not be echoed: {body}"
        );

        // Mixed schemes, both orders (valid credentials, correct origin).
        let res = get_event_multi_auth(
            app.clone(),
            &[&native, &browser],
            Some(origin),
            Some(host),
            url,
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::BAD_REQUEST,
            "native+capability order 1 on {url}"
        );
        let res = get_event_multi_auth(
            app.clone(),
            &[&browser, &native],
            Some(origin),
            Some(host),
            url,
        )
        .await;
        assert_eq!(
            res.status(),
            StatusCode::BAD_REQUEST,
            "native+capability order 2 on {url}"
        );

        // One comma-combined Authorization field is cardinality-clean and
        // must fall to the existing strict parsers (reject, do not split).
        let combined = format!("{native}, {browser}");
        let res =
            get_event_multi_auth(app.clone(), &[&combined], Some(origin), Some(host), url).await;
        assert_eq!(
            res.status(),
            StatusCode::FORBIDDEN,
            "single comma-combined field rejected by strict parsing on {url}"
        );
        let body = body_string(res).await;
        assert!(
            !body.contains(TOKEN),
            "combined credential must not be echoed: {body}"
        );
    }

    assert_eq!(
        events.subscriber_count(),
        0,
        "no duplicate or malformed probe may attach a subscriber"
    );

    // Positive controls: single valid fields still admit both aliases, so
    // the refusals above are attributable to cardinality alone. BOTH
    // successful responses are retained under distinct bindings: the v6
    // review found that overwriting the first `Response` drops its body,
    // closes the pump, and lets async subscription cleanup race the
    // subscriber-count assertion below.
    let res_native = get_event_resume(app.clone(), Some(&native), None, None, "/api/event").await;
    assert_eq!(
        res_native.status(),
        StatusCode::OK,
        "single native control /api/event"
    );
    let res_browser = get_event_resume(
        app.clone(),
        Some(&browser),
        Some(origin),
        Some(host),
        "/events",
    )
    .await;
    assert_eq!(
        res_browser.status(),
        StatusCode::OK,
        "single capability control /events"
    );
    assert_eq!(
        res_native
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream"),
        "native control serves SSE"
    );
    assert_eq!(
        res_browser
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream"),
        "capability control serves SSE"
    );

    // Consume one complete frame from EACH retained stream and strictly
    // parse it: both must be the `server.connected` JSON, proving actual
    // SSE admission rather than a bare 200 response.
    let mut stream_native = res_native.into_body().into_data_stream();
    let connected_native = first_frame(&mut stream_native).await;
    assert!(
        connected_native.starts_with("event: message\ndata: "),
        "native control connected frame header: {connected_native:?}"
    );
    let native_payload: serde_json::Value =
        serde_json::from_str(data_line(&connected_native).expect("native connected data line"))
            .expect("native connected frame is strict JSON");
    assert_eq!(
        native_payload
            .get("type")
            .and_then(serde_json::Value::as_str),
        Some("server.connected"),
        "native control connected frame type: {connected_native}"
    );
    let mut stream_browser = res_browser.into_body().into_data_stream();
    let connected_browser = first_frame(&mut stream_browser).await;
    assert!(
        connected_browser.starts_with("event: message\ndata: "),
        "capability control connected frame header: {connected_browser:?}"
    );
    let browser_payload: serde_json::Value = serde_json::from_str(
        data_line(&connected_browser).expect("capability connected data line"),
    )
    .expect("capability connected frame is strict JSON");
    assert_eq!(
        browser_payload
            .get("type")
            .and_then(serde_json::Value::as_str),
        Some("server.connected"),
        "capability control connected frame type: {connected_browser}"
    );

    // Count asserted while BOTH streams are still held — nothing shadowed
    // or reassigned, so no dropped body can race subscription cleanup.
    assert_eq!(events.subscriber_count(), 2, "controls admit exactly two");

    // Explicit cleanup: drop both streams, then the existing bounded fixture
    // helper proves both subscriptions are reclaimed (exact-count poll, no
    // count-guessing sleep).
    drop(stream_native);
    drop(stream_browser);
    wait_for_subscriber_count(&events, 0, Duration::from_secs(5)).await;
}

/// Explicit negative BrowserResume matrix on the `/events` alias (v5 covered
/// its positive path only): missing, malformed, never-issued, wrong-origin,
/// unbound, and revoked capabilities all reject at the real middleware
/// boundary, and subscriber count stays unchanged; a valid capability then
/// still admits.
#[tokio::test]
async fn events_alias_browser_resume_negative_controls() {
    let events = bus();
    let app = app_with_events(in_memory_sessions(), Some(auth()), events.clone());
    let origin = "http://127.0.0.1:54321";
    let host = "127.0.0.1:54321";

    // Missing credential.
    let res = get_event_resume(app.clone(), None, Some(origin), Some(host), "/events").await;
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "missing credential on /events"
    );

    // Malformed scheme (scheme without token).
    let res = get_event_resume(
        app.clone(),
        Some("BrowserResume"),
        Some(origin),
        Some(host),
        "/events",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "malformed capability on /events"
    );

    // Never-issued capability.
    let res = get_event_resume(
        app.clone(),
        Some("BrowserResume Zm9yZ2VkY2FwYWJpbGl0eQ"),
        Some(origin),
        Some(host),
        "/events",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "unknown capability on /events"
    );

    // Valid capability, wrong origin binding.
    let capability = issue_capability(&app, origin).await;
    let res = get_event_resume(
        app.clone(),
        Some(&format!("BrowserResume {capability}")),
        Some("http://127.0.0.1:54999"),
        Some("127.0.0.1:54999"),
        "/events",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "wrong origin on /events"
    );

    // Valid capability, no Origin/Host binding at all.
    let res = get_event_resume(
        app.clone(),
        Some(&format!("BrowserResume {capability}")),
        None,
        None,
        "/events",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "no origin binding on /events"
    );

    // Revoked capability (revocation is a separate registry operation from
    // expiry; both must refuse).
    let revoked = issue_capability(&app, origin).await;
    let res = call_route_headers(
        app.clone(),
        "DELETE",
        "/auth/browser-resume",
        Some(&format!("BrowserResume {revoked}")),
        &[("Origin", origin), ("Host", host)],
        None,
    )
    .await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT, "capability revoke");
    let res = get_event_resume(
        app.clone(),
        Some(&format!("BrowserResume {revoked}")),
        Some(origin),
        Some(host),
        "/events",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "revoked capability on /events"
    );

    assert_eq!(
        events.subscriber_count(),
        0,
        "no rejected /events probe attaches a subscriber"
    );

    // Positive control: the valid capability still admits the alias.
    let res = get_event_resume(
        app.clone(),
        Some(&format!("BrowserResume {capability}")),
        Some(origin),
        Some(host),
        "/events",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "valid capability admits /events"
    );
    assert_eq!(
        events.subscriber_count(),
        1,
        "positive control attaches exactly one"
    );
}

/// Deterministic expiry through the real lease seam: issue with a small
/// explicit `ttl_ms` (registry accepts `1..=MAX_LEASE_MS`), then wait until
/// the absolute `expires_at_unix_ms` returned by the actual issue response
/// has passed (bounded 25ms poll — the same contract the frozen `g6` expiry
/// test uses; no 15-minute sleep, no clock/env mutation, no faked metadata,
/// revocation not substituted). Both aliases must refuse at the real
/// middleware/registry boundary, and a fresh capability on the same app
/// proves the refusal is expiry-specific.
#[tokio::test]
async fn expired_browser_resume_capability_rejected_by_registry() {
    let events = bus();
    let app = app_with_events(in_memory_sessions(), Some(auth()), events.clone());
    let origin = "http://127.0.0.1:54321";
    let host = "127.0.0.1:54321";

    let (expired, expires_at_unix_ms) = issue_capability_with_expiry(&app, origin, 500).await;
    // Wait until the recorded absolute expiry has demonstrably passed.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before epoch")
            .as_millis() as u64;
        if now_ms >= expires_at_unix_ms {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "expiry observation deadline"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let res = get_event_resume(
        app.clone(),
        Some(&format!("BrowserResume {expired}")),
        Some(origin),
        Some(host),
        "/api/event",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "expired capability refused on /api/event"
    );
    let res = get_event_resume(
        app.clone(),
        Some(&format!("BrowserResume {expired}")),
        Some(origin),
        Some(host),
        "/events",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "expired capability refused on /events"
    );
    assert_eq!(
        events.subscriber_count(),
        0,
        "expired capability attaches no subscriber"
    );

    // Differential control: rejection is expiry-specific — a fresh
    // capability on the same registry still admits.
    let fresh = issue_capability(&app, origin).await;
    let res = get_event_resume(
        app.clone(),
        Some(&format!("BrowserResume {fresh}")),
        Some(origin),
        Some(host),
        "/events",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "fresh capability still admits after the expiry refusal"
    );
    assert_eq!(
        events.subscriber_count(),
        1,
        "fresh control attaches exactly one"
    );
}

/// Response header multiplicity on BOTH SSE aliases through the real router
/// fixture (regression for the observed duplicate-header defect).
///
/// The production SSE builder sets `Content-Type: text/event-stream` and
/// `Cache-Control: no-cache, no-transform` explicitly, then re-applies
/// [`crate::event_stream::sse_headers`], which appends `Content-Type` and
/// `Cache-Control` a second time. A real daemon run observed
/// `text/event-stream, text/event-stream`, which the strict probe (and the
/// upstream generated SSE client's exact content-type check) rejects.
///
/// Asserts each header appears exactly ONCE via `get_all`, retains the
/// explicit `no-cache, no-transform` policy plus the anti-buffering and
/// nosniff security headers, for valid native bearers and valid
/// HTTP-issued BrowserResume capabilities on `/api/event` and `/events`.
#[tokio::test]
async fn sse_response_headers_unique_on_both_aliases() {
    let events = bus();
    let app = app_with_events(in_memory_sessions(), Some(auth()), events.clone());
    let origin = "http://127.0.0.1:54321";
    let host = "127.0.0.1:54321";
    let native = format!("Bearer {TOKEN}");
    let capability = issue_capability(&app, origin).await;
    let browser = format!("BrowserResume {capability}");

    let assert_unique_headers = |res: &Response<Body>, path: &str, label: &str| {
        let content_types: Vec<_> = res
            .headers()
            .get_all(header::CONTENT_TYPE)
            .into_iter()
            .collect();
        assert_eq!(
            content_types.len(),
            1,
            "{path} {label}: duplicate Content-Type values: {content_types:?}"
        );
        assert_eq!(
            content_types[0].to_str().unwrap(),
            "text/event-stream",
            "{path} {label}: Content-Type must be exactly text/event-stream"
        );

        let cache_controls: Vec<_> = res
            .headers()
            .get_all(header::CACHE_CONTROL)
            .into_iter()
            .collect();
        assert_eq!(
            cache_controls.len(),
            1,
            "{path} {label}: duplicate Cache-Control values: {cache_controls:?}"
        );
        assert_eq!(
            cache_controls[0].to_str().unwrap(),
            "no-cache, no-transform",
            "{path} {label}: explicit no-cache, no-transform policy retained"
        );

        let buffering: Vec<_> = res
            .headers()
            .get_all("X-Accel-Buffering")
            .into_iter()
            .collect();
        assert_eq!(
            buffering.len(),
            1,
            "{path} {label}: duplicate X-Accel-Buffering values: {buffering:?}"
        );
        assert_eq!(
            buffering[0].to_str().unwrap(),
            "no",
            "{path} {label}: no buffering"
        );

        let nosniff: Vec<_> = res
            .headers()
            .get_all("X-Content-Type-Options")
            .into_iter()
            .collect();
        assert_eq!(
            nosniff.len(),
            1,
            "{path} {label}: duplicate X-Content-Type-Options values: {nosniff:?}"
        );
        assert_eq!(
            nosniff[0].to_str().unwrap(),
            "nosniff",
            "{path} {label}: nosniff"
        );
    };

    for path in ["/api/event", "/events"] {
        // Arm 1: valid native bearer (no Origin header required).
        let native_res = get_event_resume(app.clone(), Some(&native), None, None, path).await;
        assert_eq!(
            native_res.status(),
            StatusCode::OK,
            "{path} native arm must be admitted"
        );
        assert_unique_headers(&native_res, path, "native");
        // Drop releases this subscriber before the next arm attaches one.
        drop(native_res);

        // Arm 2: valid HTTP-issued BrowserResume capability, correct origin.
        let browser_res =
            get_event_resume(app.clone(), Some(&browser), Some(origin), Some(host), path).await;
        assert_eq!(
            browser_res.status(),
            StatusCode::OK,
            "{path} capability arm must be admitted"
        );
        assert_unique_headers(&browser_res, path, "capability");
        drop(browser_res);
    }

    // All four held/dropped responses reclaim their subscribers cleanly —
    // no frame reads, so no response-lifetime race is introduced.
    wait_for_subscriber_count(&events, 0, Duration::from_secs(5)).await;
}
