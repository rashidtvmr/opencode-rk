//! OBS-NATIVE-EVENT-TRANSPORT: authenticated SSE transport over the daemon
//! event bus.
//!
//! Serves `GET /api/event` and the advertised `GET /events` alias — both
//! routed through the SAME bearer middleware ([`crate::daemon_auth::require_bearer`])
//! before any subscription attaches, exactly like every other approved API
//! read (no alias bypass of path matching). The middleware admits either a
//! native bearer or an origin-bound `BrowserResume` capability; the handler
//! re-verifies natively only when the middleware did not mark the request as
//! origin-bound-admitted (legacy `None` routers keep their frozen semantics).
//! Auth strictly precedes subscription: missing/wrong credentials reject
//! before any subscriber is attached.
//!
//! Envelope is upstream-derived (`/api/event`): SSE `data:` frames carrying
//! JSON `{ "id", "type", "properties" }`, `server.connected` emitted first
//! (subscription is registered before the frame, so no early events are
//! lost), `server.heartbeat` keepalive, terminating `server.instance.disposed`
//! on daemon shutdown, `event: message` field for generic consumers.
//!
//! Bounds: one bounded per-connection frame channel (64 frames, 1 MiB frame
//! ceiling), a slow consumer whose bus channel overflowed is disconnected
//! (EOF) via the additive [`opencode_rk_server::event_bus::Subscription`
//! overflow counter — existing `Err(Full)` publish semantics unchanged), a
//! finite per-router HTTP admission cap ([`MAX_HTTP_SUBSCRIBERS`]; the next
//! request is refused `503` before any subscriber/pump), and dropping the
//! response/connection cancels the pump promptly (`tx.closed()` is selected
//! in the idle loop and inside every outbound send, so a full output queue
//! cannot delay cancellation past the heartbeat) and reclaims the
//! subscriber (existing `Subscription` Drop cleanup). Envelope `id` values
//! are the shared publication identity allocated once by
//! [`opencode_rk_server::event_bus::EventBus::publish`] **before fanout**
//! (every subscriber sees the identical id/payload), or generated ids for
//! connection/heartbeat/disposal frames — all from the per-bus ordering
//! counter and **never** a durable replay cursor: a reconnecting client must
//! take a fresh snapshot (no side-effect replay is ever performed).

#![forbid(unsafe_code)]

use crate::{
    daemon_auth::DaemonAuth,
    event_bus::{EventBus, Published, ServerEvent},
    event_stream::encode_frame,
};
use axum::{
    body::{Body, Bytes},
    extract::{Extension, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::stream;
use serde_json::{json, Value};
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc;

/// Per-connection buffered frame ceiling (mirrors `StreamConfig` default).
const MAX_BUFFERED_FRAMES: usize = 64;
/// Keepalive interval: upstream `server.heartbeat` every 10s; local default
/// 15s from [`crate::event_stream::StreamConfig`].
const KEEPALIVE_MS: u64 = 15_000;
/// Finite HTTP subscriber admission for one router bus: at most this many
/// concurrent SSE subscriptions; the next request is refused with `503`
/// before any subscription, pump task, or `server.connected` frame.
pub const MAX_HTTP_SUBSCRIBERS: usize = 64;

/// Map one domain event to the upstream-derived wire shape.
///
/// The envelope `id` is the **shared publication identity** allocated once by
/// the bus in [`EventBus::publish`] (before fanout), so every subscriber of a
/// publication observes the identical `id` — never a per-client wire-layer
/// counter. `events` is not consulted for identity generation here.
#[must_use]
pub fn wire_event(published: &Published) -> (String, String) {
    let (event_type, properties): (&str, Value) = match &published.event {
        ServerEvent::SessionCreated(session) => ("session.created", json!({ "session": session })),
        // Intentional contract upgrade: a legacy `MessageAppended`
        // publication (shared engine `seq: 0`, runtime-wiring replay, SDK
        // helpers) carries no persisted `MessageRecord.id`, so it is
        // EXCLUDED from the durable `message.appended` wire type. Consumers
        // get an explicit resync hint (fetch durable history by identity)
        // and the legacy `seq` is deliberately not serialized — it must
        // never be presented as a durable cursor or a fabricated message
        // identity. Independent test adjudication of this classification is
        // required (documented in CONTRACT_NOTE.md).
        ServerEvent::MessageAppended { session, .. } => (
            "session.resync_required",
            json!({ "session": session, "reason": "legacy_publication" }),
        ),
        // Explicit local extension: durable `message.appended` carrying the
        // actual persisted `MessageRecord.id` (UUID). Same wire type, richer
        // properties; not a claim of full upstream parity.
        ServerEvent::MessageStored {
            session,
            message_id,
        } => (
            "message.appended",
            json!({ "session": session, "message_id": message_id }),
        ),
        ServerEvent::ToolExecuted { name, duration_ms } => (
            "tool.executed",
            json!({ "name": name, "duration_ms": duration_ms }),
        ),
        // No credentials/request text in event bodies: permission requests are
        // surfaced by type only, keeping the stream free of secret echo.
        ServerEvent::PermissionRequested(_) => ("permission.requested", json!({})),
        ServerEvent::Shutdown => ("server.instance.disposed", json!({})),
    };
    let id = format!("ev-{}", published.id);
    (
        event_type.to_owned(),
        serde_json::to_string(&json!({ "id": id, "type": event_type, "properties": properties }))
            .expect("event envelope serialization"),
    )
}

/// Complete SSE frame (`event: message` + `data:` payload), or `None` when the
/// payload exceeds the frame ceiling (frame dropped, stream kept alive).
fn frame_for(payload: &str) -> Option<String> {
    let data = encode_frame(payload.as_bytes()).ok()?;
    Some(format!("event: message\n{data}"))
}

/// Cancellation-aware outbound send: resolves `false` as soon as the
/// response body (receiver) is dropped — even while the bounded output
/// queue is full — so the pump task never outlives its connection and the
/// [`crate::event_bus::Subscription`] drop releases the bus entry promptly.
async fn send_frame(tx: &mpsc::Sender<Result<Bytes, Infallible>>, bytes: Bytes) -> bool {
    tokio::select! {
        biased;
        _ = tx.closed() => false,
        result = tx.send(Ok(bytes)) => result.is_ok(),
    }
}

/// Build the SSE response for one subscription. `events` is the router-owned
/// bus instance (cloned into the handler via `Extension`), so two routers
/// never share a stream. Admission (`try_subscribe`, capped at
/// [`MAX_HTTP_SUBSCRIBERS`] on the production router bus) precedes the
/// connected frame; on refusal a `503` is returned with no subscriber and
/// no pump.
#[must_use]
pub fn event_response(events: EventBus) -> Response {
    let Some(mut subscription) = events.try_subscribe(None) else {
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(axum::body::Body::from("event subscription limit reached"))
            .expect("refusal response");
    };
    let events_for_id = events.clone();
    let connected_frame = frame_for(
        &serde_json::to_string(&json!({
            "id": format!("ev-{}", events_for_id.next_seq()),
            "type": "server.connected",
            "properties": {}
        }))
        .expect("connected envelope"),
    )
    .expect("connected frame fits bounds");

    let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(MAX_BUFFERED_FRAMES);
    tokio::spawn(async move {
        // Eager listener: the subscription above was registered before this
        // frame is emitted, so events published in between are not lost.
        if !send_frame(&tx, Bytes::from(connected_frame)).await {
            return;
        }
        let mut keepalive = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_millis(KEEPALIVE_MS),
            Duration::from_millis(KEEPALIVE_MS),
        );
        keepalive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                // Body dropped: reclaim immediately (idle pumps do not wait
                // for the 15s heartbeat to notice the closed output queue).
                _ = tx.closed() => break,
                _ = keepalive.tick() => {
                    let heartbeat = frame_for(
                        &serde_json::to_string(&json!({
                            "id": format!("ev-{}", events_for_id.next_seq()),
                            "type": "server.heartbeat",
                            "properties": {}
                        }))
                        .expect("heartbeat envelope"),
                    );
                    if let Some(frame) = heartbeat {
                        if !send_frame(&tx, Bytes::from(frame)).await {
                            break;
                        }
                    }
                }
                published = subscription.recv_published() => {
                    match published {
                        None => break,
                        Some(published) if matches!(published.event, ServerEvent::Shutdown) => {
                            let (_, payload) = wire_event(&published);
                            if let Some(frame) = frame_for(&payload) {
                                let _ = send_frame(&tx, Bytes::from(frame)).await;
                            }
                            break;
                        }
                        Some(published) => {
                            if subscription.has_overflowed() {
                                // Slow consumer: EOF disconnect instead of
                                // silently dropping frames forever.
                                break;
                            }
                            let (_, payload) = wire_event(&published);
                            if let Some(frame) = frame_for(&payload) {
                                if !send_frame(&tx, Bytes::from(frame)).await {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    });
    let response_stream = stream::unfold(rx, |mut receiver| async move {
        receiver.recv().await.map(|item| (item, receiver))
    });
    let builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache, no-transform")
        .header("X-Accel-Buffering", "no")
        .header("X-Content-Type-Options", "nosniff");
    builder
        .body(Body::from_stream(response_stream))
        .expect("SSE response")
}

/// SSE endpoint for `/api/event` and the authenticated `/events` alias.
///
/// Both aliases are gated by the shared bearer middleware
/// ([`crate::daemon_auth::require_bearer`]) before this handler runs — the
/// same admission contract as every other approved API read. A
/// `BrowserResume` capability that the middleware admitted (origin +
/// capability verified server-side, marker inserted — never taken from a
/// client header) is accepted without a redundant native-only re-verify;
/// otherwise native bearer verification still applies when an
/// `Extension<DaemonAuth>` router supplied credentials (legacy `None`
/// routers keep serving their frozen test semantics). On authenticated
/// compositions the handler also fails closed on Authorization cardinality
/// itself: duplicate fields are refused before the native verify, so there
/// is no native-only fallback path around the middleware's duplicate check.
pub async fn handle_events_sse(
    State(_state): State<crate::AppState>,
    headers: HeaderMap,
    auth: Option<Extension<DaemonAuth>>,
    resume_admitted: Option<Extension<crate::daemon_auth::BrowserResumeAdmitted>>,
    events: Option<Extension<EventBus>>,
) -> Response {
    if resume_admitted.is_none() {
        if let Some(Extension(credential)) = auth {
            // Cardinality fail-closed on the authenticated composition:
            // duplicate Authorization fields never reach credential
            // verification here either (the middleware already refused
            // them upstream; the marker path is cardinality-checked there
            // too, so only this native fallback needs the guard).
            if headers.get_all(header::AUTHORIZATION).iter().count() > 1 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"message":"multiple authorization fields"})),
                )
                    .into_response();
            }
            let presented = headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok());
            if presented.is_none() {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            if !credential.verify_bearer(presented) {
                return StatusCode::FORBIDDEN.into_response();
            }
        }
    }
    let Some(Extension(events)) = events else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    event_response(events)
}
