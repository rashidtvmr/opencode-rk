//! Small HTTP boundary over the native catalog and session services.
#![forbid(unsafe_code)]
pub mod acp_bridge;
pub mod acp_files;
pub mod acp_session;
pub mod admission_bounds;
pub mod agent_loop;
pub mod app_client;
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
mod browser_resume;
pub mod chat_composer;
pub mod clients;
pub mod context_accounting;
pub mod context_report;
pub mod control_decode;
pub mod control_plane_errors;
pub mod control_plane_exposure;
pub mod control_plane_inputs;
pub mod daemon;
pub mod daemon_auth;
pub mod desktop_bridge;
pub mod enterprise_link;
pub mod error_translate;
pub mod event_bus;
pub mod event_cursor;
pub mod event_http;
pub mod event_stream;
pub mod loop_driver;
pub mod mcp_http;
pub mod origin_check;
pub mod protocol_api;
pub mod rel_stamp;
pub mod rel_verify;
pub mod remote_approvals;
pub mod remote_connector;
pub mod remote_files;
pub mod remote_ledger;
pub mod remote_pty;
pub mod remote_recovery;
pub mod remote_revocation;
pub mod remote_sessions;
pub mod remote_sync;
pub mod remote_turns;
pub mod repo_ops;
pub mod route_table;
pub mod rules_globs;
pub mod rules_loader;
pub mod run_owner;
pub mod runtime_wiring;
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
pub mod web_turn_adapter;
pub mod workspace_proxy;
pub mod workspace_sessions;
use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Extension, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use futures_util::{stream, StreamExt};
use opencode_rk_agents::agent_executor::AgentExecutor;
use opencode_rk_catalog::{Catalog, CatalogQuery};
use opencode_rk_contracts::{
    ArtifactId, ArtifactKind, AttachmentId, MessageId, MessageRecord, MessageRole, PayloadRef,
    SessionId, Timestamp, MAX_DRAFT_ATTACHMENT_BYTES, WIRE_SCHEMA_VERSION,
};
use opencode_rk_providers::responses::{
    OpenAiResponsesClient, OpenAiResponsesStream, ResponsesError, ResponsesItem, ResponsesRole,
    ResponsesStopReason, ResponsesStreamEvent, ResponsesTool, MAX_RESPONSES_INPUT_BYTES,
    MAX_RESPONSES_INPUT_MESSAGES,
};
use opencode_rk_providers::custom_provider::{self, CustomProviderConfig};
use opencode_rk_security::{Decision, OperationIntent, PermissionBroker, SecurityPolicy};
use opencode_rk_sessions::{
    HistoryItem, SessionError, SessionService, ToolCallAdmission, ToolRound,
};
use opencode_rk_tools::executor::ToolExecutor;
use opencode_rk_tools::file_ops::{FileOperation, FileTool};
use opencode_rk_tools::registry::ToolRegistry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{convert::Infallible, path::PathBuf, str::FromStr, sync::Arc};

use run_owner::{
    ApprovedPermission, PendingPermissionGuard, PermissionReply, PermissionRequest,
    PermissionSource, RunOwner, TurnExecutionPermit,
};

#[derive(Clone)]
pub struct AppState {
    pub sessions: SessionService,
    pub catalog: Arc<Catalog>,
}
pub fn router(state: AppState) -> Router {
    router_with_auth(state, None)
}

/// Authenticated router: `Some(auth)` gates every `/api/*` route with the
/// bearer middleware; `None` is the legacy unauthenticated router used by
/// frozen tests. The serve path always passes `Some`.
pub fn router_with_auth(state: AppState, auth: Option<daemon_auth::DaemonAuth>) -> Router {
    let events = app_runtime::event_bus();
    // HTTP admission cap is enforced authoritatively inside
    // `router_with_events` (delegation); this constructor no longer owns an
    // out-of-band cap setter that an injected-bus composition could skip.
    router_with_events(state, auth, events)
}

/// Authenticated router with an explicitly owned event bus (`events` is
/// injected into the SSE handler and the durable turn handlers via
/// `Extension`, so two routers build two independent buses and never share
/// streams). `router_with_auth` delegates here with a fresh bus.
pub fn router_with_events(
    state: AppState,
    auth: Option<daemon_auth::DaemonAuth>,
    events: event_bus::EventBus,
) -> Router {
    // Authoritative HTTP admission cap for EVERY router composition,
    // including injected-bus production builds: lower-only (a preconfigured
    // smaller cap is preserved, never widened). Raw `EventBus::subscribe`
    // (SDK/internal callers) stays outside this HTTP-bound cap by design.
    if events.max_subscribers() > event_http::MAX_HTTP_SUBSCRIBERS {
        events.set_max_subscribers(event_http::MAX_HTTP_SUBSCRIBERS);
    }
    // One MCP status registry per router/daemon: constructed here and
    // injected as an `Extension`, so two routers never share entries and
    // there is no static global. Fresh router ⇒ empty registry ⇒ `{}`.
    let mcp_registry = mcp_http::McpStatusRegistry::new();
    let app = Router::new()
        .route("/health", get(health))
        .route(
            "/auth/browser-resume",
            post(issue_browser_resume).delete(revoke_browser_resume),
        )
        .route(
            "/auth/browser-resume/exchange",
            post(exchange_browser_resume),
        )
        .route("/api/capabilities", get(web_capabilities))
        .route("/api/workspaces", get(list_workspaces))
        .route("/api/models", get(search_models))
        .route("/api/models/{provider}/{model}", get(get_model))
        .route("/api/providers", get(list_custom_providers))
        .route("/api/providers/{id}", post(save_custom_provider))
        .route("/api/providers/{id}/models", get(discover_custom_provider_models))
        // Authenticated MCP status/lifecycle projection. Registered ahead of
        // the SPA asset fallback, so it can never be served by the public
        // fallback, and its `/api/*` prefix puts it under `require_bearer`
        // exactly like `/api/models`.
        .route(
            "/api/mcp",
            get(mcp_http::status)
                .post(mcp_http::add)
                .layer(DefaultBodyLimit::max(mcp_http::MAX_ADD_BODY_BYTES)),
        )
        .route("/api/mcp/{name}/connect", post(mcp_http::connect))
        .route("/api/mcp/{name}/disconnect", post(mcp_http::disconnect))
        .route(
            "/auth/{provider}",
            axum::routing::put(save_provider_auth)
                .layer(axum::extract::DefaultBodyLimit::max(32 * 1024)),
        )
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
        .route("/api/session/{id}/interrupt", post(interrupt_session))
        .route("/api/event", get(event_http::handle_events_sse))
        .route("/events", get(event_http::handle_events_sse))
        .route(
            "/api/session/{id}/permission",
            get(list_session_permissions),
        )
        .route(
            "/api/session/{id}/permission/{request_id}/reply",
            post(reply_session_permission),
        )
        .fallback(web_assets::serve)
        .layer(axum::extract::Extension(events))
        .layer(axum::extract::Extension(mcp_registry))
        .with_state(state);
    match auth {
        Some(credential) => app
            .layer(axum::extract::Extension(credential.clone()))
            .layer(axum::middleware::from_fn_with_state(
                credential,
                daemon_auth::require_bearer,
            )),
        None => app,
    }
}

#[derive(Deserialize)]
struct BrowserResumeIssueBody {
    ttl_ms: Option<u64>,
}

async fn issue_browser_resume(
    Extension(auth): Extension<daemon_auth::DaemonAuth>,
    headers: HeaderMap,
    body: Option<Json<BrowserResumeIssueBody>>,
) -> Response {
    let Some(origin) = valid_browser_origin(&headers) else {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"message":"browser origin denied"})),
        )
            .into_response();
    };
    let native = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    if !auth.verify_bearer(native) {
        let status = if native.is_some() {
            StatusCode::FORBIDDEN
        } else {
            StatusCode::UNAUTHORIZED
        };
        return (
            status,
            Json(json!({"message":"trusted native launch required"})),
        )
            .into_response();
    }
    let ttl_ms = body
        .and_then(|Json(value)| value.ttl_ms)
        .unwrap_or(browser_resume::MAX_LEASE_MS);
    match auth.browser_resume().issue(&origin, ttl_ms) {
        Ok((browser_capability, expires_at_unix_ms)) => (StatusCode::CREATED, Json(json!({"browser_capability":browser_capability,"expires_at_unix_ms":expires_at_unix_ms}))).into_response(),
        Err(browser_resume::IssueError::InvalidTtl) => (StatusCode::BAD_REQUEST, Json(json!({"message":"invalid browser resume TTL"}))).into_response(),
        Err(browser_resume::IssueError::Capacity) => (StatusCode::TOO_MANY_REQUESTS, Json(json!({"message":"browser resume capacity reached"}))).into_response(),
        Err(browser_resume::IssueError::Clock | browser_resume::IssueError::Randomness(_)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"message":"browser resume credential unavailable"}))).into_response(),
    }
}

async fn exchange_browser_resume(
    Extension(registry): Extension<Arc<browser_resume::BrowserResumeRegistry>>,
    headers: HeaderMap,
) -> Response {
    let Some(origin) = valid_browser_origin(&headers) else {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"message":"browser origin denied"})),
        )
            .into_response();
    };
    let Some(capability) = resume_authorization(&headers) else {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"message":"browser resume denied"})),
        )
            .into_response();
    };
    match registry.exchange(capability, &origin) {
        Some((browser_capability, expires_at_unix_ms)) => (StatusCode::OK, Json(json!({"browser_capability":browser_capability,"expires_at_unix_ms":expires_at_unix_ms}))).into_response(),
        None => (StatusCode::FORBIDDEN, Json(json!({"message":"browser resume denied"}))).into_response(),
    }
}

async fn revoke_browser_resume(
    Extension(registry): Extension<Arc<browser_resume::BrowserResumeRegistry>>,
    headers: HeaderMap,
) -> Response {
    let Some(origin) = valid_browser_origin(&headers) else {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"message":"browser origin denied"})),
        )
            .into_response();
    };
    let Some(capability) = resume_authorization(&headers) else {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"message":"browser resume denied"})),
        )
            .into_response();
    };
    if registry.revoke(capability, &origin) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::FORBIDDEN,
            Json(json!({"message":"browser resume denied"})),
        )
            .into_response()
    }
}

fn valid_browser_origin(headers: &HeaderMap) -> Option<String> {
    let origin = headers.get(header::ORIGIN)?.to_str().ok()?;
    if origin.len() > 128 || !origin.starts_with("http://127.0.0.1:") {
        return None;
    }
    let port = origin.strip_prefix("http://127.0.0.1:")?;
    if port.is_empty()
        || port.len() > 5
        || !port.bytes().all(|b| b.is_ascii_digit())
        || (port.len() > 1 && port.starts_with("0"))
    {
        return None;
    }
    (port.parse::<u16>().ok()? != 0).then(|| origin.to_owned())
}

fn resume_authorization(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("BrowserResume ")
}

#[derive(Deserialize)]
struct ProviderAuthBody {
    #[serde(rename = "type")]
    kind: String,
    key: String,
    metadata: Option<std::collections::BTreeMap<String, String>>,
}

async fn save_provider_auth(
    axum::extract::Path(provider): axum::extract::Path<String>,
    Json(body): Json<ProviderAuthBody>,
) -> Result<Json<Value>, ApiFailure> {
    if body.kind != "api"
        || provider.len() > 128
        || body.key.len() > 16 * 1024
        || body.key.trim().is_empty()
    {
        return Err(ApiFailure::bad_request("invalid provider credential"));
    }
    opencode_rk_providers::persisted_auth::save_api_key(provider, body.key, body.metadata)
        .await
        .map_err(ApiFailure::internal)?;
    Ok(Json(json!(true)))
}

#[derive(Deserialize)]
struct CustomProviderBody { name: String, base_url: String, api_key: Option<String> }

async fn list_custom_providers() -> Result<Json<Value>, ApiFailure> {
    let providers = custom_provider::list().await.map_err(ApiFailure::internal)?;
    Ok(Json(json!({"providers": providers.into_iter().map(|p| json!({"id":p.id,"name":p.name,"base_url":p.base_url})).collect::<Vec<_>>() })))
}

async fn save_custom_provider(
    Path(id): Path<String>, Json(body): Json<CustomProviderBody>,
) -> Result<Json<Value>, ApiFailure> {
    let config = CustomProviderConfig { id, name: body.name, base_url: body.base_url, api_key: body.api_key.filter(|key| !key.trim().is_empty()) };
    let api_key = config.api_key.clone();
    let provider_id = config.id.clone();
    custom_provider::save(config).await.map_err(|e| ApiFailure::bad_request(e.to_string()))?;
    if let Some(key) = api_key {
        opencode_rk_providers::persisted_auth::save_api_key(provider_id, key, None)
            .await.map_err(ApiFailure::bad_request)?;
    }
    Ok(Json(json!(true)))
}

async fn responses_client(provider_id: &str) -> Result<OpenAiResponsesClient, ResponsesError> {
    if provider_id == "openai" { return OpenAiResponsesClient::from_persisted_env().await; }
    let config = custom_provider::load(provider_id).await.map_err(|e| ResponsesError::InvalidConfig(e.to_string()))?
        .ok_or_else(|| ResponsesError::InvalidConfig("provider not found".into()))?;
    let provider_config = opencode_rk_providers::config::ProviderConfig {
        provider_id: config.id.clone(), base_url: config.base_url, api_key_env: format!("{}_API_KEY", config.id.to_uppercase()),
        timeout_secs: 60, max_tokens: 4096, temperature: 0.7,
    };
    OpenAiResponsesClient::from_config(provider_config, config.api_key)
}

async fn discover_custom_provider_models(Path(id): Path<String>) -> Result<Json<Value>, ApiFailure> {
    let config = custom_provider::load(&id).await.map_err(ApiFailure::internal)?.ok_or_else(|| ApiFailure::not_found("provider not found"))?;
    let models = custom_provider::discover_models(&config).await.map_err(|error| ApiFailure::bad_gateway(error.to_string()))?;
    Ok(Json(json!({"models": models.into_iter().map(|(model,name)| json!({"id":model,"name":name,"provider_id":id})).collect::<Vec<_>>() })))
}
async fn interrupt_session(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiFailure> {
    let id = parse_session_id(&id)?;
    state.sessions.get(id).await.map_err(|error| {
        if error.to_string().contains("session not found") {
            ApiFailure::not_found(error.to_string())
        } else {
            ApiFailure::internal(error)
        }
    })?;
    run_owner::cancel_and_wait(id).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_session_permissions(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    state.sessions.get(id).await.map_err(|error| {
        if error.to_string().contains("session not found") {
            ApiFailure::not_found(error.to_string())
        } else {
            ApiFailure::internal(error)
        }
    })?;
    let data = run_owner::active(id)
        .and_then(|owner| owner.pending())
        .into_iter()
        .collect::<Vec<_>>();
    Ok(Json(json!({"data":data})))
}

#[derive(Deserialize)]
struct PermissionReplyBody {
    reply: String,
    message: Option<String>,
}

async fn reply_session_permission(
    Path((id, request_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(body): Json<PermissionReplyBody>,
) -> Result<StatusCode, ApiFailure> {
    let id = parse_session_id(&id)?;
    state.sessions.get(id).await.map_err(|error| {
        if error.to_string().contains("session not found") {
            ApiFailure::not_found(error.to_string())
        } else {
            ApiFailure::internal(error)
        }
    })?;
    let Some(owner) = run_owner::active(id) else {
        return Err(ApiFailure::not_found("permission request not found"));
    };
    let Some(pending) = owner.pending() else {
        return Err(ApiFailure::not_found("permission request not found"));
    };
    if pending.id != request_id || pending.session_id != id.to_string() {
        return Err(ApiFailure::not_found("permission request not found"));
    }
    match body.reply.as_str() {
        "once" => {
            if !owner.reply_once(&request_id) {
                return Err(ApiFailure::not_found("permission request not found"));
            }
        }
        "always" => {
            if !owner.reply_always(&request_id, pending.action, pending.resources[0].clone()) {
                return Err(ApiFailure::not_found("permission request not found"));
            }
        }
        "reject" => {
            let _ = body.message;
            if !owner.reply_reject(&request_id) {
                return Err(ApiFailure::not_found("permission request not found"));
            }
        }
        _ => {
            return Err(ApiFailure::bad_request(
                "reply must be once, always, or reject",
            ))
        }
    }
    Ok(StatusCode::NO_CONTENT)
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
            let turn_enabled = turn_tool_config().contains(&tool.id);
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
    Extension(events): Extension<event_bus::EventBus>,
    Json(body): Json<CreateSessionBody>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let session = state
        .sessions
        .create(body.title)
        .await
        .map_err(ApiFailure::internal)?;
    // Publish only after the durable create succeeded; the bus allocates the
    // shared publication identity (all SSE subscribers observe the same id).
    let _ = events.publish(event_bus::ServerEvent::SessionCreated(session.id));
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
    let before = params.before.as_deref().map(parse_message_id).transpose()?;
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
    Extension(events): Extension<event_bus::EventBus>,
    Json(body): Json<AppendMessageBody>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let id = parse_session_id(&id)?;
    let message = state
        .sessions
        .append_text(id, MessageRole::User, body.text)
        .await
        .map_err(ApiFailure::internal)?;
    // Publish ONLY after the durable append succeeded: the payload carries the
    // actual persisted `MessageRecord.id` (UUID), so consumers can fetch
    // history by identity. A failed append publishes nothing.
    let _ = events.publish(event_bus::ServerEvent::MessageStored {
        session: id,
        message_id: message.id,
    });
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
        return Err(ApiFailure::unprocessable(
            "attachment exceeds the supported size bound",
        ));
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
    Ok((StatusCode::CREATED, Json(json!({"attachment":attachment}))))
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
    #[serde(default)]
    reasoning_effort: String,
}

fn turn_admission_failure(error: run_owner::AdmissionError) -> ApiFailure {
    match error {
        run_owner::AdmissionError::Capacity | run_owner::AdmissionError::SessionActive => {
            ApiFailure::too_many_requests(match error {
                run_owner::AdmissionError::SessionActive => "session already has an active turn",
                run_owner::AdmissionError::Capacity => "too many active turns",
                run_owner::AdmissionError::RegistryUnavailable => {
                    "turn ownership registry unavailable"
                }
            })
        }
        run_owner::AdmissionError::RegistryUnavailable => {
            ApiFailure::internal("turn ownership registry unavailable")
        }
    }
}

async fn create_turn(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(events): Extension<event_bus::EventBus>,
    Json(body): Json<CreateTurnBody>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let id = parse_session_id(&id)?;
    let permit = TurnExecutionPermit::acquire(id).map_err(turn_admission_failure)?;
    let owner = permit.owner();
    state.sessions.get(id).await.map_err(ApiFailure::internal)?;

    if body.text.trim().is_empty() {
        return Err(ApiFailure::bad_request("turn text must not be empty"));
    }
    let (provider_id, model_id) = body
        .model
        .split_once('/')
        .filter(|(provider, model)| !provider.is_empty() && !model.is_empty())
        .ok_or_else(|| ApiFailure::bad_request("model must use provider/model format"))?;
    if !body.reasoning_effort.is_empty()
        && !matches!(
            body.reasoning_effort.as_str(),
            "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
        )
    {
        return Err(ApiFailure::bad_request("unsupported reasoning effort"));
    }

    let provider = responses_client(provider_id)
        .await
        .map_err(provider_failure)?;
    let user_message = state
        .sessions
        .append_text(id, MessageRole::User, body.text)
        .await
        .map_err(ApiFailure::internal)?;
    // Durable user append -> publish persisted identity (shared id allocated
    // once by the bus, actual MessageRecord.id in the payload).
    let _ = events.publish(event_bus::ServerEvent::MessageStored {
        session: id,
        message_id: user_message.id,
    });
    let input = bounded_typed_history(
        &state.sessions,
        id,
        MAX_RESPONSES_INPUT_MESSAGES,
        MAX_RESPONSES_INPUT_BYTES,
    )
    .await
    .map_err(ApiFailure::internal)?;
    let mut cancelled = owner.cancellation_receiver();
    let assistant_text = tokio::select! {
        biased;
        _ = cancelled.changed() => return Err(ApiFailure::bad_gateway("turn cancelled")),
        result = provider.create_with_items(model_id, &body.reasoning_effort, &input) => {
            result.map_err(provider_failure)?
        },
    };
    if owner.is_cancelled() {
        return Err(ApiFailure::bad_gateway("turn cancelled"));
    }
    let assistant_message = state
        .sessions
        .append_text(id, MessageRole::Assistant, assistant_text)
        .await
        .map_err(ApiFailure::internal)?;
    let _ = events.publish(event_bus::ServerEvent::MessageStored {
        session: id,
        message_id: assistant_message.id,
    });

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "user_message": user_message,
            "assistant_message": assistant_message
        })),
    ))
}

struct TurnStreamState {
    client: OpenAiResponsesClient,
    provider: OpenAiResponsesStream,
    sessions: SessionService,
    session_id: SessionId,
    user_message: Option<MessageRecord>,
    /// User message ID preserved after user_message.take() for round linkage.
    turn_message_id: MessageId,
    assistant_text: String,
    reasoning_summary: String,
    stage: TurnStreamStage,
    _permit: TurnExecutionPermit,
    owner: Arc<RunOwner>,
    /// Router-owned event bus for durable append publication.
    events: event_bus::EventBus,
    /// Agentic loop state: provider tool schema + step budget + typed history.
    tools: Vec<ResponsesTool>,
    enabled_tools: Vec<String>,
    /// Permission broker consulted before every tool execution.
    broker: PermissionBroker,
    /// File capability rooted at the turn's captured project directory.
    file_tool: FileTool,
    history_items: Vec<ResponsesItem>,
    loop_control: LoopController,
    pending_calls: Vec<RequestedCall>,
    executed_outputs: Vec<CallOutputItem>,
    emit_cursor: usize,
    model: String,
    reasoning_effort: String,
    /// Set when the loop must finalize with this stop reason (step cap hit).
    forced_stop: Option<TurnStop>,
    /// A human rejection/cancellation settles the admitted tool output but
    /// must not start another provider round.
    stop_after_outputs: bool,
    /// Agents-crate executor: mirror of the real agent loop plan (CONVERGENCE AGENTS).
    _agent_plan: AgentExecutor,
    /// Ordinal of the current tool round within this turn.
    round_ordinal: u32,
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

const MAX_FILE_WRITE_ARGUMENT_BYTES: usize = 1024 * 1024;
const MAX_PENDING_TURN_CALLS: usize = (MAX_RESPONSES_INPUT_MESSAGES - 1) / 2;

/// Parse a provider write request without touching the filesystem.
fn file_write_operation(arguments: &str) -> Result<FileOperation, String> {
    if arguments.len() > MAX_FILE_WRITE_ARGUMENT_BYTES {
        return Err("write arguments exceed size limit".to_owned());
    }
    let parsed: Value = serde_json::from_str(arguments)
        .map_err(|_| "write arguments are not valid JSON".to_owned())?;
    let path = parsed
        .get("path")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| "write arguments require a non-empty string path".to_owned())?;
    let content = parsed
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| "write arguments require string content".to_owned())?;
    let append = match parsed.get("append") {
        None => false,
        Some(value) => value
            .as_bool()
            .ok_or_else(|| "write arguments require a boolean append flag".to_owned())?,
    };
    if path.len() > 4096 || content.len() > MAX_FILE_WRITE_ARGUMENT_BYTES {
        return Err("write path or content exceeds size limit".to_owned());
    }
    Ok(FileOperation::Write {
        path: PathBuf::from(path),
        content: content.to_owned(),
        append,
    })
}

/// Dispatch the builtin write only through the file capability broker.
///
/// The public/provider result deliberately contains no path or file payload.
struct WriteOutcome {
    output: String,
    stop_after_outputs: bool,
}

async fn execute_write(
    arguments: &str,
    broker: &PermissionBroker,
    file_tool: &FileTool,
    owner: &Arc<RunOwner>,
    message_id: MessageId,
    call_id: &str,
) -> WriteOutcome {
    let outcome = |output: String, stop_after_outputs: bool| WriteOutcome {
        output,
        stop_after_outputs,
    };
    let operation = match file_write_operation(arguments) {
        Ok(operation) => operation,
        Err(error) => return outcome(format!("error: {error}"), false),
    };
    let FileOperation::Write { ref path, .. } = operation else {
        return outcome("write denied".to_owned(), false);
    };
    let resource = file_tool
        .project_root_path()
        .filter(|_| !path.is_absolute())
        .map(|root| root.join(path))
        .unwrap_or_else(|| path.clone())
        .to_string_lossy()
        .into_owned();
    let intent = OperationIntent::File {
        action: opencode_rk_security::FileAction::Write,
        path: PathBuf::from(&resource),
    };
    let decision = broker.authorize(&intent);
    let approved = match decision {
        Decision::Allow => None,
        Decision::Deny { .. } => return outcome("write denied".to_owned(), false),
        Decision::RequireHuman { .. }
            if run_owner::has_always_grant(
                owner.session_id,
                "write",
                &resource,
                &broker.policy_context(),
            ) =>
        {
            match broker.approve_human(
                &intent,
                owner.session_id.to_string(),
                owner.run_id,
                call_id.to_owned(),
            ) {
                Some(capability) => Some(capability),
                None => return outcome("write requires human approval".to_owned(), false),
            }
        }
        Decision::RequireHuman { .. } => {
            let request = PermissionRequest {
                id: format!("per_{}", opencode_rk_contracts::ApprovalId::new()),
                session_id: owner.session_id.to_string(),
                action: "write".to_owned(),
                resources: vec![resource.clone()],
                source: PermissionSource {
                    kind: "tool",
                    message_id: message_id.to_string(),
                    call_id: call_id.to_owned(),
                },
            };
            let request_id = request.id.clone();
            let receiver = match owner.install_pending(request, intent.clone(), broker) {
                Ok(receiver) => receiver,
                Err(_) => return outcome("write requires human approval".to_owned(), false),
            };
            let _pending_guard = PendingPermissionGuard::new(Arc::clone(owner), request_id);
            let mut cancelled = owner.cancellation_receiver();
            let approved = tokio::select! {
                biased;
                _ = cancelled.changed() => ApprovedPermission { reply: PermissionReply::Cancelled, capability: None },
                result = receiver => result.unwrap_or(ApprovedPermission { reply: PermissionReply::Cancelled, capability: None }),
            };
            match approved.reply {
                PermissionReply::Once | PermissionReply::Always => approved.capability,
                PermissionReply::Reject => return outcome("write rejected".to_owned(), true),
                PermissionReply::Cancelled => return outcome("write cancelled".to_owned(), true),
            }
        }
    };
    let Some(_effect_guard) = owner.begin_effect() else {
        return outcome("write cancelled".to_owned(), true);
    };
    let result = match approved {
        Some(approval) => file_tool.execute_human_approved(
            operation,
            broker,
            approval,
            &owner.session_id.to_string(),
            owner.run_id,
            call_id,
        ),
        None => file_tool.execute_authorized(operation, broker),
    };
    match result {
        Ok(result) if result.success => outcome("write success".to_owned(), false),
        Ok(result) => {
            let error = result.error.unwrap_or_default();
            if error.contains("requires human approval") {
                outcome("write requires human approval".to_owned(), false)
            } else if error.contains("denied") {
                outcome("write denied".to_owned(), false)
            } else {
                outcome("write failed".to_owned(), false)
            }
        }
        Err(_) => outcome("write failed".to_owned(), false),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TurnStreamStage {
    User,
    Provider,
    /// Dispatch accumulated [`TurnStreamState::pending_calls`] now.
    Executing,
    /// Emit one `tool_output` NDJSON event per executed call.
    EmitOutputs,
    /// Reload durable outputs and start the next provider round (or finalize).
    NextRound,
    Done,
}

async fn create_turn_stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(events): Extension<event_bus::EventBus>,
    Json(body): Json<CreateTurnBody>,
) -> Result<Response, ApiFailure> {
    let id = parse_session_id(&id)?;
    let permit = TurnExecutionPermit::acquire(id).map_err(turn_admission_failure)?;
    let owner = permit.owner();
    let project_root = std::env::current_dir()
        .map_err(|_| ApiFailure::internal("could not determine project root"))?;
    let file_tool = FileTool::with_project_root(project_root.clone());
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(project_root));
    state.sessions.get(id).await.map_err(ApiFailure::internal)?;

    if body.text.trim().is_empty() {
        return Err(ApiFailure::bad_request("turn text must not be empty"));
    }
    let (provider_id, model_id) = body
        .model
        .split_once('/')
        .filter(|(provider, model)| !provider.is_empty() && !model.is_empty())
        .ok_or_else(|| ApiFailure::bad_request("model must use provider/model format"))?;
    if !body.reasoning_effort.is_empty()
        && !matches!(
            body.reasoning_effort.as_str(),
            "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
        )
    {
        return Err(ApiFailure::bad_request("unsupported reasoning effort"));
    }

    let client = responses_client(provider_id)
        .await
        .map_err(provider_failure)?;
    let user_message = state
        .sessions
        .append_text(id, MessageRole::User, body.text)
        .await
        .map_err(ApiFailure::internal)?;
    let _ = events.publish(event_bus::ServerEvent::MessageStored {
        session: id,
        message_id: user_message.id,
    });
    let user_message_id = user_message.id;
    let input = bounded_typed_history(
        &state.sessions,
        id,
        MAX_RESPONSES_INPUT_MESSAGES,
        MAX_RESPONSES_INPUT_BYTES,
    )
    .await
    .map_err(ApiFailure::internal)?;

    // Agentic loop setup: advertise tools only when turn-tool execution is
    // explicitly enabled (OPENCODE_RK_TURN_TOOLS). The list is deny-by-default:
    // an advertised tool is executable, an unadvertised one is not.
    let turn_tools = turn_tool_config();
    let registry = ToolRegistry::new();
    let tools: Vec<ResponsesTool> = registry
        .list()
        .into_iter()
        .filter(|tool| turn_tools.contains(&tool.id))
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
    let history_items = input;

    let provider = client
        .clone()
        .stream_with_tools(model_id, &body.reasoning_effort, &history_items, &tools)
        .await
        .map_err(provider_failure)?;

    let stream = stream::unfold(
        TurnStreamState {
            client,
            provider,
            sessions: state.sessions,
            session_id: id,
            user_message: Some(user_message),
            turn_message_id: user_message_id,
            assistant_text: String::new(),
            reasoning_summary: String::new(),
            stage: TurnStreamStage::User,
            _permit: permit,
            owner,
            events,
            tools,
            enabled_tools: turn_tools,
            broker,
            file_tool,
            history_items,
            loop_control: LoopController::with_cap(max_steps),
            pending_calls: Vec::new(),
            executed_outputs: Vec::new(),
            emit_cursor: 0,
            model: model_id.to_owned(),
            reasoning_effort: body.reasoning_effort.clone(),
            forced_stop: None,
            stop_after_outputs: false,
            _agent_plan: AgentExecutor::new(vec![
                opencode_rk_agents::agent_executor::LoopStep::ProviderCall,
                opencode_rk_agents::agent_executor::LoopStep::ToolDispatch,
                opencode_rk_agents::agent_executor::LoopStep::PolicyCheck,
                opencode_rk_agents::agent_executor::LoopStep::Settle,
            ])
            .expect("fixed 4-step turn plan fits agent executor capacity"),
            round_ordinal: 0,
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
                        if state.owner.is_cancelled() {
                            return None;
                        }
                        let mut cancelled = state.owner.cancellation_receiver();
                        let event = tokio::select! {
                            biased;
                            _ = cancelled.changed() => {
                                return None;
                            }
                            event = state.provider.next_event() => event,
                        };
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
                                state.reasoning_summary.push_str(&delta);
                                return Some((
                                    Ok::<Bytes, Infallible>(ndjson(json!({
                                        "type": "reasoning_summary_delta",
                                        "delta": delta,
                                    }))),
                                    state,
                                ));
                            }
                            Ok(Some(ResponsesStreamEvent::FunctionCall {
                                call_id,
                                name,
                                arguments,
                            })) => {
                                if state.pending_calls.len() >= MAX_PENDING_TURN_CALLS {
                                    state.stage = TurnStreamStage::Done;
                                    return Some((
                                        Ok::<Bytes, Infallible>(stream_error(
                                            "bad_gateway",
                                            "provider tool batch exceeds the replay bound",
                                        )),
                                        state,
                                    ));
                                }
                                state.pending_calls.push(RequestedCall {
                                    call_id: call_id.clone(),
                                    name: name.clone(),
                                    arguments: arguments.clone(),
                                });
                                return Some((
                                    Ok::<Bytes, Infallible>(ndjson(json!({
                                        "type": "tool_call",
                                        "call_id": call_id,
                                        "name": name,
                                        "arguments": if name == "write" { "{}" } else { arguments.as_str() },
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
                                let reasoning_summary =
                                    std::mem::take(&mut state.reasoning_summary);
                                let stop_reason = stop.as_str();
                                let persisted_summary =
                                    (!reasoning_summary.is_empty()).then_some(reasoning_summary);
                                match state
                                    .sessions
                                    .append_assistant_with_reasoning(
                                        state.session_id,
                                        assistant_text,
                                        persisted_summary.clone(),
                                    )
                                    .await
                                {
                                    Ok(message) => {
                                        let _ = state.events.publish(
                                            event_bus::ServerEvent::MessageStored {
                                                session: state.session_id,
                                                message_id: message.id,
                                            },
                                        );
                                        state.stage = TurnStreamStage::Done;
                                        return Some((
                                            Ok::<Bytes, Infallible>(ndjson(json!({
                                                "type": "assistant_message",
                                                "message": message,
                                                "reasoning_summary": persisted_summary,
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
                    }
                    TurnStreamStage::Executing => {
                        // One round = one provider stream + at most one
                        // tool dispatch batch. The budget was checked at
                        // the completed event; consume it here.
                        let _ = state.loop_control.begin_round();
                        let executor = ToolExecutor::new();
                        let batch = std::mem::take(&mut state.pending_calls);
                        let admissions: Result<Vec<_>, SessionError> = batch
                            .iter()
                            .map(|call| {
                                let body = PayloadRef::inline(format!("[{}] pending", call.name))
                                    .map_err(|error| {
                                    SessionError::Contract(error.to_string())
                                })?;
                                Ok(ToolCallAdmission {
                                    call_id: call.call_id.clone(),
                                    name: call.name.clone(),
                                    arguments: call.arguments.clone(),
                                    message: MessageRecord {
                                        id: MessageId::new(),
                                        session_id: state.session_id,
                                        role: MessageRole::Tool,
                                        body,
                                        created_at: Timestamp::now(),
                                    },
                                })
                            })
                            .collect();
                        let admissions = match admissions {
                            Ok(calls) => calls,
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
                        };
                        let message_ids: Vec<_> =
                            admissions.iter().map(|call| call.message.id).collect();
                        let round = ToolRound {
                            round_id: format!(
                                "round-{}-{}",
                                state.turn_message_id, state.round_ordinal
                            ),
                            session_id: state.session_id,
                            turn_message_id: state.turn_message_id,
                            round_ordinal: state.round_ordinal,
                            // The pending-call byte/count gate bounds this conversion.
                            expected_pairs: batch.len() as u32,
                        };
                        // Admission is one transaction for ALL calls, including
                        // denied/overflow outputs. No tool runs before it commits.
                        if let Err(error) = state
                            .sessions
                            .admit_tool_round(round.clone(), admissions)
                            .await
                        {
                            state.stage = TurnStreamStage::Done;
                            return Some((
                                Ok::<Bytes, Infallible>(stream_error(
                                    "internal_error",
                                    format!("failed to admit tool round: {error}"),
                                )),
                                state,
                            ));
                        }
                        state.round_ordinal = state.round_ordinal.saturating_add(1);
                        let mut round_outputs: Vec<CallOutputItem> = Vec::new();
                        for (index, (call, message_id)) in
                            batch.into_iter().zip(message_ids).enumerate()
                        {
                            let permitted = state.enabled_tools.contains(&call.name);
                            let mut stop_after_output = false;
                            let raw_output = if state.owner.is_cancelled() {
                                "error: call not executed because turn was cancelled".to_owned()
                            } else if state.stop_after_outputs {
                                "error: call not executed after an earlier permission rejection"
                                    .to_owned()
                            } else if index >= MAX_CALLS_PER_ROUND {
                                format!(
                                    "error: step budget per round exceeded (max {MAX_CALLS_PER_ROUND} calls); call not executed"
                                )
                            } else if permitted {
                                if call.name == "write" {
                                    let outcome = execute_write(
                                        &call.arguments,
                                        &state.broker,
                                        &state.file_tool,
                                        &state.owner,
                                        state.turn_message_id,
                                        &call.call_id,
                                    )
                                    .await;
                                    stop_after_output = outcome.stop_after_outputs;
                                    outcome.output
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
                                                .execute(
                                                    opencode_rk_tools::executor::ToolCall::new(
                                                        call.call_id.clone(),
                                                        call.name.clone(),
                                                        arguments,
                                                    ),
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
                            // Persist the real output before exposing its event:
                            // a disconnected reader cannot discard a completed pair.
                            if let Err(error) = state
                                .sessions
                                .persist_tool_output(
                                    round.clone(),
                                    index as u64,
                                    bounded.clone(),
                                    message_id,
                                )
                                .await
                            {
                                state.stage = TurnStreamStage::Done;
                                return Some((
                                    Ok::<Bytes, Infallible>(stream_error(
                                        "internal_error",
                                        format!("failed to persist tool output: {error}"),
                                    )),
                                    state,
                                ));
                            }
                            if !state.owner.is_cancelled() && !state.stop_after_outputs {
                                round_outputs.push(CallOutputItem {
                                    call_id: call.call_id.clone(),
                                    name: call.name,
                                    output: bounded,
                                });
                            }
                            if stop_after_output {
                                state.stop_after_outputs = true;
                            }
                        }
                        if state.owner.is_cancelled() {
                            return None;
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
                        if state.stop_after_outputs {
                            state.stop_after_outputs = false;
                            state.forced_stop = Some(TurnStop::Completed);
                        }
                        state.stage = TurnStreamStage::NextRound;
                        continue;
                    }
                    TurnStreamStage::NextRound => {
                        state.executed_outputs.clear();
                        if let Some(stop) = state.forced_stop.take() {
                            let assistant_text = std::mem::take(&mut state.assistant_text);
                            let reasoning_summary = std::mem::take(&mut state.reasoning_summary);
                            let persisted_summary =
                                (!reasoning_summary.is_empty()).then_some(reasoning_summary);
                            match state
                                .sessions
                                .append_assistant_with_reasoning(
                                    state.session_id,
                                    assistant_text,
                                    persisted_summary.clone(),
                                )
                                .await
                            {
                                Ok(message) => {
                                    state.stage = TurnStreamStage::Done;
                                    return Some((
                                        Ok::<Bytes, Infallible>(ndjson(json!({
                                            "type": "assistant_message",
                                            "message": message,
                                            "reasoning_summary": persisted_summary,
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
                        // Next provider round: reload typed history from storage
                        // to ensure accurate replay across restarts.
                        let history_items = match bounded_typed_history(
                            &state.sessions,
                            state.session_id,
                            MAX_RESPONSES_INPUT_MESSAGES,
                            MAX_RESPONSES_INPUT_BYTES,
                        )
                        .await
                        {
                            Ok(items) => items,
                            Err(error) => {
                                state.stage = TurnStreamStage::Done;
                                return Some((
                                    Ok::<Bytes, Infallible>(stream_error(
                                        "internal_error",
                                        format!("failed to reload typed history: {error}"),
                                    )),
                                    state,
                                ));
                            }
                        };
                        state.history_items = history_items;
                        match state
                            .client
                            .stream_with_tools(
                                &state.model,
                                &state.reasoning_effort,
                                &state.history_items,
                                &state.tools,
                            )
                            .await
                        {
                            Ok(next_stream) => {
                                if state.owner.is_cancelled() {
                                    return None;
                                }
                                state.provider = next_stream;
                                state.stage = TurnStreamStage::Provider;
                                continue;
                            }
                            Err(error) => {
                                state.stage = TurnStreamStage::Done;
                                let failure = provider_failure(error);
                                return Some((
                                    Ok::<Bytes, Infallible>(stream_error(
                                        failure.code,
                                        failure.message,
                                    )),
                                    state,
                                ));
                            }
                        }
                    }
                    TurnStreamStage::Done => return None,
                }
            }
        },
    );

    // Keep execution/settlement owned by a bounded producer rather than by the
    // HTTP response body. A disconnected client stops receiving events but
    // cannot cancel a tool output persistence future midway through a typed
    // admission batch. The task exits only after the stream has durably settled
    // its admitted calls and dropped the turn permit.
    let (event_tx, event_rx) = tokio::sync::mpsc::channel::<Result<Bytes, Infallible>>(16);
    tokio::spawn(async move {
        let mut stream = Box::pin(stream);
        while let Some(event) = stream.next().await {
            if event_tx.send(event).await.is_err() {
                while stream.next().await.is_some() {}
                break;
            }
        }
    });
    let response_stream = stream::unfold(event_rx, |mut receiver| async move {
        receiver.recv().await.map(|event| (event, receiver))
    });

    Ok((
        StatusCode::CREATED,
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(response_stream),
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

/// Convert a `HistoryItem` from typed storage into a `ResponsesItem` for
/// provider requests. Reconstructs function_call and function_call_output
/// items from typed tool records, and plain text items from message rows.
fn history_item_to_responses_item(item: HistoryItem) -> Result<ResponsesItem, SessionError> {
    match item {
        HistoryItem::Message(message) => {
            let role = match message.role {
                MessageRole::System => ResponsesRole::System,
                MessageRole::User => ResponsesRole::User,
                MessageRole::Assistant => ResponsesRole::Assistant,
                MessageRole::Tool => {
                    return Err(SessionError::Contract("unlinked tool history".to_owned()));
                }
            };
            let PayloadRef::Inline { text } = &message.body else {
                return Err(SessionError::Contract("unresolved blob history".to_owned()));
            };
            Ok(ResponsesItem::Text {
                role,
                content: text.clone(),
            })
        }
        HistoryItem::Tool(typed) => {
            if typed.kind == "call" {
                Ok(ResponsesItem::FunctionCall {
                    call_id: typed.call_id,
                    name: typed.name,
                    arguments: typed.payload,
                })
            } else if typed.kind == "output" {
                Ok(ResponsesItem::FunctionCallOutput {
                    call_id: typed.call_id,
                    output: typed.payload,
                })
            } else {
                Err(SessionError::Contract(
                    "invalid typed tool record kind".to_owned(),
                ))
            }
        }
    }
}

/// Load bounded typed history from storage for provider continuation/restart.
/// Returns `ResponsesItem` entries reconstructed from both regular messages
/// and typed tool records, ordered by insertion order.
async fn bounded_typed_history(
    sessions: &SessionService,
    session_id: SessionId,
    max_items: usize,
    max_bytes: usize,
) -> Result<Vec<ResponsesItem>, SessionError> {
    let items = sessions
        .bounded_history(session_id, max_items, max_bytes)
        .await?;
    items
        .into_iter()
        .map(history_item_to_responses_item)
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
                opencode_rk_security::PermissionRule::new(
                    "blocked-tool",
                    opencode_rk_security::RuleEffect::Deny,
                ),
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
