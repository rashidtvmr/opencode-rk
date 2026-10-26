
//! Executable HTTP/SQLite coverage for the background-compaction boundary.
//!
//! This deliberately uses only the public router and SessionService APIs.  It
//! does not inspect source text or call `pub(crate)` holder helpers directly.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use futures_util::StreamExt;
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_providers::responses::{responses_request_payload, ResponsesItem, ResponsesRole};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use std::{
    env,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tempfile::tempdir;
use tokio::sync::Notify;
use tower::ServiceExt;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Environment(Vec<(&'static str, Option<std::ffi::OsString>)>);

impl Environment {
    fn set(values: &[(&'static str, String)]) -> Self {
        Self(
            values
                .iter()
                .map(|(key, value)| {
                    let old = env::var_os(key);
                    env::set_var(key, value);
                    (*key, old)
                })
                .collect(),
        )
    }
}

impl Drop for Environment {
    fn drop(&mut self) {
        for (key, old) in &self.0 {
            match old {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }
}

#[derive(Clone)]
struct ProviderState {
    calls: Arc<AtomicUsize>,
    summaries: Arc<AtomicUsize>,
    summary_seen: Arc<Notify>,
    release_first_summary: Arc<Notify>,
    block_first_summary: Arc<std::sync::atomic::AtomicBool>,
    usage_responses: Arc<AtomicUsize>,
    partial_seen: Arc<Notify>,
    normal_input: Arc<tokio::sync::Mutex<Vec<String>>>,
}

async fn responses(State(state): State<ProviderState>, Json(body): Json<Value>) -> Response {
    state.calls.fetch_add(1, Ordering::SeqCst);
    state.usage_responses.fetch_add(1, Ordering::SeqCst);
    let is_summary = body["input"]
        .to_string()
        .to_ascii_lowercase()
        .contains("summarize the older conversation");
    if !is_summary {
        state
            .normal_input
            .lock()
            .await
            .push(body["input"].to_string());
    }
    if is_summary {
        state.summaries.fetch_add(1, Ordering::SeqCst);
        state.summary_seen.notify_waiters();
        if state.block_first_summary.swap(false, Ordering::SeqCst) {
            state.release_first_summary.notified().await;
        }
    }
    let text = if is_summary {
        "DISPOSABLE_BACKGROUND_SUMMARY"
    } else {
        "DISPOSABLE_ASSISTANT_REPLY"
    };
    if body["stream"] == true {
        state.partial_seen.notify_waiters();
        if body["model"]
            .as_str()
            .is_some_and(|model| model.ends_with("disposable-cancel"))
        {
            let stream = futures_util::stream::once(async {
                Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(
                    "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n",
                ))
            }).chain(futures_util::stream::pending());
            return Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/event-stream")
                .body(Body::from_stream(stream))
                .unwrap();
        }
        let events = if body["model"]
            .as_str()
            .is_some_and(|model| model.ends_with("disposable-failure"))
        {
            "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n".to_owned()
        } else if body["model"]
            .as_str()
            .is_some_and(|model| model.ends_with("disposable-missing"))
        {
            "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_owned()
        } else {
            format!("event: response.output_text.delta\ndata: {{\"type\":\"response.output_text.delta\",\"delta\":\"{text}\"}}\n\nevent: response.completed\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\",\"usage\":{{\"input_tokens\":125000,\"output_tokens\":32,\"input_tokens_details\":{{\"cached_tokens\":120000}}}}}}}}\n\n")
        };
        return Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream")
            .body(Body::from(events))
            .unwrap();
    }
    Json(json!({"id": if is_summary { "background-summary" } else { "turn-response" }, "status":"completed", "usage":{"input_tokens":125000,"output_tokens":32,"input_tokens_details":{"cached_tokens":120000}}, "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}]})).into_response()
}

async fn provider(state: ProviderState) -> (String, ProviderGuard) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/responses", post(responses))
                .with_state(state),
        )
        .await
        .unwrap();
    });
    (format!("http://{address}"), ProviderGuard(Some(task)))
}

struct ProviderGuard(Option<tokio::task::JoinHandle<()>>);

impl Drop for ProviderGuard {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
        }
    }
}

impl ProviderGuard {
    async fn shutdown(mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
            let _ = task.await;
        }
    }
}

async fn post_turn(
    app: axum::Router,
    session: opencode_rk_contracts::SessionId,
    text: String,
) -> StatusCode {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/api/sessions/{session}/turns"))
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                json!({"model":"openai/disposable","text":text}).to_string(),
            ))
            .unwrap(),
    )
    .await
    .unwrap()
    .status()
}

fn estimate_fixture_request(input: &[ResponsesItem], next_text: &str) -> u64 {
    let request = responses_request_payload("openai/disposable", "", input, &[], false)
        .expect("fixture input must use the production request builder");
    let bytes = serde_json::to_vec(&request)
        .expect("production request payload must remain serializable")
        .len() as u64;
    (bytes.saturating_add(3) / 4)
        .saturating_add((next_text.len() as u64).saturating_add(3) / 4)
}

#[tokio::test]
async fn successful_turn_holds_background_result_without_durable_checkpoint() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    let state = ProviderState {
        calls: Arc::new(AtomicUsize::new(0)),
        summaries: Arc::new(AtomicUsize::new(0)),
        summary_seen: Arc::new(Notify::new()),
        release_first_summary: Arc::new(Notify::new()),
        block_first_summary: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        usage_responses: Arc::new(AtomicUsize::new(0)),
        partial_seen: Arc::new(Notify::new()),
        normal_input: Arc::new(tokio::sync::Mutex::new(Vec::new())),
    };
    let (base, provider_task) = provider(state.clone()).await;
    let _env = Environment::set(&[
        ("OPENAI_BASE_URL", base),
        ("OPENAI_API_KEY", "disposable-background-key".into()),
        ("OPENCODE_RK_HOME", root.path().display().to_string()),
        ("OPENCODE_RK_COMPACTION__ENABLED", "true".into()),
        ("OPENCODE_RK_COMPACTION__STRATEGY", "fallback".into()),
        // The persisted history is just below the finite scheduler ceiling;
        // the bounded second-turn input crosses it and drives the real
        // foreground-due route.
        ("OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS", "140000".into()),
        ("OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS", "1".into()),
    ]);

    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(root.path().join("blobs")).unwrap(),
    ));
    let session = sessions
        .create_in_directory("background runtime", Some(root.path().to_owned()))
        .await
        .unwrap();
    // Build a genuinely large persisted history through SessionService. Each
    // message remains below storage's 65536-byte content bound, while the
    // aggregate request is large enough to exercise the real precompute band.
    // The two extra oldest messages calibrate the fixture's old prefix only;
    // they do not change the retained anchor or the prospective second-turn
    // tail. Their combined 111,000 bytes keep the production 512 KiB input
    // cap intact while moving only the second preflight across 140,000.
    let mut fixture_input = Vec::new();
    sessions
        .append_text(
            session.id,
            MessageRole::User,
            format!("old prefix user {}", "p".repeat(55_500)),
        )
        .await
        .unwrap();
    fixture_input.push(ResponsesItem::Text {
        role: ResponsesRole::User,
        content: format!("old prefix user {}", "p".repeat(55_500)),
    });
    sessions
        .append_text(
            session.id,
            MessageRole::Assistant,
            format!("old prefix assistant {}", "q".repeat(55_500)),
        )
        .await
        .unwrap();
    fixture_input.push(ResponsesItem::Text {
        role: ResponsesRole::Assistant,
        content: format!("old prefix assistant {}", "q".repeat(55_500)),
    });
    for index in 0..4 {
        let user = format!("user history turn {index} {}", "x".repeat(49_980));
        let assistant = format!("assistant history turn {index} {}", "y".repeat(49_975));
        sessions
            .append_text(
                session.id,
                MessageRole::User,
                user.clone(),
            )
            .await
            .unwrap();
        fixture_input.push(ResponsesItem::Text {
            role: ResponsesRole::User,
            content: user,
        });
        sessions
            .append_text(
                session.id,
                MessageRole::Assistant,
                assistant.clone(),
            )
            .await
            .unwrap();
        fixture_input.push(ResponsesItem::Text {
            role: ResponsesRole::Assistant,
            content: assistant,
        });
    }

    // This uses the same request payload builder and byte/token conversion as
    // the production preflight, rather than an independent byte-only mirror.
    // Upstream compaction checks due before consuming a held summary
    // (compaction.ts:288-307), and estimates measured plus unmeasured prompt
    // text (compaction.ts:1067-1096). Keep the first turn below due, then add
    // its real persisted user/assistant pair before checking the second turn.
    let first_estimate = estimate_fixture_request(&fixture_input, "next turn");
    assert!(first_estimate < 140_000, "first fixture preflight must remain below due");
    let mut second_input = fixture_input.clone();
    second_input.push(ResponsesItem::Text {
        role: ResponsesRole::User,
        content: "next turn".into(),
    });
    second_input.push(ResponsesItem::Text {
        role: ResponsesRole::Assistant,
        content: "DISPOSABLE_ASSISTANT_REPLY".into(),
    });
    let second_estimate = estimate_fixture_request(&second_input, &"z".repeat(49_000));
    assert!(second_estimate >= 140_000, "second fixture preflight must be due");
    let retained_tail_bytes = "assistant history turn 3 ".len() + 49_975 + 49_000;
    assert!((retained_tail_bytes as u64).div_ceil(4) <= 30_000);

    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let summary_ready = state.summary_seen.notified();
    let (first_status_tx, first_status_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = first_status_tx.send(post_turn(app, session.id, "next turn".into()).await);
    });

    tokio::time::timeout(Duration::from_secs(2), summary_ready)
        .await
        .expect("successful turn must reach the actual background provider boundary");
    // The summary remains blocked while the public foreground turn finishes.
    // This independently probes whether RunOwner::finish cancels it.
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), first_status_rx)
            .await
            .expect("foreground turn must finish while summary is blocked")
            .expect("foreground status sender must remain live"),
        StatusCode::CREATED
    );
    state.release_first_summary.notify_waiters();
    assert!(state.summaries.load(Ordering::SeqCst) >= 1);
    assert!(
        sessions
            .compaction_checkpoint(session.id)
            .await
            .unwrap()
            .is_none(),
        "background completion must not persist a checkpoint"
    );

    // A second public turn is the real foreground-due path. If the held job
    // survived and was consumable, this turn would reuse it exactly once. The
    // current candidate instead falls back after owner cancellation, issuing a
    // second summary request; retaining this assertion makes that failure
    // visible rather than converting it to a synthetic PASS.
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(2),
            post_turn(
                router(AppState {
                    sessions: sessions.clone(),
                    catalog: Arc::new(Catalog::default()),
                }),
                session.id,
                "z".repeat(49_000),
            ),
        )
        .await
        .expect("foreground due turn must finish"),
        StatusCode::CREATED
    );
    assert_eq!(
        state.summaries.load(Ordering::SeqCst),
        1,
        "foreground compaction should consume the held summary rather than issue a second request"
    );
    let checkpoint = sessions
        .compaction_checkpoint(session.id)
        .await
        .unwrap()
        .expect("foreground compaction must persist the consumed held checkpoint");
    let checkpoint_json: serde_json::Value =
        serde_json::from_str(&checkpoint.window_json).expect("checkpoint JSON must remain typed");
    assert_eq!(checkpoint_json["object"], "opencode.fallback_compaction");
    assert!(checkpoint_json["summary"]
        .as_str()
        .is_some_and(|text| text.contains("DISPOSABLE_BACKGROUND_SUMMARY")));
    let normal_inputs = state.normal_input.lock().await;
    assert!(
        normal_inputs
            .iter()
            .any(|input| input.contains("DISPOSABLE_BACKGROUND_SUMMARY")),
        "foreground provider request must include the held checkpoint marker"
    );
    assert!(
        normal_inputs.iter().any(|input| input.contains("z")),
        "foreground request must retain the new turn tail"
    );

    provider_task.shutdown().await;
}
