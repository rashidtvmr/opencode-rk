//! Reachable remembered-permission regression for the task tool.
//!
//! The real broker baseline always human-gates `task`; a broker-`Allow` path is
//! unreachable through the public router. This test asserts that baseline
//! explicitly, then proves a durable tool-wide deny rejects the task before any
//! child is created, any child provider request is made, or any permission
//! prompt remains. It is honest remembered-deny behavior coverage; it does not
//! claim to distinguish the pre-broker guard from the older install_pending
//! rejection path.

use std::{
    env,
    future::IntoFuture,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Request, StatusCode},
    routing::post,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_security::{Decision, OperationIntent, PermissionBroker, SecurityPolicy};
use opencode_rk_server::{AppState, router};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tower::ServiceExt;

struct Env(&'static str, Option<std::ffi::OsString>);

impl Env {
    fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = env::var_os(name);
        env::set_var(name, value);
        Self(name, previous)
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        if let Some(value) = &self.1 {
            env::set_var(self.0, value);
        } else {
            env::remove_var(self.0);
        }
    }
}

struct Fixture {
    requests: AtomicUsize,
    child_requests: AtomicUsize,
}

async fn provider(
    State(fixture): State<Arc<Fixture>>,
    Json(body): Json<Value>,
) -> ([(&'static str, &'static str); 1], String) {
    fixture.requests.fetch_add(1, Ordering::SeqCst);
    let items = body["input"].as_array().expect("provider input array");
    let last_user = items
        .iter()
        .rposition(|item| item["role"] == "user")
        .expect("provider input has a user message");
    let text = items[last_user]["content"].to_string();
    let tool_output = items[last_user + 1..]
        .iter()
        .any(|item| item["type"] == "function_call_output");

    let event = if text.contains("CHILD_MUST_NOT_RUN") {
        fixture.child_requests.fetch_add(1, Ordering::SeqCst);
        json!({"type":"response.output_text.delta","delta":"child must not run"})
    } else if !tool_output {
        json!({"type":"response.output_item.done","item":{
            "type":"function_call",
            "id":"fc_task_saved_deny",
            "call_id":"task_saved_deny_call",
            "name":"task",
            "arguments":json!({
                "description":"remembered deny probe",
                "prompt":"CHILD_MUST_NOT_RUN: this child must never start",
                "subagent_type":"explore"
            }).to_string()
        }})
    } else {
        // No continuation is expected after the terminal rejected task output.
        // Keep this branch inert; the request-count assertion catches one.
        json!({"type":"response.output_text.delta","delta":"unexpected provider continuation"})
    };

    (
        [("content-type", "text/event-stream")],
        format!(
            "data: {event}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"
        ),
    )
}

fn request(method: &str, path: String, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_task_deny_rejects_without_prompt_or_child() {
    let dir = tempfile::tempdir().unwrap();
    let rules_path = dir.path().join("permission-rules.json");
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&rules_path).unwrap();
        file.write_all(
            br#"{"rules":[{"tool":"task","decision":"deny","scope":"always","session_id":null,"resource":null,"context":null}]}"#,
        )
        .unwrap();
        file.sync_all().unwrap();
    }

    // The turn's real broker is lean_default: task has an unconditional
    // RequireHuman baseline, so a broker-Allow regression is unreachable.
    let intent = OperationIntent::Tool {
        name: "task".to_owned(),
        description: json!({
            "description":"remembered deny probe",
            "prompt":"CHILD_MUST_NOT_RUN: this child must never start"
        })
        .to_string(),
    };
    let baseline = PermissionBroker::new(SecurityPolicy::lean_default(dir.path().to_path_buf()));
    assert!(
        matches!(baseline.authorize(&intent), Decision::RequireHuman { .. }),
        "task baseline must be human-gated; a true broker Allow is unreachable"
    );

    let fixture = Arc::new(Fixture {
        requests: AtomicUsize::new(0),
        child_requests: AtomicUsize::new(0),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(
        axum::serve(
            listener,
            Router::new().fallback(post(provider)).with_state(fixture.clone()),
        )
        .into_future(),
    );

    let _env = [
        Env::set("OPENAI_BASE_URL", base),
        Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()),
        Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", rules_path.as_os_str()),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task"),
    ];

    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(dir.path().join("blobs")).unwrap(),
    ));
    let parent = sessions
        .create_in_directory("parent", Some(dir.path().into()))
        .await
        .unwrap();
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });

    tokio::time::timeout(Duration::from_secs(5), async {
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                format!("/api/sessions/{}/turns/stream", parent.id),
                json!({"text":"please delegate","model":"openai/fixture"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let stream = String::from_utf8_lossy(&bytes);
        assert!(
            stream.contains("\"type\":\"tool_output\"") && stream.contains("task rejected"),
            "saved task deny must appear as a rejected tool result: {stream}"
        );
        assert!(
            stream.contains("\"success\":false") && stream.contains("\"is_error\":true"),
            "the rejected task result must be an error output: {stream}"
        );
        assert!(
            stream.contains("\"type\":\"assistant_message\"")
                && stream.contains("\"stop_reason\":\"completed\""),
            "the turn must terminate cleanly after the rejected task: {stream}"
        );
        assert!(
            !stream.contains("child must not run"),
            "no child output may be produced: {stream}"
        );

        let children = value(
            app.clone()
                .oneshot(request(
                    "GET",
                    format!("/api/sessions/{}/children", parent.id),
                    json!({}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(
            children["children"].as_array().map(Vec::len),
            Some(0),
            "no child may be registered: {children}"
        );

        let permissions = value(
            app.clone()
                .oneshot(request(
                    "GET",
                    format!("/api/session/{}/permission", parent.id),
                    json!({}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(
            permissions["data"].as_array().map(Vec::len),
            Some(0),
            "saved deny must not leave a pending prompt: {permissions}"
        );
    })
    .await
    .expect("saved-deny journey must settle within 5 seconds");

    assert_eq!(
        fixture.child_requests.load(Ordering::SeqCst),
        0,
        "the denied child must never reach the provider"
    );
    assert_eq!(
        fixture.requests.load(Ordering::SeqCst),
        1,
        "a rejected task must stop before any provider continuation"
    );

    mock.abort();
}
