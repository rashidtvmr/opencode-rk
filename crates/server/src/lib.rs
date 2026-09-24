//! Small HTTP boundary over the native catalog and session services.
#![forbid(unsafe_code)]
pub mod acp_bridge;
pub mod acp_files;
pub mod acp_session;
pub mod admission_bounds;
pub mod app_client;
pub mod agent_loop;
pub use agent_loop::{
    function_call_output as loop_function_call_output, truncate_tool_output, CallOutput,
    LoopController, RequestedCall, TurnStop, MAX_CALLS_PER_ROUND, MAX_TOOL_OUTPUT_CHARS,
    MAX_TURN_STEPS,
};
pub mod app_protocols;
pub mod app_runtime;
pub mod auto_loop;
pub mod auto_report;
pub mod auto_window;
pub mod chat_composer;
pub mod clients;
pub mod control_decode;
pub mod control_plane_errors;
pub mod control_plane_exposure;
pub mod control_plane_inputs;
pub mod daemon;
pub mod daemon_auth;
pub mod desktop_bridge;
pub mod loop_driver;
pub mod rules_globs;
pub mod rules_loader;
pub mod context_report;
pub mod context_accounting;
pub mod runtime_wiring;
pub mod enterprise_link;
pub mod error_translate;
pub mod event_bus;
pub mod event_cursor;
pub mod event_stream;
pub mod origin_check;
pub mod protocol_api;
pub mod rel_stamp;
pub mod rel_verify;
pub mod remote_ledger;
pub mod remote_sync;
pub mod repo_ops;
pub mod route_table;
pub mod sdk_client;
pub mod sdk_spawns;
pub mod sync_log;
pub mod transcript_lane;
pub mod turn_parts;
pub mod turn_service;
pub mod voice_capture;
pub mod web_artifact;
pub mod web_assets;
pub mod web_attachments;
pub mod web_config;
pub mod web_cors;
pub mod web_entry_probe;
pub mod web_footer;
pub mod web_headers;
pub mod web_host;
pub mod web_route;
pub mod web_suffix;
pub mod web_tool_chooser;
pub mod workspace_proxy;
pub mod remote_approvals;
pub mod remote_connector;
pub mod remote_files;
pub mod remote_pty;
pub mod remote_recovery;
pub mod remote_revocation;
pub mod remote_sessions;
pub mod remote_turns;
pub mod web_turn_adapter;
pub mod workspace_sessions;
use crate::app_runtime::PolicyDecision;
use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Extension, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use futures_util::stream;
use opencode_rk_catalog::{Catalog, CatalogQuery};
use opencode_rk_contracts::{
    ArtifactId, ArtifactKind, AssistantReference, AssistantToolCall, AttachmentId, MessageId,
    MessageRecord, MessageRole, PayloadRef, SessionId, MAX_ASSISTANT_TOOL_CALLS,
    MAX_DRAFT_ATTACHMENT_BYTES, MAX_REASONING_SUMMARY_BYTES, WIRE_SCHEMA_VERSION,
};
use opencode_rk_providers::responses::{
    OpenAiResponsesClient, OpenAiResponsesStream, ResponsesError, ResponsesInput, ResponsesItem,
    ResponsesRole, ResponsesStopReason, ResponsesStreamEvent, ResponsesTool,
    MAX_RESPONSES_INPUT_MESSAGES,
};
use opencode_rk_security::{Decision, OperationIntent, PermissionBroker, SecurityPolicy};
use opencode_rk_sessions::{SessionError, SessionService};
use opencode_rk_tools::executor::ToolExecutor;
use opencode_rk_tools::shell_tool::{ShellConfig, ShellError, ShellResult, ShellTool};
use opencode_rk_tools::registry::ToolRegistry;
use opencode_rk_agents::agent_executor::AgentExecutor;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    fs,
    future::Future,
    path::PathBuf,
    pin::Pin,
    str::FromStr,
    sync::{atomic::{AtomicU64, Ordering}, Arc},
};
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore, SemaphorePermit};

static TURN_PERMITS: Semaphore = Semaphore::const_new(2);
static NEXT_SHELL_ARTIFACT: AtomicU64 = AtomicU64::new(1);

const SHELL_STARTUP_WRAPPER: &str =
    "tmp=\"$1.tmp.$$\"; printf '%s\\n' \"$$\" > \"$tmp\"; mv -f \"$tmp\" \"$1\"; exec bash -c \"$2\"";
const MAX_SHELL_COMMAND_BYTES: usize = 64 * 1024;
const MAX_SHELL_ERROR_BYTES: usize = 1024;
const MAX_SHELL_ARTIFACT_ATTEMPTS: u64 = 8;

struct ShellArtifact {
    directory: PathBuf,
    readiness_path: PathBuf,
}

impl ShellArtifact {
    fn create() -> Result<Self, String> {
        let root = std::env::temp_dir();
        let pid = std::process::id();
        for _ in 0..MAX_SHELL_ARTIFACT_ATTEMPTS {
            let sequence = NEXT_SHELL_ARTIFACT.fetch_add(1, Ordering::Relaxed);
            let directory = root.join(format!("opencode-rk-shell-{pid}-{sequence}"));
            match fs::create_dir(&directory) {
                Ok(()) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut permissions = fs::metadata(&directory)
                            .map_err(|error| format!("shell readiness metadata: {error}"))?
                            .permissions();
                        permissions.set_mode(0o700);
                        fs::set_permissions(&directory, permissions)
                            .map_err(|error| format!("shell readiness permissions: {error}"))?;
                    }
                    return Ok(Self {
                        readiness_path: directory.join("ready.pid"),
                        directory,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("shell readiness directory: {error}")),
            }
        }
        Err("shell readiness directory allocation exhausted".to_owned())
    }
}

impl Drop for ShellArtifact {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.readiness_path);
        let _ = fs::remove_dir_all(&self.directory);
    }
}

struct ShellExecutionGuard {
    artifact: ShellArtifact,
    startup: mpsc::Receiver<u32>,
    execution: Option<Pin<Box<dyn Future<Output = Result<ShellResult, ShellError>> + Send>>>,
    result: Option<Result<ShellResult, ShellError>>,
}

impl ShellExecutionGuard {
    async fn wait_for_startup(&mut self) -> Result<u32, String> {
        if self.result.is_some() {
            return Err("shell execution finished before readiness".to_owned());
        }
        let Some(mut execution) = self.execution.take() else {
            return Err("shell execution future missing".to_owned());
        };
        tokio::select! {
            result = &mut execution => {
                self.result = Some(result);
                Err("shell execution finished before readiness".to_owned())
            }
            startup = self.startup.recv() => {
                match startup {
                    Some(pid) if pid > 1 => {
                        self.execution = Some(execution);
                        Ok(pid)
                    }
                    Some(_) => {
                        self.execution = Some(execution);
                        Err("shell startup returned invalid PID".to_owned())
                    }
                    None => {
                        self.execution = Some(execution);
                        Err("shell startup channel closed before readiness".to_owned())
                    }
                }
            }
        }
    }

    async fn finish(mut self) -> Result<ShellResult, String> {
        let result = if let Some(result) = self.result.take() {
            result
        } else if let Some(execution) = self.execution.take() {
            execution.await
        } else {
            return Err("shell execution future missing".to_owned());
        };
        result.map_err(|error| error.to_string())
    }
}

fn bounded_shell_error(error: impl Into<String>) -> String {
    let mut error = error.into();
    if error.len() > MAX_SHELL_ERROR_BYTES {
        error.truncate(MAX_SHELL_ERROR_BYTES);
        error.push_str(" [truncated]");
    }
    error
}

enum HttpTurnPermit {
    Shared { _permit: OwnedSemaphorePermit },
    Legacy { _permit: SemaphorePermit<'static> },
}

fn acquire_http_turn_permit(
    runtime: Option<&runtime_wiring::RuntimeWiring>,
) -> Result<HttpTurnPermit, ApiFailure> {
    if let Some(runtime) = runtime {
        return runtime
            .try_acquire_turn()
            .map(|permit| HttpTurnPermit::Shared { _permit: permit })
            .map_err(|_| ApiFailure::too_many_requests("too many active turns"));
    }
    TURN_PERMITS
        .try_acquire()
        .map(|permit| HttpTurnPermit::Legacy { _permit: permit })
        .map_err(|_| ApiFailure::too_many_requests("too many active turns"))
}

fn publish_runtime_event(
    events: Option<&event_bus::EventBus>,
    event: event_bus::ServerEvent,
) {
    if let Some(events) = events {
        events.publish(event).ok();
    }
}

#[derive(Clone)]
pub struct AppState {
    pub sessions: SessionService,
    pub catalog: Arc<Catalog>,
}
#[derive(Clone, Copy)]
struct StructuredActivityEnabled;
pub fn router(state: AppState) -> Router {
    router_with_auth(state, None)
}

/// Authenticated router: `Some(auth)` gates every `/api/*` route with the
/// bearer middleware; `None` is the legacy unauthenticated router used by
/// frozen tests. The serve path always passes `Some`.
pub fn router_with_auth(state: AppState, auth: Option<daemon_auth::DaemonAuth>) -> Router {
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/capabilities", get(web_capabilities))
        .route("/api/workspaces", get(list_workspaces))
        .route("/api/models", get(search_models))
        .route("/api/models/{provider}/{model}", get(get_model))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}", get(get_session).patch(rename_session))
        .route("/api/sessions/{id}/archive", post(archive_session))
        .route(
            "/api/sessions/{id}/messages",
            get(list_messages).post(append_message),
        )
        .route("/api/sessions/{id}/history", get(list_history_page))
        .route(
            "/api/sessions/{id}/attachments",
            get(list_draft_attachments)
                .post(upload_draft_attachment)
                .layer(DefaultBodyLimit::max(MAX_DRAFT_ATTACHMENT_BYTES)),
        )
        .route(
            "/api/sessions/{id}/attachments/{attachment_id}",
            delete(delete_draft_attachment),
        )
        .route("/api/sessions/{id}/activity", get(list_assistant_activity))
        .route("/api/sessions/{id}/artifacts", get(list_artifacts))
        .route(
            "/api/sessions/{id}/messages/{message_id}/artifacts",
            post(create_artifact),
        )
        .route(
            "/api/sessions/{id}/artifacts/{artifact_id}",
            get(get_artifact),
        )
        .route(
            "/api/sessions/{id}/artifacts/{artifact_id}/versions",
            post(append_artifact_version),
        )
        .route(
            "/api/sessions/{id}/messages/{message_id}/branch",
            post(branch_session),
        )
        .route(
            "/api/sessions/{id}/messages/{message_id}/retry-branch",
            post(prepare_retry_branch),
        )
        .route("/api/sessions/{id}/fork", get(get_fork_provenance))
        .route("/api/sessions/{id}/turns", post(create_turn))
        .route("/api/sessions/{id}/turns/stream", post(create_turn_stream))
        .fallback(web_assets::serve)
        .with_state(state);
    match auth {
        Some(credential) => app
            .layer(Extension(StructuredActivityEnabled))
            .layer(axum::middleware::from_fn_with_state(
                credential,
                daemon_auth::require_bearer,
            )),
        None => app,
    }
}
async fn health() -> Json<Value> {
    Json(json!({"schema_version":WIRE_SCHEMA_VERSION,"status":"ok","runtime":"native-rust"}))
}
async fn web_capabilities() -> Json<Value> {
    let registry = ToolRegistry::new();
    let mut tools = registry
        .list()
        .into_iter()
        .map(|tool| {
            // Honest capability report: a tool is turn-executable exactly when
            // the operator's allowlist (OPENCODE_RK_TURN_TOOLS) admits it.
            let turn_enabled = turn_tool_config().iter().any(|name| *name == tool.id);
            json!({
                "id": tool.id,
                "name": tool.name,
                "description": tool.description,
                "enabled": tool.enabled,
                "type": tool.tool_type,
                "tags": tool.tags,
                "available_for_web_turn": turn_enabled,
                "reason": if turn_enabled {
                    format!(
                        "executable by the turn adapter; configured via OPENCODE_RK_TURN_TOOLS ({})",
                        tool.id
                    )
                } else {
                    "not executable by the turn adapter; add it to OPENCODE_RK_TURN_TOOLS to enable".to_owned()
                },
            })
        })
        .collect::<Vec<_>>();
    tools.sort_by(|left, right| {
        left["id"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["id"].as_str().unwrap_or_default())
    });
    Json(json!({
        "schema_version": WIRE_SCHEMA_VERSION,
        "tools": tools,
        "plugins": {
            "available_for_web_turn": false,
            "reason": "the web daemon does not yet own a plugin host or turn adapter"
        },
        "approvals": {
            "available_for_web_turn": false,
            "reason": "approval records exist natively but the web turn path has no execution-owned approval flow"
        },
        "attachments": {
            "draft_ingest": true,
            "available_for_web_turn": false,
            "reason": "draft files are persisted but the provider attachment adapter is unavailable"
        },
        "search": {
            "available": false,
            "reason": "local grep is not a web-search adapter"
        },
        "deep_research": {
            "available": false,
            "reason": "no native research execution and citation adapter is installed"
        },
        "voice": {
            "available": false,
            "reason": "no native transcription or realtime audio adapter is installed"
        },
        "artifacts": {
            "available": true,
            "editing_available": true,
            "run_available": false,
            "apply_available": false,
            "reason": "durable editing and versioning are available; run/apply require a safe native execution and approval bridge"
        }
    }))
}
async fn list_workspaces(State(state): State<AppState>) -> Result<Json<Value>, ApiFailure> {
    let Some(workspaces) = state
        .sessions
        .list_workspaces(500)
        .await
        .map_err(ApiFailure::internal)?
    else {
        return Ok(Json(json!({
            "schema_version": WIRE_SCHEMA_VERSION,
            "available": false,
            "workspaces": [],
            "session_scope_available": false,
            "memory_available": false,
            "reason": "the native workspace catalog is not configured for this server"
        })));
    };
    let workspaces = workspaces
        .into_iter()
        .map(|workspace| {
            let id = workspace
                .id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            json!({
                "id": id,
                "label": workspace.label,
                "project_root": workspace.project_root,
                "status": workspace.status,
                "created_at_us": workspace.created_at_us,
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "schema_version": WIRE_SCHEMA_VERSION,
        "available": true,
        "workspaces": workspaces,
        "session_scope_available": false,
        "memory_available": false,
        "reason": "workspace registry metadata is available; session membership and memory/context authority are not implemented"
    })))
}
#[derive(Debug, Deserialize)]
struct ModelSearchParams {
    q: Option<String>,
    provider: Option<String>,
    reasoning: Option<bool>,
    tools: Option<bool>,
    attachments: Option<bool>,
    structured_output: Option<bool>,
    limit: Option<usize>,
}
async fn search_models(
    State(state): State<AppState>,
    Query(p): Query<ModelSearchParams>,
) -> Result<Json<Value>, ApiFailure> {
    let models = state
        .catalog
        .search(&CatalogQuery {
            text: p.q,
            provider: p.provider,
            reasoning: p.reasoning,
            tools: p.tools,
            attachments: p.attachments,
            structured_output: p.structured_output,
            limit: p.limit.unwrap_or(100).clamp(1, 500),
        })
        .map_err(ApiFailure::internal)?;
    Ok(Json(
        json!({"schema_version":WIRE_SCHEMA_VERSION,"source":"models.dev","count":models.len(),"models":models}),
    ))
}
async fn get_model(
    State(state): State<AppState>,
    Path((provider, model)): Path<(String, String)>,
) -> Result<Json<Value>, ApiFailure> {
    let model = state
        .catalog
        .model(&provider, &model)
        .map_err(|e| ApiFailure::not_found(e.to_string()))?;
    Ok(Json(
        json!({"schema_version":WIRE_SCHEMA_VERSION,"source":"models.dev","model":model}),
    ))
}
#[derive(Debug, Deserialize)]
struct SessionListParams {
    all: Option<bool>,
}
async fn list_sessions(
    State(state): State<AppState>,
    Query(p): Query<SessionListParams>,
) -> Result<Json<Value>, ApiFailure> {
    let sessions = state
        .sessions
        .list(p.all.unwrap_or(false))
        .await
        .map_err(ApiFailure::internal)?;
    Ok(Json(json!({"sessions":sessions})))
}
#[derive(Debug, Deserialize)]
struct CreateSessionBody {
    title: String,
}
async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionBody>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let session = state
        .sessions
        .create(body.title)
        .await
        .map_err(ApiFailure::internal)?;
    Ok((StatusCode::CREATED, Json(json!({"session":session}))))
}
async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    let session = state.sessions.get(id).await.map_err(|e| {
        if e.to_string().contains("session not found") {
            ApiFailure::not_found(e.to_string())
        } else {
            ApiFailure::internal(e)
        }
    })?;
    Ok(Json(json!({"session":session})))
}
#[derive(Debug, Deserialize)]
struct RenameSessionBody {
    title: String,
}
async fn rename_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<RenameSessionBody>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    state
        .sessions
        .rename(id, body.title)
        .await
        .map_err(ApiFailure::internal)?;
    let session = state.sessions.get(id).await.map_err(ApiFailure::internal)?;
    Ok(Json(json!({"session":session})))
}
async fn archive_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    state
        .sessions
        .archive(id)
        .await
        .map_err(ApiFailure::internal)?;
    let session = state.sessions.get(id).await.map_err(ApiFailure::internal)?;
    Ok(Json(json!({"session":session})))
}

#[derive(Debug, Deserialize)]
struct MessageListParams {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct HistoryPageParams {
    limit: Option<usize>,
    before: Option<String>,
}

async fn list_messages(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<MessageListParams>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    let messages = state
        .sessions
        .messages(id, params.limit.unwrap_or(100).clamp(1, 500))
        .await
        .map_err(ApiFailure::internal)?;
    Ok(Json(json!({"messages":messages})))
}

async fn list_history_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<HistoryPageParams>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    let before = params
        .before
        .as_deref()
        .map(parse_message_id)
        .transpose()?;
    let (messages, next_before) = state
        .sessions
        .history_page(id, before, params.limit.unwrap_or(50).clamp(1, 100))
        .await
        .map_err(|error| match error {
            SessionError::HistoryCursorNotFound(_) | SessionError::NotFound(_) => {
                ApiFailure::not_found(error.to_string())
            }
            other => ApiFailure::internal(other),
        })?;
    Ok(Json(json!({
        "messages": messages,
        "next_before": next_before,
    })))
}

async fn list_assistant_activity(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<MessageListParams>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    state.sessions.get(id).await.map_err(ApiFailure::internal)?;
    let activity = state
        .sessions
        .assistant_activity(id, params.limit.unwrap_or(100).clamp(1, 500))
        .await
        .map_err(ApiFailure::internal)?;
    Ok(Json(json!({"activity": activity})))
}

#[derive(Debug, Deserialize)]
struct AppendMessageBody {
    text: String,
}

async fn append_message(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AppendMessageBody>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let id = parse_session_id(&id)?;
    let message = state
        .sessions
        .append_text(id, MessageRole::User, body.text)
        .await
        .map_err(ApiFailure::internal)?;
    Ok((StatusCode::CREATED, Json(json!({"message":message}))))
}

#[derive(Debug, Deserialize)]
struct UploadAttachmentParams {
    name: String,
}

async fn upload_draft_attachment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<UploadAttachmentParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let id = parse_session_id(&id)?;
    if body.is_empty() || body.len() > MAX_DRAFT_ATTACHMENT_BYTES {
        return Err(ApiFailure::unprocessable("attachment exceeds the supported size bound"));
    }
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    let attachment = state
        .sessions
        .create_draft_attachment(id, params.name, mime, body.to_vec())
        .await
        .map_err(attachment_failure)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"attachment":attachment})),
    ))
}

async fn list_draft_attachments(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    match state.sessions.draft_attachments(id).await {
        Ok(attachments) => Ok(Json(json!({
            "attachments":attachments,
            "available":true,
        }))),
        Err(SessionError::DraftAttachmentUnavailable) => Ok(Json(json!({
            "attachments":[],
            "available":false,
            "reason":"draft attachments are unavailable for this format-2 branch session until blob stores are unified",
        }))),
        Err(error) => Err(attachment_failure(error)),
    }
}

async fn delete_draft_attachment(
    State(state): State<AppState>,
    Path((id, attachment_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiFailure> {
    let id = parse_session_id(&id)?;
    let attachment_id = parse_attachment_id(&attachment_id)?;
    state
        .sessions
        .delete_draft_attachment(id, attachment_id)
        .await
        .map_err(attachment_failure)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct CreateArtifactBody {
    kind: ArtifactKind,
    title: String,
    language: Option<String>,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AppendArtifactVersionBody {
    content: String,
}

async fn list_artifacts(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    let artifacts = state
        .sessions
        .artifacts(id, 32)
        .await
        .map_err(artifact_failure)?;
    Ok(Json(json!({
        "available": true,
        "artifacts": artifacts,
        "run_available": false,
        "apply_available": false,
    })))
}

async fn create_artifact(
    State(state): State<AppState>,
    Path((id, message_id)): Path<(String, String)>,
    Json(body): Json<CreateArtifactBody>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let id = parse_session_id(&id)?;
    let message_id = parse_message_id(&message_id)?;
    let artifact = state
        .sessions
        .create_artifact(
            id,
            message_id,
            body.kind,
            body.title,
            body.language,
            body.content,
        )
        .await
        .map_err(artifact_failure)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "artifact": artifact,
            "run_available": false,
            "apply_available": false,
        })),
    ))
}

async fn get_artifact(
    State(state): State<AppState>,
    Path((id, artifact_id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    let artifact_id = parse_artifact_id(&artifact_id)?;
    let artifact = state
        .sessions
        .artifact(id, artifact_id)
        .await
        .map_err(artifact_failure)?;
    Ok(Json(json!({
        "artifact": artifact,
        "run_available": false,
        "apply_available": false,
    })))
}

async fn append_artifact_version(
    State(state): State<AppState>,
    Path((id, artifact_id)): Path<(String, String)>,
    Json(body): Json<AppendArtifactVersionBody>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let id = parse_session_id(&id)?;
    let artifact_id = parse_artifact_id(&artifact_id)?;
    let artifact = state
        .sessions
        .append_artifact_version(id, artifact_id, body.content)
        .await
        .map_err(artifact_failure)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "artifact": artifact,
            "run_available": false,
            "apply_available": false,
        })),
    ))
}

async fn branch_session(
    State(state): State<AppState>,
    Path((id, message_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let id = parse_session_id(&id)?;
    let message_id = parse_message_id(&message_id)?;
    let (session, fork) = state
        .sessions
        .branch_from_message(id, message_id)
        .await
        .map_err(branch_failure)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "session": session,
            "fork": {
                "parent_session_id": fork.parent_session_id,
                "fork_message_seq": fork.fork_message_seq,
                "boundary_message_id": message_id,
            }
        })),
    ))
}

async fn prepare_retry_branch(
    State(state): State<AppState>,
    Path((id, message_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let id = parse_session_id(&id)?;
    let message_id = parse_message_id(&message_id)?;
    let (session, fork, request_text) = state
        .sessions
        .prepare_retry_branch(id, message_id)
        .await
        .map_err(branch_failure)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "session": session,
            "fork": {
                "parent_session_id": fork.parent_session_id,
                "fork_message_seq": fork.fork_message_seq,
                "boundary_message_id": fork.boundary_message_id,
            },
            "request_text": request_text,
            "trigger_message_id": message_id,
        })),
    ))
}

async fn get_fork_provenance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    state.sessions.get(id).await.map_err(|error| {
        if error.to_string().contains("session not found") {
            ApiFailure::not_found(error.to_string())
        } else {
            ApiFailure::internal(error)
        }
    })?;
    let fork = state
        .sessions
        .fork_provenance(id)
        .await
        .map_err(ApiFailure::internal)?;
    Ok(Json(json!({"fork": fork.map(|fork| json!({
        "parent_session_id": fork.parent_session_id,
        "fork_message_seq": fork.fork_message_seq,
        "boundary_message_id": fork.boundary_message_id,
    }))})))
}

#[derive(Debug, Deserialize)]
struct CreateTurnBody {
    text: String,
    model: String,
    reasoning_effort: String,
}

async fn create_turn(
    runtime: Option<Extension<runtime_wiring::RuntimeWiring>>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CreateTurnBody>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let runtime = runtime.map(|Extension(runtime)| runtime);
    let _permit = acquire_http_turn_permit(runtime.as_ref())?;
    let sessions = runtime
        .as_ref()
        .map(|runtime| runtime.engine().sessions.clone())
        .unwrap_or_else(|| state.sessions.clone());
    let id = parse_session_id(&id)?;
    sessions.get(id).await.map_err(ApiFailure::internal)?;

    if body.text.trim().is_empty() {
        return Err(ApiFailure::bad_request("turn text must not be empty"));
    }
    let (provider_id, model_id) = body
        .model
        .split_once('/')
        .filter(|(provider, model)| !provider.is_empty() && !model.is_empty())
        .ok_or_else(|| ApiFailure::bad_request("model must use provider/model format"))?;
    if provider_id != "openai" {
        return Err(ApiFailure::bad_request(format!(
            "provider '{provider_id}' does not have a native turn adapter yet"
        )));
    }
    if !matches!(
        body.reasoning_effort.as_str(),
        "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
    ) {
        return Err(ApiFailure::bad_request("unsupported reasoning effort"));
    }

    let provider = OpenAiResponsesClient::from_env().map_err(provider_failure)?;
    let user_message = sessions
        .append_text(id, MessageRole::User, body.text)
        .await
        .map_err(ApiFailure::internal)?;
    publish_runtime_event(
        runtime.as_ref().map(|runtime| runtime.events()),
        event_bus::ServerEvent::MessageAppended {
            session: id,
            seq: 0,
        },
    );
    let history = sessions
        .messages(id, 500)
        .await
        .map_err(ApiFailure::internal)?;
    let mut input = responses_history(&history)?;
    if !history.iter().any(|message| message.id == user_message.id) {
        if input.len() == MAX_RESPONSES_INPUT_MESSAGES {
            input.remove(0);
        }
        let PayloadRef::Inline { text } = &user_message.body else {
            return Err(ApiFailure::internal(
                "newly appended user message was not stored inline",
            ));
        };
        input.push(ResponsesInput::new(ResponsesRole::User, text.clone()));
    }
    let assistant_text = provider
        .create(model_id, &body.reasoning_effort, &input)
        .await
        .map_err(provider_failure)?;
    let assistant_message = sessions
        .append_text(id, MessageRole::Assistant, assistant_text)
        .await
        .map_err(ApiFailure::internal)?;
    publish_runtime_event(
        runtime.as_ref().map(|runtime| runtime.events()),
        event_bus::ServerEvent::MessageAppended {
            session: id,
            seq: 0,
        },
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "user_message": user_message,
            "assistant_message": assistant_message
        })),
    ))
}

struct TurnStreamState {
    provider: OpenAiResponsesStream,
    sessions: SessionService,
    session_id: SessionId,
    user_message: Option<MessageRecord>,
    assistant_text: String,
    reasoning_summary: String,
    activity_tools: Vec<AssistantToolCall>,
    references: Vec<AssistantReference>,
    structured_activity_enabled: bool,
    stage: TurnStreamStage,
    _permit: HttpTurnPermit,
    events: Option<event_bus::EventBus>,
    /// Agentic loop state: provider tool schema + step budget + typed history.
    tools: Vec<ResponsesTool>,
    enabled_tools: Vec<String>,
    /// Permission broker consulted before every tool execution.
    broker: PermissionBroker,
    history_items: Vec<ResponsesItem>,
    loop_control: LoopController,
    pending_calls: Vec<RequestedCall>,
    executed_outputs: Vec<CallOutputItem>,
    emit_cursor: usize,
    model: String,
    reasoning_effort: String,
    /// Set when the loop must finalize with this stop reason (step cap hit).
    forced_stop: Option<TurnStop>,
    /// Agents-crate executor: mirror of the real agent loop plan (CONVERGENCE AGENTS).
    agent_plan: AgentExecutor,
    /// Stream-owned shell tasks. Dropping the stream drops these guards, which
    /// aborts their task and therefore drops the process-owning `ShellTool`.
    shell_guards: HashMap<String, ShellExecutionGuard>,
    /// Shell call IDs already admitted to the one-shot broker/startup path.
    prepared_shell_calls: HashSet<String>,
    /// Bounded explicit startup failures; shell calls never fall back to the
    /// uncancellable legacy executor.
    shell_startup_errors: HashMap<String, String>,
}

/// Turn-tool allowlist from OPENCODE_RK_TURN_TOOLS (comma-separated tool ids).
/// Empty by default: tool execution is opt-in, deny-by-default.
fn turn_tool_config() -> Vec<String> {
    std::env::var("OPENCODE_RK_TURN_TOOLS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// One executed (or explicitly denied) tool output awaiting persistence.
#[derive(Clone)]
struct CallOutputItem {
    call_id: String,
    name: String,
    output: String,
}

fn is_shell_call(name: &str) -> bool {
    matches!(name, "bash" | "shell")
}

fn shell_command(arguments: &str) -> Result<String, String> {
    let parsed: Value = serde_json::from_str(arguments)
        .map_err(|_| "shell arguments are not valid JSON".to_owned())?;
    let command = parsed
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "shell arguments require a string command".to_owned())?;
    if command.is_empty() {
        return Err("shell command must not be empty".to_owned());
    }
    if command.len() > MAX_SHELL_COMMAND_BYTES {
        return Err(format!(
            "shell command exceeds maximum of {MAX_SHELL_COMMAND_BYTES} bytes"
        ));
    }
    Ok(command.to_owned())
}

fn new_shell_execution(command: String) -> Result<ShellExecutionGuard, String> {
    let artifact = ShellArtifact::create()?;
    let (startup_tx, startup_rx) = mpsc::channel(1);
    let readiness_path = artifact.readiness_path.clone();
    let shell = ShellTool::new(
        "bash",
        vec![
            "-c".to_owned(),
            SHELL_STARTUP_WRAPPER.to_owned(),
            "--".to_owned(),
            readiness_path.to_string_lossy().into_owned(),
            command,
        ],
    );
    let config = ShellConfig {
        allowed_commands: vec!["bash".to_owned()],
        ..ShellConfig::default()
    };
    let execution = Box::pin(shell.execute_with_startup(config, readiness_path, startup_tx));
    Ok(ShellExecutionGuard {
        artifact,
        startup: startup_rx,
        execution: Some(execution),
        result: None,
    })
}

fn shell_result_output(result: &ShellResult) -> String {
    let mut output = result.stdout.clone();
    if !result.stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("[stderr]\n");
        output.push_str(&result.stderr);
    }
    if !result.success && output.is_empty() {
        output.push_str("shell command failed");
    }
    output
}

#[derive(Clone, Copy, PartialEq)]
enum TurnStreamStage {
    User,
    Provider,
    /// Dispatch accumulated [`TurnStreamState::pending_calls`] now.
    Executing,
    /// Emit one `tool_output` NDJSON event per executed call.
    EmitOutputs,
    /// Persist round outputs and start the next provider round (or finalize).
    NextRound,
    Done,
}

async fn create_turn_stream(
    runtime: Option<Extension<runtime_wiring::RuntimeWiring>>,
    structured_activity: Option<Extension<StructuredActivityEnabled>>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CreateTurnBody>,
) -> Result<Response, ApiFailure> {
    let runtime = runtime.map(|Extension(runtime)| runtime);
    let permit = acquire_http_turn_permit(runtime.as_ref())?;
    let events = runtime.as_ref().map(|runtime| runtime.events().clone());
    let sessions = runtime
        .as_ref()
        .map(|runtime| runtime.engine().sessions.clone())
        .unwrap_or_else(|| state.sessions.clone());
    let id = parse_session_id(&id)?;
    sessions.get(id).await.map_err(ApiFailure::internal)?;

    if body.text.trim().is_empty() {
        return Err(ApiFailure::bad_request("turn text must not be empty"));
    }
    let (provider_id, model_id) = body
        .model
        .split_once('/')
        .filter(|(provider, model)| !provider.is_empty() && !model.is_empty())
        .ok_or_else(|| ApiFailure::bad_request("model must use provider/model format"))?;
    if provider_id != "openai" {
        return Err(ApiFailure::bad_request(format!(
            "provider '{provider_id}' does not have a native turn adapter yet"
        )));
    }
    if !matches!(
        body.reasoning_effort.as_str(),
        "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
    ) {
        return Err(ApiFailure::bad_request("unsupported reasoning effort"));
    }

    let provider = OpenAiResponsesClient::from_env().map_err(provider_failure)?;
    let user_message = sessions
        .append_text(id, MessageRole::User, body.text)
        .await
        .map_err(ApiFailure::internal)?;
    publish_runtime_event(
        events.as_ref(),
        event_bus::ServerEvent::MessageAppended {
            session: id,
            seq: 0,
        },
    );
    let history = sessions
        .messages(id, 500)
        .await
        .map_err(ApiFailure::internal)?;
    let mut input = responses_history(&history)?;
    if !history.iter().any(|message| message.id == user_message.id) {
        if input.len() == MAX_RESPONSES_INPUT_MESSAGES {
            input.remove(0);
        }
        let PayloadRef::Inline { text } = &user_message.body else {
            return Err(ApiFailure::internal(
                "newly appended user message was not stored inline",
            ));
        };
        input.push(ResponsesInput::new(ResponsesRole::User, text.clone()));
    }

    // Agentic loop setup: advertise tools only when turn-tool execution is
    // explicitly enabled (OPENCODE_RK_TURN_TOOLS). The list is deny-by-default:
    // an advertised tool is executable, an unadvertised one is not.
    let turn_tools = turn_tool_config();
    let registry = ToolRegistry::new();
    let runtime_engine = runtime.as_ref().map(|runtime| runtime.engine());
    let enabled_tools: Vec<String> = registry
        .list()
        .into_iter()
        .filter(|tool| {
            turn_tools.iter().any(|name| *name == tool.id)
                && runtime_engine.map_or(true, |engine| {
                    engine
                        .tools
                        .iter()
                        .any(|snapshot| snapshot.enabled && snapshot.id == tool.id)
                        && engine.policy.decision(&tool.id) == PolicyDecision::Allow
                })
        })
        .map(|tool| tool.id.clone())
        .collect();
    let tools: Vec<ResponsesTool> = registry
        .list()
        .into_iter()
        .filter(|tool| enabled_tools.iter().any(|name| *name == tool.id))
        .map(|tool| {
            ResponsesTool::function(
                tool.id.clone(),
                tool.description.clone(),
                json!({ "type": "object", "properties": {} }),
            )
        })
        .collect();
    let max_steps = std::env::var("OPENCODE_RK_TURN_MAX_STEPS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|steps| *steps > 0)
        .unwrap_or(MAX_TURN_STEPS);
    let history_items: Vec<ResponsesItem> = input.into_iter().map(Into::into).collect();

    let provider = provider
        .stream_with_tools(model_id, &body.reasoning_effort, &history_items, &tools)
        .await
        .map_err(provider_failure)?;

    let stream = stream::unfold(
        TurnStreamState {
            provider,
            sessions,
            session_id: id,
            user_message: Some(user_message),
            assistant_text: String::new(),
            reasoning_summary: String::new(),
            activity_tools: Vec::new(),
            references: Vec::new(),
            structured_activity_enabled: structured_activity.is_some(),
            stage: TurnStreamStage::User,
            _permit: permit,
            events,
            tools,
            enabled_tools,
            broker: PermissionBroker::new(SecurityPolicy::lean_default(
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            )),
            history_items,
            loop_control: LoopController::with_cap(max_steps),
            pending_calls: Vec::new(),
            executed_outputs: Vec::new(),
            emit_cursor: 0,
            model: model_id.to_owned(),
            reasoning_effort: body.reasoning_effort.clone(),
            forced_stop: None,
            agent_plan: AgentExecutor::new(vec![
                opencode_rk_agents::agent_executor::LoopStep::ProviderCall,
                opencode_rk_agents::agent_executor::LoopStep::ToolDispatch,
                opencode_rk_agents::agent_executor::LoopStep::PolicyCheck,
                opencode_rk_agents::agent_executor::LoopStep::Settle,
            ]).expect("fixed 4-step turn plan fits agent executor capacity"),
            shell_guards: HashMap::new(),
            prepared_shell_calls: HashSet::new(),
            shell_startup_errors: HashMap::new(),
        },
        |mut state| async move {
            loop {
                match state.stage {
                    TurnStreamStage::User => {
                        state.stage = TurnStreamStage::Provider;
                        let message = state
                            .user_message
                            .take()
                            .expect("stream user message is emitted once");
                        return Some((
                            Ok::<Bytes, Infallible>(ndjson(json!({
                                "type": "user_message",
                                "message": message,
                            }))),
                            state,
                        ));
                    }
                    TurnStreamStage::Provider => {
                        let event = state.provider.next_event().await;
                        match event {
                        Ok(Some(ResponsesStreamEvent::OutputTextDelta(delta))) => {
                            state.assistant_text.push_str(&delta);
                            return Some((
                                Ok::<Bytes, Infallible>(ndjson(json!({
                                    "type": "assistant_delta",
                                    "delta": delta,
                                }))),
                                state,
                            ));
                        }
                        Ok(Some(ResponsesStreamEvent::ReasoningSummaryDelta(delta))) => {
                            if state.reasoning_summary.len().saturating_add(delta.len())
                                > MAX_REASONING_SUMMARY_BYTES
                            {
                                state.stage = TurnStreamStage::Done;
                                return Some((
                                    Ok::<Bytes, Infallible>(stream_error(
                                        "resource_exhausted",
                                        "reasoning summary exceeds the transcript activity bound",
                                    )),
                                    state,
                                ));
                            }
                            state.reasoning_summary.push_str(&delta);
                            return Some((
                                Ok::<Bytes, Infallible>(ndjson(json!({
                                    "type": "reasoning_summary_delta",
                                    "delta": delta,
                                }))),
                                state,
                            ));
                        }
                        Ok(Some(ResponsesStreamEvent::UrlCitation { label, url })) => {
                            if !state.structured_activity_enabled {
                                state.stage = TurnStreamStage::Done;
                                return Some((
                                    Ok::<Bytes, Infallible>(stream_error(
                                        "bad_gateway",
                                        "unsupported provider stream event: response.output_text.annotation.added",
                                    )),
                                    state,
                                ));
                            }
                            state.references.push(AssistantReference {
                                label: label.clone(),
                                url: url.clone(),
                            });
                            return Some((
                                Ok::<Bytes, Infallible>(ndjson(json!({
                                    "type": "reference",
                                    "label": label,
                                    "url": url,
                                }))),
                                state,
                            ));
                        }
                        Ok(Some(ResponsesStreamEvent::FunctionCall {
                            call_id,
                            name,
                            arguments,
                        })) => {
                            if state
                                .activity_tools
                                .len()
                                .saturating_add(state.pending_calls.len())
                                >= MAX_ASSISTANT_TOOL_CALLS
                            {
                                state.stage = TurnStreamStage::Done;
                                return Some((
                                    Ok::<Bytes, Infallible>(stream_error(
                                        "resource_exhausted",
                                        "tool activity exceeds the durable turn bound",
                                    )),
                                    state,
                                ));
                            }
                            if is_shell_call(&name)
                                && state.enabled_tools.iter().any(|enabled| enabled == &name)
                            {
                                let already_prepared =
                                    !state.prepared_shell_calls.insert(call_id.clone());
                                if already_prepared {
                                    state.shell_startup_errors.insert(
                                        call_id.clone(),
                                        "duplicate shell call ID was not executed".to_owned(),
                                    );
                                } else if state.shell_guards.len() >= MAX_CALLS_PER_ROUND {
                                    state.shell_startup_errors.insert(
                                        call_id.clone(),
                                        format!(
                                            "shell call limit exceeded (max {MAX_CALLS_PER_ROUND})"
                                        ),
                                    );
                                } else {
                                    match shell_command(&arguments) {
                                        Ok(command) => {
                                            match state.broker.authorize(&OperationIntent::Tool {
                                                name: name.clone(),
                                                description: "turn tool call".to_owned(),
                                            }) {
                                                Decision::Allow => {
                                                    match new_shell_execution(command) {
                                                        Ok(mut guard) => {
                                                            match guard.wait_for_startup().await {
                                                                Ok(_pid) => {
                                                                    state.shell_guards.insert(
                                                                        call_id.clone(),
                                                                    guard,
                                                                    );
                                                                }
                                                                Err(error) => {
                                                                    state.shell_startup_errors.insert(
                                                                        call_id.clone(),
                                                                        bounded_shell_error(error),
                                                                    );
                                                                }
                                                            }
                                                        }
                                                        Err(error) => {
                                                            state.shell_startup_errors.insert(
                                                                call_id.clone(),
                                                                bounded_shell_error(error),
                                                            );
                                                        }
                                                    }
                                                }
                                                Decision::Deny { reason } => {
                                                    state.shell_startup_errors.insert(
                                                        call_id.clone(),
                                                        format!(
                                                            "error: tool '{}' denied: {}",
                                                            name, reason
                                                        ),
                                                    );
                                                }
                                                Decision::RequireHuman { reason, .. } => {
                                                    state.shell_startup_errors.insert(
                                                        call_id.clone(),
                                                        format!(
                                                            "error: tool '{}' requires human approval: {}",
                                                            name, reason
                                                        ),
                                                    );
                                                }
                                            }
                                        }
                                        Err(error) => {
                                            state.shell_startup_errors.insert(
                                                call_id.clone(),
                                                bounded_shell_error(error),
                                            );
                                        }
                                    }
                                }
                            }
                            state.pending_calls.push(RequestedCall {
                                call_id: call_id.clone(),
                                name: name.clone(),
                                arguments: arguments.clone(),
                            });
                            // Replay rule: the next round's input must contain
                            // the model's function_call before its output.
                            state
                                .history_items
                                .push(ResponsesItem::FunctionCall {
                                    call_id: call_id.clone(),
                                    name: name.clone(),
                                    arguments: arguments.clone(),
                                });
                            return Some((
                                Ok::<Bytes, Infallible>(ndjson(json!({
                                    "type": "tool_call",
                                    "call_id": call_id,
                                    "name": name,
                                    "arguments": arguments,
                                }))),
                                state,
                            ));
                        }
                        Ok(Some(ResponsesStreamEvent::Completed { stop_reason })) => {
                            // Terminal provider event: either the turn ends
                            // here or the accumulated tool calls execute and
                            // the loop continues with the next round.
                            if !state.pending_calls.is_empty() {
                                if !state.loop_control.can_start_next_round() {
                                    let stop = TurnStop::MaxSteps {
                                        steps: state.loop_control.max_steps(),
                                    };
                                    state.stage = TurnStreamStage::NextRound;
                                    state.forced_stop = Some(stop);
                                    continue;
                                }
                                state.stage = TurnStreamStage::Executing;
                                continue;
                            }
                            let stop = match stop_reason {
                                ResponsesStopReason::Completed => TurnStop::Completed,
                                ResponsesStopReason::Incomplete { reason } => {
                                    TurnStop::Incomplete { reason }
                                }
                            };
                            let assistant_text = std::mem::take(&mut state.assistant_text);
                            let reasoning_summary = std::mem::take(&mut state.reasoning_summary);
                            let stop_reason = stop.as_str();
                            let persisted_summary = (!reasoning_summary.is_empty())
                                .then_some(reasoning_summary);
                            let activity_tools = state.activity_tools.clone();
                            let references = state.references.clone();
                            match state
                                .sessions
                                .append_assistant_with_activity(
                                    state.session_id,
                                    assistant_text,
                                    persisted_summary.clone(),
                                    activity_tools,
                                    references.clone(),
                                )
                                .await
                            {
                                Ok(message) => {
                                    publish_runtime_event(
                                        state.events.as_ref(),
                                        event_bus::ServerEvent::MessageAppended {
                                            session: state.session_id,
                                            seq: 0,
                                        },
                                    );
                                    state.stage = TurnStreamStage::Done;
                                    return Some((
                                        Ok::<Bytes, Infallible>(ndjson(json!({
                                            "type": "assistant_message",
                                            "message": message,
                                            "reasoning_summary": persisted_summary,
                                            "references": references,
                                            "stop_reason": stop_reason,
                                        }))),
                                        state,
                                    ));
                                }
                                Err(error) => {
                                    state.stage = TurnStreamStage::Done;
                                    return Some((
                                        Ok::<Bytes, Infallible>(stream_error(
                                            "internal_error",
                                            error.to_string(),
                                        )),
                                        state,
                                    ));
                                }
                            }
                        }
                        Ok(None) => {
                            state.stage = TurnStreamStage::Done;
                            return Some((
                                Ok::<Bytes, Infallible>(stream_error(
                                    "bad_gateway",
                                    "provider stream ended before completion",
                                )),
                                state,
                            ));
                        }
                        Err(error) => {
                            let failure = provider_failure(error);
                            state.stage = TurnStreamStage::Done;
                            return Some((
                                Ok::<Bytes, Infallible>(stream_error(
                                    failure.code,
                                    failure.message,
                                )),
                                state,
                            ));
                        }
                        }
                    }                    TurnStreamStage::Executing => {
                            // One round = one provider stream + at most one
                            // tool dispatch batch. The budget was checked at
                            // the completed event; consume it here.
                            let _ = state.loop_control.begin_round();
                            let executor = ToolExecutor::new();
                        let mut round_outputs: Vec<CallOutputItem> = Vec::new();
                        let mut batch = std::mem::take(&mut state.pending_calls);
                        // Truncate oversized batches with explicit error outputs.
                        let (kept, overflow) = state.loop_control.truncate_calls(&batch);
                        batch = kept;
                        for item in overflow {
                            round_outputs.push(CallOutputItem {
                                call_id: item.call_id,
                                name: String::new(),
                                output: item.output,
                            });
                        }
                        for call in batch {
                            let permitted = state
                                .enabled_tools
                                .iter()
                                .any(|enabled| *enabled == call.name);
                            let raw_output = if permitted {
                                if is_shell_call(&call.name) {
                                    if let Some(error) =
                                        state.shell_startup_errors.remove(&call.call_id)
                                    {
                                        error
                                    } else if let Some(guard) =
                                        state.shell_guards.remove(&call.call_id)
                                    {
                                        match guard.finish().await {
                                            Ok(result) => shell_result_output(&result),
                                            Err(error) => bounded_shell_error(error),
                                        }
                                    } else {
                                        "error: shell execution was not prepared".to_owned()
                                    }
                                } else {
                                    match state.broker.authorize(&OperationIntent::Tool {
                                        name: call.name.clone(),
                                        description: "turn tool call".to_owned(),
                                    }) {
                                        Decision::Allow => {
                                            let arguments: Value =
                                                serde_json::from_str(&call.arguments)
                                                    .unwrap_or_else(|_| json!({}));
                                            let result = executor
                                                .execute_authorized(
                                                    opencode_rk_tools::executor::ToolCall::new(
                                                        call.call_id.clone(),
                                                        call.name.clone(),
                                                        arguments,
                                                    ),
                                                    &state.broker,
                                                )
                                                .await;
                                            if result.success {
                                                result.output
                                            } else {
                                                result
                                                    .error
                                                    .unwrap_or_else(|| "tool failed".to_owned())
                                            }
                                        }
                                        Decision::Deny { reason } => {
                                            format!(
                                                "error: tool '{}' denied: {}",
                                                call.name, reason
                                            )
                                        }
                                        Decision::RequireHuman { reason, .. } => {
                                            format!(
                                                "error: tool '{}' requires human approval: {}",
                                                call.name, reason
                                            )
                                        }
                                    }
                                }
                            } else {
                                format!(
                                    "error: tool '{}' is not permitted by turn policy (enable via OPENCODE_RK_TURN_TOOLS)",
                                    call.name
                                )
                            };
                            let bounded = truncate_tool_output(&raw_output);
                            round_outputs.push(CallOutputItem {
                                call_id: call.call_id.clone(),
                                name: call.name,
                                output: bounded,
                            });
                        }
                        state.executed_outputs = round_outputs;
                        state.stage = TurnStreamStage::EmitOutputs;
                        continue;
                    }
                    TurnStreamStage::EmitOutputs => {
                            if state.emit_cursor < state.executed_outputs.len() {
                                let item = state.executed_outputs[state.emit_cursor].clone();
                                state.emit_cursor += 1;
                                return Some((
                                    Ok::<Bytes, Infallible>(ndjson(json!({
                                        "type": "tool_output",
                                        "call_id": item.call_id,
                                        "name": item.name,
                                        "output": item.output,
                                    }))),
                                    state,
                                ));
                            }
                            state.emit_cursor = 0;
                            state.stage = TurnStreamStage::NextRound;
                            continue;
                        }
                    TurnStreamStage::NextRound => {
                        // Persist round outputs (tool transcript rows) and
                        // either start the next provider round or finalize
                        // with the forced stop reason.
                        let outputs = std::mem::take(&mut state.executed_outputs);
                        for item in &outputs {
                            let record = state
                                .sessions
                                .append_text(
                                    state.session_id,
                                    MessageRole::Tool,
                                    format!("[{}] {}", item.name, item.output),
                                )
                                .await;
                            if record.is_err() {
                                state.stage = TurnStreamStage::Done;
                                return Some((
                                    Ok::<Bytes, Infallible>(stream_error(
                                        "internal_error",
                                        "failed to persist tool output",
                                    )),
                                    state,
                                ));
                            }
                            if !item.name.is_empty() {
                                publish_runtime_event(
                                    state.events.as_ref(),
                                    event_bus::ServerEvent::ToolExecuted {
                                        name: item.name.clone(),
                                        duration_ms: 0,
                                    },
                                );
                            }
                            publish_runtime_event(
                                state.events.as_ref(),
                                event_bus::ServerEvent::MessageAppended {
                                    session: state.session_id,
                                    seq: 0,
                                },
                            );
                            state.history_items.push(ResponsesItem::FunctionCallOutput {
                                call_id: item.call_id.clone(),
                                output: item.output.clone(),
                            });
                            if !item.name.is_empty() {
                                state.activity_tools.push(AssistantToolCall {
                                    call_id: item.call_id.clone(),
                                    name: item.name.clone(),
                                    state: if item.output.starts_with("error:") {
                                        "failed".to_owned()
                                    } else {
                                        "completed".to_owned()
                                    },
                                    ok: !item.output.starts_with("error:"),
                                });
                            }
                        }
                        if let Some(stop) = state.forced_stop.take() {
                            let assistant_text = std::mem::take(&mut state.assistant_text);
                            let reasoning_summary =
                                std::mem::take(&mut state.reasoning_summary);
                            let persisted_summary =
                                (!reasoning_summary.is_empty()).then_some(reasoning_summary);
                            let activity_tools = state.activity_tools.clone();
                            let references = state.references.clone();
                            match state
                                .sessions
                                .append_assistant_with_activity(
                                    state.session_id,
                                    assistant_text,
                                    persisted_summary.clone(),
                                    activity_tools,
                                    references.clone(),
                                )
                                .await
                            {
                                Ok(message) => {
                                    publish_runtime_event(
                                        state.events.as_ref(),
                                        event_bus::ServerEvent::MessageAppended {
                                            session: state.session_id,
                                            seq: 0,
                                        },
                                    );
                                    state.stage = TurnStreamStage::Done;
                                    return Some((
                                        Ok::<Bytes, Infallible>(ndjson(json!({
                                            "type": "assistant_message",
                                            "message": message,
                                            "reasoning_summary": persisted_summary,
                                            "references": references,
                                            "stop_reason": stop.as_str(),
                                        }))),
                                        state,
                                    ));
                                }
                                Err(error) => {
                                    state.stage = TurnStreamStage::Done;
                                    return Some((
                                        Ok::<Bytes, Infallible>(stream_error(
                                            "internal_error",
                                            error.to_string(),
                                        )),
                                        state,
                                    ));
                                }
                            }
                        }
                        // Next provider round with the grown typed history.
                        let client = match OpenAiResponsesClient::from_env() {
                            Ok(client) => client,
                            Err(error) => {
                                state.stage = TurnStreamStage::Done;
                                let failure = provider_failure(error);
                                return Some((Ok::<Bytes, Infallible>(stream_error(failure.code, failure.message)), state));
                            }
                        };
                        match client
                            .stream_with_tools(
                                &state.model,
                                &state.reasoning_effort,
                                &state.history_items,
                                &state.tools,
                            )
                            .await
                        {
                            Ok(next_stream) => {
                                state.provider = next_stream;
                                state.assistant_text.clear();
                                state.stage = TurnStreamStage::Provider;
                                continue;
                            }
                            Err(error) => {
                                state.stage = TurnStreamStage::Done;
                                let failure = provider_failure(error);
                                return Some((Ok::<Bytes, Infallible>(stream_error(failure.code, failure.message)), state));
                            }
                        }
                    }
                    TurnStreamStage::Done => return None,
                }
            }
        },
    );

    Ok((
        StatusCode::CREATED,
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

fn ndjson(value: Value) -> Bytes {
    let mut bytes = serde_json::to_vec(&value).expect("stream event serialization");
    bytes.push(b'\n');
    Bytes::from(bytes)
}

fn stream_error(code: &'static str, message: impl Into<String>) -> Bytes {
    ndjson(json!({
        "type": "error",
        "code": code,
        "message": message.into(),
    }))
}

fn responses_history(
    history: &[opencode_rk_contracts::MessageRecord],
) -> Result<Vec<ResponsesInput>, ApiFailure> {
    let start = history.len().saturating_sub(MAX_RESPONSES_INPUT_MESSAGES);
    history[start..]
        .iter()
        .map(|message| {
            let role = match message.role {
                MessageRole::System => ResponsesRole::System,
                MessageRole::User => ResponsesRole::User,
                MessageRole::Assistant => ResponsesRole::Assistant,
                MessageRole::Tool => {
                    return Err(ApiFailure::bad_request(
                        "tool transcript entries need a native Responses tool adapter",
                    ));
                }
            };
            let PayloadRef::Inline { text } = &message.body else {
                return Err(ApiFailure::bad_request(
                    "blob transcript entries need a native Responses attachment adapter",
                ));
            };
            Ok(ResponsesInput::new(role, text.clone()))
        })
        .collect()
}

fn provider_failure(error: ResponsesError) -> ApiFailure {
    match error {
        ResponsesError::MissingCredential(_) | ResponsesError::InvalidConfig(_) => {
            ApiFailure::service_unavailable(error.to_string())
        }
        ResponsesError::EmptyModel
        | ResponsesError::UnsupportedReasoningEffort(_)
        | ResponsesError::TooManyMessages { .. }
        | ResponsesError::InputTooLarge { .. } => ApiFailure::bad_request(error.to_string()),
        ResponsesError::Request(_)
        | ResponsesError::Upstream { .. }
        | ResponsesError::ResponseTooLarge { .. }
        | ResponsesError::InvalidJson(_)
        | ResponsesError::EmptyOutput
        | ResponsesError::OutputTooLarge { .. }
        | ResponsesError::ReasoningSummaryTooLarge { .. }
        | ResponsesError::UnsupportedStreamEvent(_)
        | ResponsesError::StreamFailed(_)
        | ResponsesError::UnexpectedStreamEnd => ApiFailure::bad_gateway(error.to_string()),
    }
}
fn parse_session_id(value: &str) -> Result<SessionId, ApiFailure> {
    SessionId::from_str(value).map_err(|_| ApiFailure::bad_request("invalid session id"))
}
fn parse_message_id(value: &str) -> Result<MessageId, ApiFailure> {
    MessageId::from_str(value).map_err(|_| ApiFailure::bad_request("invalid message id"))
}
fn parse_attachment_id(value: &str) -> Result<AttachmentId, ApiFailure> {
    AttachmentId::from_str(value).map_err(|_| ApiFailure::bad_request("invalid attachment id"))
}
fn parse_artifact_id(value: &str) -> Result<ArtifactId, ApiFailure> {
    ArtifactId::from_str(value).map_err(|_| ApiFailure::bad_request("invalid artifact id"))
}
fn attachment_failure(error: SessionError) -> ApiFailure {
    match error {
        SessionError::NotFound(_) | SessionError::DraftAttachmentNotFound(_) => {
            ApiFailure::not_found(error.to_string())
        }
        SessionError::DraftAttachmentUnavailable
        | SessionError::DraftAttachmentTooLarge
        | SessionError::InvalidDraftAttachment => ApiFailure::unprocessable(error.to_string()),
        other => ApiFailure::internal(other),
    }
}
fn branch_failure(error: SessionError) -> ApiFailure {
    match error {
        SessionError::BranchMessageNotFound(_) => ApiFailure::not_found(error.to_string()),
        SessionError::InvalidBranchBoundary
        | SessionError::ForkHistoryTooLarge
        | SessionError::ForkDepthExceeded
        | SessionError::BranchPayloadUnsupported => ApiFailure::unprocessable(error.to_string()),
        SessionError::BranchingUnavailable => ApiFailure::service_unavailable(error.to_string()),
        other => ApiFailure::internal(other),
    }
}
fn artifact_failure(error: SessionError) -> ApiFailure {
    match error {
        SessionError::NotFound(_) | SessionError::ArtifactNotFound(_) => {
            ApiFailure::not_found(error.to_string())
        }
        SessionError::ArtifactSourceInvalid(_)
        | SessionError::ArtifactContentTooLarge
        | SessionError::ArtifactLimitExceeded
        | SessionError::InvalidArtifact => ApiFailure::unprocessable(error.to_string()),
        other => ApiFailure::internal(other),
    }
}
#[derive(Debug)]
struct ApiFailure {
    status: StatusCode,
    code: &'static str,
    message: String,
}
impl ApiFailure {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: message.into(),
        }
    }
    fn unprocessable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "unprocessable_entity",
            message: message.into(),
        }
    }
    fn too_many_requests(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "too_many_requests",
            message: message.into(),
        }
    }
    fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "service_unavailable",
            message: message.into(),
        }
    }
    fn bad_gateway(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            code: "bad_gateway",
            message: message.into(),
        }
    }
    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: error.to_string(),
        }
    }
}
#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}
impl IntoResponse for ApiFailure {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            code: self.code,
            message: &self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod broker_gate_tests {
    use super::*;

    #[test]
    fn tool_intent_deny_produces_denial_output_without_execute() {
        let broker = PermissionBroker::new(SecurityPolicy::lean_default("/work/project"))
            .with_permissions(opencode_rk_security::PermissionSet::new(vec![
                opencode_rk_security::PermissionRule::new("blocked-tool", opencode_rk_security::RuleEffect::Deny),
            ]));
        let allowed = broker.authorize(&OperationIntent::Tool {
            name: "allowed-tool".to_owned(),
            description: "turn tool call".to_owned(),
        });
        assert_eq!(allowed, Decision::Allow);
        let denied = broker.authorize(&OperationIntent::Tool {
            name: "blocked-tool".to_owned(),
            description: "turn tool call".to_owned(),
        });
        assert!(matches!(denied, Decision::Deny { .. }));
    }
}
