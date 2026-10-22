//! Configured command agent policies must govern the real provider tool loop.
#![forbid(unsafe_code)]

use std::{
    env,
    ffi::OsString,
    future::IntoFuture,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, StatusCode},
    routing::post,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{AppState, router};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tokio::sync::MutexGuard;
use tower::ServiceExt;

const CUSTOM_MARKER: &str = "CUSTOM_AUDITOR_WRITE_PROBE";
const REVIEWER_MARKER: &str = "BUILTIN_REVIEWER_WRITE_PROBE";
const EXPLORE_MARKER: &str = "BUILTIN_EXPLORE_WRITE_PROBE";
const WRITER_MARKER: &str = "CUSTOM_WRITER_CONTROL_PROBE";
const MARKERS: [&str; 4] = [
    CUSTOM_MARKER,
    REVIEWER_MARKER,
    EXPLORE_MARKER,
    WRITER_MARKER,
];
const TURN_ALLOWLIST: &str = "write";
const DENIED_OUTPUT: &str = "not permitted by turn policy";
const WRITE_CONTENT: &str = "configured command write control";
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Holds the process environment lock until every modified variable is restored.
struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    async fn install(root: &Path, provider_base: &str) -> Self {
        let lock = ENV_LOCK.lock().await;
        let config = root.join("global-config");
        let own_config = root.join("own-config");
        let rules = root.join("permission-rules.json");
        let values = [
            ("OPENAI_BASE_URL", OsString::from(provider_base)),
            ("OPENAI_API_KEY", OsString::from("disposable-fixture-key")),
            ("HOME", root.as_os_str().to_owned()),
            ("XDG_CONFIG_HOME", root.as_os_str().to_owned()),
            ("OPENCODE_CONFIG_DIR", config.into_os_string()),
            ("OPENCODE_RK_CONFIG_DIR", own_config.into_os_string()),
            ("OPENCODE_RK_HOME", root.as_os_str().to_owned()),
            ("OPENCODE_RK_PERMISSION_RULES", rules.into_os_string()),
            ("OPENCODE_RK_TURN_TOOLS", OsString::from(TURN_ALLOWLIST)),
        ];
        let previous = values
            .iter()
            .map(|(key, _)| (*key, env::var_os(key)))
            .collect();
        for (key, value) in values {
            env::set_var(key, value);
        }
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, previous) in self.previous.drain(..) {
            if let Some(value) = previous {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }
    }
}

#[derive(Clone, Debug)]
struct ProviderRound {
    marker: String,
    tools: Vec<String>,
    outputs: Vec<String>,
}

fn provider_tool_names(body: &Value) -> Vec<String> {
    match body.get("tools") {
        None => Vec::new(),
        Some(tools) => tools
            .as_array()
            .expect("provider tools must be an array when present")
            .iter()
            .filter_map(|tool| tool["name"].as_str().map(str::to_owned))
            .collect(),
    }
}

#[test]
fn provider_tools_accepts_omitted_or_array_tools_only() {
    assert_eq!(provider_tool_names(&json!({})), Vec::<String>::new());
    assert_eq!(
        provider_tool_names(&json!({"tools":[]})),
        Vec::<String>::new()
    );
    assert_eq!(
        provider_tool_names(&json!({"tools":[{"name":"write"}]})),
        vec!["write"]
    );
}

#[derive(Default)]
struct ProviderFixture {
    root: PathBuf,
    rounds: std::sync::Mutex<Vec<ProviderRound>>,
}

async fn provider(
    State(fixture): State<Arc<ProviderFixture>>,
    Json(body): Json<Value>,
) -> ([(&'static str, &'static str); 1], String) {
    let items = body["input"].as_array().expect("provider input array");
    let last_user = items
        .iter()
        .rposition(|item| item["role"] == "user")
        .expect("command user prompt");
    let prompt = items[last_user]["content"].to_string();
    let marker = MARKERS
        .iter()
        .find(|marker| prompt.contains(**marker))
        .expect("known configured command prompt")
        .to_string();
    let later_items = &items[last_user + 1..];
    let outputs = later_items
        .iter()
        .filter(|item| item["type"] == "function_call_output")
        .filter_map(|item| item["output"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let tools = provider_tool_names(&body);
    fixture
        .rounds
        .lock()
        .expect("fixture rounds")
        .push(ProviderRound {
            marker: marker.clone(),
            tools,
            outputs: outputs.clone(),
        });

    let event = if outputs.is_empty() {
        let call_id = format!("write_{marker}");
        let path = fixture.root.join(format!("{marker}.txt"));
        json!({
            "type":"response.output_item.done",
            "item":{
                "type":"function_call",
                "id":format!("fc_{call_id}"),
                "call_id":call_id,
                "name":"write",
                "arguments":json!({"path":path,"content":WRITE_CONTENT}).to_string()
            }
        })
    } else {
        json!({"type":"response.output_text.delta","delta":"configured command fixture complete"})
    };
    let wire = format!(
        "data: {event}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"
    );
    ([("content-type", "text/event-stream")], wire)
}

struct Harness {
    dir: tempfile::TempDir,
    project: PathBuf,
    session_id: String,
    app: Router,
    fixture: Arc<ProviderFixture>,
    mock: tokio::task::JoinHandle<()>,
    _env: EnvGuard,
}

impl Harness {
    async fn new() -> Self {
        let dir = tempfile::tempdir().expect("disposable command fixture");
        let project = dir.path().join("project");
        let target_root = dir.path().join("outside-targets");
        std::fs::create_dir_all(project.join(".opencode")).expect("project config directory");
        std::fs::create_dir_all(&target_root).expect("disposable outside-project target directory");
        let global_config = dir.path().join("global-config");
        let own_config = dir.path().join("own-config");
        std::fs::create_dir_all(&global_config).expect("isolated global config");
        std::fs::create_dir_all(&own_config).expect("isolated own config");
        let config = json!({
            "commands": {
                "custom-audit": {"template":CUSTOM_MARKER,"agent":"auditor","model":"openai/fixture"},
                "reviewer-audit": {"template":REVIEWER_MARKER,"agent":"reviewer","model":"openai/fixture"},
                "explore-audit": {"template":EXPLORE_MARKER,"agent":"explore","model":"openai/fixture"},
                "custom-write": {"template":WRITER_MARKER,"agent":"fixture-writer","model":"openai/fixture"}
            },
            "agents": {
                "auditor": {"description":"Read-only custom auditor","read_only":true},
                "reviewer": {"description":"Configured reviewer","read_only":false},
                "explore": {"description":"Configured explorer","read_only":false},
                "fixture-writer": {"description":"Writable control agent","read_only":false}
            }
        });
        std::fs::write(project.join(".opencode/opencode.json"), config.to_string())
            .expect("configured command fixture");

        let fixture = Arc::new(ProviderFixture {
            root: target_root,
            rounds: std::sync::Mutex::new(Vec::new()),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback provider bind");
        let provider_base = format!(
            "http://{}/v1",
            listener.local_addr().expect("provider address")
        );
        let provider_state = fixture.clone();
        let mock = tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                Router::new()
                    .fallback(post(provider))
                    .with_state(provider_state),
            )
            .into_future()
            .await;
        });
        let env = EnvGuard::install(dir.path(), &provider_base).await;

        let storage =
            Storage::open_in_memory(dir.path().join("blobs")).expect("in-memory session storage");
        let sessions = SessionService::new(Arc::new(storage));
        let session = sessions
            .create_in_directory("configured command policy", Some(project.clone()))
            .await
            .expect("disposable session");
        let app = router(AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        });
        Self {
            dir,
            project,
            session_id: session.id.to_string(),
            app,
            fixture,
            mock,
            _env: env,
        }
    }

    async fn stop(self) {
        self.mock.abort();
        let _ = self.mock.await;
    }

    fn file(&self, marker: &str) -> PathBuf {
        self.dir
            .path()
            .join("outside-targets")
            .join(format!("{marker}.txt"))
    }

    fn first_round(&self, marker: &str) -> ProviderRound {
        self.fixture
            .rounds
            .lock()
            .expect("fixture rounds")
            .iter()
            .find(|round| round.marker == marker && round.outputs.is_empty())
            .cloned()
            .unwrap_or_else(|| panic!("provider did not receive first round for {marker}"))
    }
}

struct CommandObservation {
    marker: String,
    call_id: String,
    minted_permissions: Vec<Value>,
    tool_output: Value,
    final_permissions: Vec<Value>,
}

fn request(method: Method, path: String, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("fixture request")
}

async fn value(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("bounded fixture response");
    serde_json::from_slice(&bytes).expect("fixture JSON response")
}

async fn permission_rows(app: &Router, session_id: &str) -> Vec<Value> {
    let response = app
        .clone()
        .oneshot(request(
            Method::GET,
            format!("/api/session/{session_id}/permission"),
            json!({}),
        ))
        .await
        .expect("permission listing route");
    value(response).await["data"]
        .as_array()
        .cloned()
        .expect("permission data array")
}

async fn wait_for_first_round(fixture: &ProviderFixture, marker: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if fixture
                .rounds
                .lock()
                .expect("fixture rounds")
                .iter()
                .any(|round| round.marker == marker && round.outputs.is_empty())
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("provider first round for {marker} must arrive"));
}

async fn pending_or_finished(
    app: &Router,
    session_id: &str,
    response_body: &tokio::task::JoinHandle<Vec<u8>>,
) -> Vec<Value> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let rows = permission_rows(app, session_id).await;
            if !rows.is_empty() || response_body.is_finished() {
                return rows;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("command must finish or expose a real broker permission promptly")
}

async fn run_command(
    harness: &Harness,
    command: &str,
    marker: &str,
    reply: &str,
) -> CommandObservation {
    let response = harness
        .app
        .clone()
        .oneshot(request(
            Method::POST,
            format!("/api/sessions/{}/command", harness.session_id),
            json!({"command":command}),
        ))
        .await
        .expect("configured command route");
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "configured command must use the real turn route"
    );
    let body = tokio::spawn(async move {
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("bounded command stream")
            .to_vec()
    });

    wait_for_first_round(&harness.fixture, marker).await;
    let minted_permissions = pending_or_finished(&harness.app, &harness.session_id, &body).await;
    if let Some(pending) = minted_permissions.first() {
        let id = pending["id"].as_str().expect("broker permission id");
        let response = harness
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                format!("/api/session/{}/permission/{id}/reply", harness.session_id),
                json!({"reply":reply}),
            ))
            .await
            .expect("real broker reply route");
        assert_eq!(
            response.status(),
            StatusCode::NO_CONTENT,
            "fixture reply must settle pending permission"
        );
    }
    let bytes = tokio::time::timeout(Duration::from_secs(6), body)
        .await
        .expect("configured command response must drain")
        .expect("command stream task");
    let events = bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice::<Value>(line).expect("command NDJSON event"))
        .collect::<Vec<_>>();
    let call_id = format!("write_{marker}");
    let tool_output = events
        .iter()
        .find(|event| event["type"] == "tool_output" && event["call_id"] == call_id)
        .cloned()
        .unwrap_or_else(|| panic!("missing correlated tool_output for {call_id}: {events:?}"));
    let final_permissions = permission_rows(&harness.app, &harness.session_id).await;
    CommandObservation {
        marker: marker.to_owned(),
        call_id,
        minted_permissions,
        tool_output,
        final_permissions,
    }
}

fn assert_read_only_denial(harness: &Harness, observation: &CommandObservation) {
    let round = harness.first_round(&observation.marker);
    assert!(
        !round.tools.iter().any(|tool| tool == "write"),
        "{} must not advertise write: {:?}",
        observation.marker,
        round.tools
    );
    assert!(
        observation.minted_permissions.is_empty(),
        "{} must not mint human approval: {:?}",
        observation.marker,
        observation.minted_permissions
    );
    assert_eq!(observation.tool_output["call_id"], observation.call_id);
    assert!(
        observation.tool_output["output"]
            .as_str()
            .is_some_and(|output| output.contains(DENIED_OUTPUT)),
        "{} forced write must return correlated turn-policy denial: {:?}",
        observation.marker,
        observation.tool_output
    );
    assert!(
        observation.final_permissions.is_empty(),
        "{} must leave no pending approval: {:?}",
        observation.marker,
        observation.final_permissions
    );
    assert!(
        !harness.file(&observation.marker).exists(),
        "{} denied write must leave no file",
        observation.marker
    );
    let rounds = harness.fixture.rounds.lock().expect("fixture rounds");
    let continuation = rounds
        .iter()
        .find(|round| round.marker == observation.marker && !round.outputs.is_empty())
        .unwrap_or_else(|| panic!("{} must reach provider continuation", observation.marker));
    assert!(
        continuation
            .outputs
            .iter()
            .any(|output| output.contains(DENIED_OUTPUT)),
        "{} denial must be correlated into provider history: {:?}",
        observation.marker,
        continuation.outputs
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_custom_read_only_agent_cannot_advertise_or_execute_write() {
    let harness = Harness::new().await;
    let observation = run_command(&harness, "custom-audit", CUSTOM_MARKER, "reject").await;
    assert_read_only_denial(&harness, &observation);
    harness.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_false_cannot_relax_builtin_read_only_floor() {
    let harness = Harness::new().await;
    let reviewer = run_command(&harness, "reviewer-audit", REVIEWER_MARKER, "reject").await;
    assert_read_only_denial(&harness, &reviewer);
    let explore = run_command(&harness, "explore-audit", EXPLORE_MARKER, "reject").await;
    assert_read_only_denial(&harness, &explore);

    // Writable configured control proves the route still dispatches real writes
    // after an explicit broker approval, rather than globally disabling write.
    let writer = run_command(&harness, "custom-write", WRITER_MARKER, "once").await;
    let writer_round = harness.first_round(WRITER_MARKER);
    assert!(
        writer_round.tools.iter().any(|tool| tool == "write"),
        "full custom agent must retain write: {:?}",
        writer_round.tools
    );
    let approval = writer
        .minted_permissions
        .iter()
        .find(|pending| pending["action"] == "write")
        .unwrap_or_else(|| {
            panic!(
                "writable control must request broker approval: {:?}",
                writer.minted_permissions
            )
        });
    assert_eq!(
        approval["resources"][0].as_str(),
        harness.file(WRITER_MARKER).to_str(),
        "approval must bind the exact disposable target"
    );
    assert_eq!(writer.tool_output["call_id"], writer.call_id);
    assert!(
        writer.tool_output["output"]
            .as_str()
            .is_some_and(|output| output.contains("write success")),
        "approved write must succeed: {:?}",
        writer.tool_output
    );
    assert_eq!(
        std::fs::read_to_string(harness.file(WRITER_MARKER)).expect("approved control file"),
        WRITE_CONTENT
    );
    assert!(
        writer.final_permissions.is_empty(),
        "approved control must settle broker request"
    );
    harness.stop().await;
}
