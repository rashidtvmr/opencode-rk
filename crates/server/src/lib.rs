//! Small HTTP boundary over the native catalog and session services.
#![forbid(unsafe_code)]
pub mod acp_bridge;
pub mod acp_files;
pub mod acp_session;
pub mod admission_bounds;

pub mod builtin_agents;
pub mod agent_loop;
pub mod app_client;
pub use agent_loop::{
    CallOutput, LoopController, MAX_CALLS_PER_ROUND, MAX_TOOL_OUTPUT_CHARS, MAX_TURN_STEPS,
    RequestedCall, TurnStop, function_call_output as loop_function_call_output,
    truncate_tool_output,
};
pub mod app_protocols;
pub mod app_runtime;
pub mod auto_loop;
pub mod auto_report;
pub mod auto_window;
mod browser_resume;
pub mod chat_composer;
pub mod clients;
pub mod configured_commands;
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
pub mod permission_rules;
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
mod task_children;
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
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Extension, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use futures_util::{StreamExt, stream};
use opencode_rk_agents::agent_executor::AgentExecutor;
use opencode_rk_catalog::{Catalog, CatalogQuery};
use opencode_rk_contracts::{
    ArtifactId, ArtifactKind, AttachmentId, MAX_DRAFT_ATTACHMENT_BYTES, MessageId, MessageRecord,
    MessageRole, PayloadRef, SessionId, Timestamp, WIRE_SCHEMA_VERSION,
};
use opencode_rk_providers::custom_provider::{self, CustomProviderConfig};
use opencode_rk_providers::responses::{
    MAX_RESPONSES_INPUT_BYTES, MAX_RESPONSES_INPUT_MESSAGES, OpenAiResponsesClient,
    OpenAiResponsesStream, ResponsesError, ResponsesItem, ResponsesRole, ResponsesStopReason,
    ResponsesStreamEvent, ResponsesTool,
};
use opencode_rk_security::{Decision, OperationIntent, PermissionBroker, SecurityPolicy};
use opencode_rk_sessions::{
    HistoryItem, SessionError, SessionService, ToolCallAdmission, ToolRound,
};
use opencode_rk_tools::executor::ToolExecutor;
use opencode_rk_tools::file_ops::{FileOperation, FileTool};
use opencode_rk_tools::registry::ToolRegistry;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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
    let children = task_children::Children::default();
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
        .route("/api/commands", get(list_configured_commands))
        .route("/api/permission-rules",get(list_permission_rules).post(save_permission_rule))
        .route("/api/models/{provider}/{model}", get(get_model))
        .route("/api/providers", get(list_custom_providers))
        .route("/api/providers/{id}", post(save_custom_provider))
        .route(
            "/api/providers/{id}/models",
            get(discover_custom_provider_models),
        )
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
        .route("/api/sessions/{id}/command", post(run_configured_command))
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
        .route("/api/sessions/{id}/children", get(task_children::list))
        .route("/api/sessions/{id}/children/{child}/stop", post(task_children::stop))
        .route("/api/sessions/{id}/children/{child}/retry", post(task_children::retry))
        .route("/api/sessions/{id}/children/{child}/continue", post(task_children::followup))
        .route("/api/sessions/{id}/shell/stream", post(native_shell_stream))
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
        .layer(axum::extract::Extension(children))
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
struct CustomProviderBody {
    name: String,
    base_url: String,
    api_key: Option<String>,
    buffered_model_ids: Option<Vec<String>>,
}

async fn list_custom_providers() -> Result<Json<Value>, ApiFailure> {
    let providers = custom_provider::list()
        .await
        .map_err(ApiFailure::internal)?;
    Ok(Json(
        json!({"providers": providers.into_iter().map(|p| json!({"id":p.id,"name":p.name,"base_url":p.base_url,"buffered_model_ids":p.buffered_model_ids})).collect::<Vec<_>>() }),
    ))
}

async fn save_custom_provider(
    Path(id): Path<String>,
    Json(body): Json<CustomProviderBody>,
) -> Result<Json<Value>, ApiFailure> {
    let preserve_buffered = body.buffered_model_ids.is_none();
    let buffered_model_ids = body.buffered_model_ids.unwrap_or_default();
    let config = CustomProviderConfig {
        id,
        name: body.name,
        base_url: body.base_url,
        api_key: body.api_key.filter(|key| !key.trim().is_empty()),
        buffered_model_ids,
    };
    let api_key = config.api_key.clone();
    let provider_id = config.id.clone();
    if preserve_buffered {
        custom_provider::save_preserving_buffered_models(config).await
    } else {
        custom_provider::save(config).await
    }
    .map_err(|e| ApiFailure::bad_request(e.to_string()))?;
    if let Some(key) = api_key {
        opencode_rk_providers::persisted_auth::save_api_key(provider_id, key, None)
            .await
            .map_err(ApiFailure::bad_request)?;
    }
    Ok(Json(json!(true)))
}

async fn responses_client(provider_id: &str) -> Result<OpenAiResponsesClient, ResponsesError> {
    if provider_id == "openai" {
        return OpenAiResponsesClient::from_persisted_env().await;
    }
    let config = custom_provider::load(provider_id)
        .await
        .map_err(|e| ResponsesError::InvalidConfig(e.to_string()))?
        .ok_or_else(|| ResponsesError::InvalidConfig("provider not found".into()))?;
    let provider_config = opencode_rk_providers::config::ProviderConfig {
        provider_id: config.id.clone(),
        base_url: config.base_url,
        api_key_env: format!("{}_API_KEY", config.id.to_uppercase()),
        timeout_secs: 60,
        max_tokens: 4096,
        temperature: 0.7,
    };
    OpenAiResponsesClient::from_config(provider_config, config.api_key)
        .map(|client| client.with_buffered_models(config.buffered_model_ids))
}

async fn discover_custom_provider_models(
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let config = custom_provider::load(&id)
        .await
        .map_err(ApiFailure::internal)?
        .ok_or_else(|| ApiFailure::not_found("provider not found"))?;
    let models = custom_provider::discover_models_detailed(&config)
        .await
        .map_err(|error| ApiFailure::bad_gateway(error.to_string()))?;
    Ok(Json(
        json!({"models": models.into_iter().map(|model| json!({"id":model.id,"name":model.name,"provider_id":id,"context_window":model.context_window})).collect::<Vec<_>>() }),
    ))
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
    Extension(children): Extension<task_children::Children>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    state.sessions.get(id).await.map_err(|error| {
        if error.to_string().contains("session not found") {
            ApiFailure::not_found(error.to_string())
        } else {
            ApiFailure::internal(error)
        }
    })?;
    let mut data = run_owner::active(id)
        .and_then(|owner| owner.pending())
        .into_iter()
        .collect::<Vec<_>>();
    for (_, mut pending) in children.pending(id) {
        // Parent projection is transport-only. Reply resolves the exact owned
        // child and the broker still mints/consumes a child-bound proof.
        pending.session_id = id.to_string();
        pending.action = format!("Subagent: {}", pending.action);
        data.push(pending);
    }
    Ok(Json(json!({"data":data})))
}

#[derive(Deserialize)]
struct PermissionReplyBody {
    reply: String,
    scope: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct PermissionRuleQuery {session_id:Option<String>}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PermissionRuleBody {tool:String,decision:String,scope:String,session_id:Option<String>}
async fn list_permission_rules(State(state):State<AppState>,Query(query):Query<PermissionRuleQuery>)->Result<Json<Value>,ApiFailure> {
    if let Some(id)=query.session_id.as_deref() {state.sessions.get(parse_session_id(id)?).await.map_err(ApiFailure::internal)?;}
    let rules=tokio::task::spawn_blocking(move||permission_rules::list(query.session_id.as_deref())).await.map_err(ApiFailure::internal)?.map_err(ApiFailure::bad_request)?;
    Ok(Json(json!({"rules":rules})))
}
async fn save_permission_rule(State(state):State<AppState>,Json(body):Json<PermissionRuleBody>)->Result<StatusCode,ApiFailure> {
    if let Some(id)=body.session_id.as_deref() {state.sessions.get(parse_session_id(id)?).await.map_err(ApiFailure::internal)?;}
    let rule=permission_rules::Rule {tool:body.tool,decision:body.decision,scope:body.scope.clone(),
        session_id:if body.scope=="session" {body.session_id} else {None},resource:None,context:None};
    tokio::task::spawn_blocking(move||permission_rules::upsert(rule)).await.map_err(ApiFailure::internal)?.map_err(ApiFailure::bad_request)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn reply_session_permission(
    Path((id, request_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Extension(children): Extension<task_children::Children>,
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
    let Some(owner) = run_owner::active(id).filter(|owner| owner.pending().is_some_and(|p| p.id == request_id))
        .or_else(|| children.permission_owner(id, &request_id)) else {
        return Err(ApiFailure::not_found("permission request not found"));
    };
    let Some(pending) = owner.pending() else {
        return Err(ApiFailure::not_found("permission request not found"));
    };
    if pending.id != request_id || pending.session_id != owner.session_id.to_string() {
        return Err(ApiFailure::not_found("permission request not found"));
    }
    let allow=match body.reply.as_str() {"once"|"always"=>true,"reject"=>false,_=>return Err(ApiFailure::bad_request("invalid permission reply"))};
    let scope=body.scope.as_deref().unwrap_or(if body.reply=="always" {"always"} else {"once"});
    let scope=if scope=="turn" {"once"} else {scope};
    if !matches!(scope,"once"|"session"|"always") || body.reply=="once"&&scope!="once" {return Err(ApiFailure::bad_request("invalid permission scope"));}
    if !owner.reply_scoped(&request_id,allow,scope).map_err(ApiFailure::bad_request)? {return Err(ApiFailure::not_found("permission request not found"));}
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
    let mut rows = Vec::with_capacity(sessions.len());
    for session in sessions {
        let directory = state
            .sessions
            .directory(session.id)
            .await
            .map_err(ApiFailure::internal)?;
        let mut value = serde_json::to_value(session).map_err(ApiFailure::internal)?;
        if let Some(directory) = directory {
            value["directory"] = json!(directory);
        }
        rows.push(value);
    }
    Ok(Json(json!({"sessions":rows})))
}
#[derive(Debug, Deserialize)]
struct CreateSessionBody {
    title: String,
    directory: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NativeShellBody {
    command: String,
    workdir: Option<String>,
}

struct NativeShellReceiver {
    receiver: tokio::sync::mpsc::Receiver<Result<Bytes, Infallible>>,
    owner: Arc<RunOwner>,
}
impl Drop for NativeShellReceiver {
    fn drop(&mut self) {
        self.owner.cancel();
    }
}

async fn native_shell_stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(events): Extension<event_bus::EventBus>,
    Json(body): Json<NativeShellBody>,
) -> Result<Response, ApiFailure> {
    let id = parse_session_id(&id)?;
    let permit = TurnExecutionPermit::acquire(id).map_err(turn_admission_failure)?;
    let owner = permit.owner();
    state.sessions.get(id).await.map_err(ApiFailure::internal)?;
    if body.command.trim().is_empty()
        || body.command.len() > 32 * 1024
        || body.command.contains('\0')
    {
        return Err(ApiFailure::bad_request("invalid shell command"));
    }
    let cwd = session_workdir(&state.sessions, id, body.workdir.as_deref()).await?;
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(cwd.clone()));
    let executor = ToolExecutor::new()
        .with_cwd(cwd.clone())
        .with_broker(broker.clone());
    let user = state
        .sessions
        .append_text(id, MessageRole::User, format!("!{}", body.command))
        .await
        .map_err(ApiFailure::internal)?;
    let message_id = MessageId::new();
    let call_id = format!("shell-{message_id}");
    let arguments = json!({"command":body.command,"workdir":cwd});
    let round = ToolRound {
        round_id: format!("round-{}-0", user.id),
        session_id: id,
        turn_message_id: user.id,
        round_ordinal: 0,
        expected_pairs: 1,
    };
    state
        .sessions
        .admit_tool_round(
            round.clone(),
            vec![ToolCallAdmission {
                call_id: call_id.clone(),
                name: "bash".to_owned(),
                arguments: arguments.to_string(),
                message: MessageRecord {
                    id: message_id,
                    session_id: id,
                    role: MessageRole::Tool,
                    body: PayloadRef::inline("[bash] pending").map_err(ApiFailure::internal)?,
                    created_at: Timestamp::now(),
                },
            }],
        )
        .await
        .map_err(ApiFailure::internal)?;
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<Bytes, Infallible>>(16);
    let receiving_owner = Arc::clone(&owner);
    tokio::spawn(async move {
        let _permit = permit;
        let _ = events.publish(event_bus::ServerEvent::MessageStored {
            session: id,
            message_id: user.id,
        });
        let _ = sender
            .send(Ok(ndjson(json!({"type":"user_message","message":user}))))
            .await;
        let _=sender.send(Ok(ndjson(json!({"type":"tool_call","call_id":call_id,"name":"bash","arguments":arguments.to_string()})))).await;
        let outcome = execute_native_tool(
            opencode_rk_tools::executor::ToolCall::new(&call_id, "bash", arguments),
            &executor,
            &broker,
            &owner,
            user.id,
        )
        .await;
        let (output, payload) = native_result_payload(&outcome.output, Some(outcome.success));
        if let Err(error) = state
            .sessions
            .persist_tool_output(round, 0, payload, message_id)
            .await
        {
            let _ = sender
                .send(Ok(stream_error("internal_error", error.to_string())))
                .await;
            return;
        }
        let _ = events.publish(event_bus::ServerEvent::MessageStored {
            session: id,
            message_id,
        });
        let _=sender.send(Ok(ndjson(json!({"type":"tool_output","call_id":call_id,"name":"bash","output":output,"success":outcome.success,"is_error":!outcome.success})))).await;
        match state
            .sessions
            .append_text(id, MessageRole::Assistant, String::new())
            .await
        {
            Ok(message) => {
                let _ = events.publish(event_bus::ServerEvent::MessageStored {
                    session: id,
                    message_id: message.id,
                });
                let _=sender.send(Ok(ndjson(json!({"type":"assistant_message","message":message,"stop_reason":"completed"})))).await;
            }
            Err(error) => {
                let _ = sender
                    .send(Ok(stream_error("internal_error", error.to_string())))
                    .await;
            }
        }
    });
    let response_stream = stream::unfold(
        NativeShellReceiver {
            receiver,
            owner: receiving_owner,
        },
        |mut state| async move { state.receiver.recv().await.map(|event| (event, state)) },
    );
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(response_stream),
    )
        .into_response())
}
async fn create_session(
    State(state): State<AppState>,
    Extension(events): Extension<event_bus::EventBus>,
    Json(body): Json<CreateSessionBody>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let directory = match body.directory.as_deref() {
        Some(path) => Some(validate_directory(path, None).await?),
        None => None,
    };
    let session = state
        .sessions
        .create_in_directory(body.title, directory.clone())
        .await
        .map_err(ApiFailure::internal)?;
    // Publish only after the durable create succeeded; the bus allocates the
    // shared publication identity (all SSE subscribers observe the same id).
    let _ = events.publish(event_bus::ServerEvent::SessionCreated(session.id));
    let mut value = serde_json::to_value(session).map_err(ApiFailure::internal)?;
    if let Some(directory) = directory {
        value["directory"] = json!(directory);
    }
    Ok((StatusCode::CREATED, Json(json!({"session":value}))))
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
    let mut value = serde_json::to_value(session).map_err(ApiFailure::internal)?;
    if let Some(directory) = state
        .sessions
        .directory(id)
        .await
        .map_err(ApiFailure::internal)?
    {
        value["directory"] = json!(directory);
    }
    Ok(Json(json!({"session":value})))
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
    let limit = params.limit.unwrap_or(100).clamp(1, 500);
    let messages = state
        .sessions
        .messages(id, limit)
        .await
        .map_err(ApiFailure::internal)?;
    let tools = state
        .sessions
        .display_tool_history(id, limit)
        .await
        .map_err(ApiFailure::internal)?;
    let activities = state
        .sessions
        .assistant_activity(id, limit)
        .await
        .map_err(ApiFailure::internal)?;
    let mut rows = Vec::with_capacity(messages.len());
    for message in messages {
        let message_id = message.id;
        let mut value = serde_json::to_value(message).map_err(ApiFailure::internal)?;
        let mut parts = Vec::new();
        for tool in tools.iter().filter(|tool| tool.message_id == message_id) {
            if tool.kind == "call" {
                parts.push(json!({"type":"tool_call","call_id":tool.call_id,"name":tool.name,
                    "arguments":serde_json::from_str::<Value>(&tool.payload).unwrap_or_else(|_|json!({})),
                    "status":if tools.iter().any(|output|output.kind=="output" && output.call_id==tool.call_id && output.round_id==tool.round_id){"completed"}else{"pending"}}));
            } else if tool.kind == "output" {
                let envelope = serde_json::from_str::<Value>(&tool.payload)
                    .ok()
                    .filter(|value| value["format"] == "opencode.tool-result.v1");
                let output = envelope
                    .as_ref()
                    .and_then(|value| value["output"].as_str())
                    .unwrap_or(&tool.payload);
                let success = envelope
                    .as_ref()
                    .and_then(|value| value["success"].as_bool());
                parts.push(json!({"type":"tool_output","call_id":tool.call_id,"name":tool.name,"output":output,
                    "success":success,"is_error":success.map(|success|!success),
                    "error":if success==Some(false){Some(output)}else{None}}));
            }
        }
        // Assistant activity is already durable and bounded independently of the display body.
        for activity in &activities {
            if activity.message_id == message_id {
                value["reasoning_summary"] = json!(activity.reasoning_summary);
            }
        }
        if !parts.is_empty() {
            value["parts"] = json!(parts);
        }
        rows.push(value);
    }
    Ok(Json(json!({"messages":rows})))
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

#[derive(Clone, Debug, Deserialize)]
struct CreateTurnBody {
    text: String,
    model: String,
    #[serde(default)]
    reasoning_effort: String,
    workdir: Option<String>,
    #[serde(skip)]
    agent_prompt: Option<String>,
    #[serde(skip)]
    read_only: bool,
    #[serde(skip)]
    replay_requires_no_tools: bool,
}

/// Internal-only context for a provider retry. This is never deserialized from
/// HTTP: a retry reuses the already durable user message instead of appending
/// another copy of the prompt.
#[derive(Clone, Copy)]
struct RetryTurnContext {
    user_message_id: MessageId,
    child_id: SessionId,
    generation: u32,
}

async fn command_inventory(root: PathBuf) -> Result<configured_commands::Inventory, ApiFailure> {
    static SCANS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
    let permit = SCANS.try_acquire().map_err(|_| ApiFailure::too_many_requests("command inventory is busy"))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        configured_commands::load(&root)
    })
        .await.map_err(|_| ApiFailure::internal("command configuration worker failed"))?
        .map_err(ApiFailure::bad_request)
}

async fn list_configured_commands() -> Result<Json<Value>, ApiFailure> {
    let root = std::env::current_dir().map_err(ApiFailure::internal)?;
    let inventory = command_inventory(root).await?;
    let agents = inventory.mention_agents();
    Ok(Json(json!({"agents":agents,"commands": inventory.commands.into_iter().map(|(name, command)|
        json!({"name":name,"description":command.description,"agent":command.agent,"model":command.model})
    ).collect::<Vec<_>>()})))
}

#[derive(Deserialize)]
struct ConfiguredCommandBody {
    command: String,
    #[serde(default)]
    arguments: String,
    model: Option<String>,
    agent: Option<String>,
}

async fn run_configured_command(
    State(state): State<AppState>,
    Path(raw_id): Path<String>,
    Extension(events): Extension<event_bus::EventBus>,
    Extension(children): Extension<task_children::Children>,
    Json(body): Json<ConfiguredCommandBody>,
) -> Result<Response, ApiFailure> {
    if body.command.is_empty() || body.command.len() > 128 || body.command.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
        return Err(ApiFailure::bad_request("invalid command name"));
    }
    let id = parse_session_id(&raw_id)?;
    state.sessions.get(id).await.map_err(ApiFailure::internal)?;
    let root = session_workdir(&state.sessions, id, None).await?;
    let inventory = command_inventory(root).await?;
    let command = inventory.commands.get(&body.command)
        .ok_or_else(|| ApiFailure::not_found("configured command not found"))?;
    let agent_name = command.agent.as_deref().or(body.agent.as_deref()).unwrap_or("build");
    if agent_name.is_empty() || agent_name.len() > 128 || agent_name.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
        return Err(ApiFailure::bad_request("invalid configured command agent name"));
    }
    let agent = inventory.agents.get(agent_name);
    if !matches!(agent_name, "build" | "plan") && agent.is_none() {
        return Err(ApiFailure::bad_request("configured command agent not found"));
    }
    if agent.is_some_and(|value| value.get("disable").and_then(Value::as_bool) == Some(true)
        || value.get("disabled").and_then(Value::as_bool) == Some(true)) {
        return Err(ApiFailure::bad_request("configured command agent is disabled"));
    }
    if agent.is_some_and(|value| value.get("permissions").is_some() || value.get("permission").is_some()) {
        return Err(ApiFailure::bad_request("command agent-specific permission rules are not available"));
    }
    if command.subagent.or(command.subtask).unwrap_or(false)
        || agent.and_then(|value| value.get("mode")).and_then(Value::as_str) == Some("subagent") {
        return Err(ApiFailure::bad_request("background command execution is not available"));
    }
    let configured_model = command.model.as_deref()
        .or_else(|| agent.and_then(|value| value.get("model")).and_then(Value::as_str))
        .or(body.model.as_deref())
        .ok_or_else(|| ApiFailure::bad_request("command requires a model"))?;
    let (model, mut reasoning_effort) = configured_model.rsplit_once('#')
        .map(|(model, variant)| (model.to_owned(), variant.to_owned()))
        .unwrap_or_else(|| (configured_model.to_owned(), "high".into()));
    if let Some(variant) = command.variant.as_ref() { reasoning_effort = variant.clone(); }
    let text = configured_commands::expand(&command.template, &body.arguments).map_err(ApiFailure::bad_request)?;
    let prompt = agent.and_then(|value| value.get("prompt")).and_then(Value::as_str).unwrap_or("");
    if prompt.len() > 32 * 1024 { return Err(ApiFailure::bad_request("agent prompt exceeds limit")); }
    let read_only = agent_name == "plan"
        || agent.and_then(|value| value.get("read_only")).and_then(Value::as_bool).unwrap_or(false)
        || builtin_agents::resolve(agent_name).is_some_and(|role| role.read_only);
    let agent_prompt = Some(format!("Agent: {agent_name}\n{prompt}"));
    create_turn_stream(State(state), Path(raw_id), Extension(events), Extension(children), Json(CreateTurnBody {
        text, model, reasoning_effort, workdir: None, agent_prompt, read_only, replay_requires_no_tools:false,
    })).await
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

async fn validate_directory(
    raw: &str,
    base: Option<&std::path::Path>,
) -> Result<PathBuf, ApiFailure> {
    if raw.is_empty() || raw.len() > 4096 || raw.contains('\0') {
        return Err(ApiFailure::bad_request("invalid working directory"));
    }
    let path = PathBuf::from(raw);
    let path = if path.is_absolute() {
        path
    } else {
        base.ok_or_else(|| ApiFailure::bad_request("working directory must be absolute"))?
            .join(path)
    };
    let path = tokio::fs::canonicalize(path)
        .await
        .map_err(|_| ApiFailure::bad_request("working directory does not exist"))?;
    if !tokio::fs::metadata(&path)
        .await
        .map_err(ApiFailure::internal)?
        .is_dir()
    {
        return Err(ApiFailure::bad_request(
            "working directory is not a directory",
        ));
    }
    Ok(path)
}

async fn session_workdir(
    sessions: &SessionService,
    id: SessionId,
    requested: Option<&str>,
) -> Result<PathBuf, ApiFailure> {
    let base = sessions
        .directory(id)
        .await
        .map_err(ApiFailure::internal)?
        .unwrap_or(std::env::current_dir().map_err(ApiFailure::internal)?);
    match requested {
        Some(path) => validate_directory(path, Some(&base)).await,
        None => Ok(base),
    }
}

struct NativeToolOutcome {
    output: String,
    success: bool,
    stop_after_outputs: bool,
}

/// Bound encoded durable bytes, including JSON escapes, while retaining valid
/// typed edit previews. A sliced JSON string is not a replayable tool result.
fn native_result_payload(raw: &str, success: Option<bool>) -> (String, String) {
    let encode = |output: &str| match success {
        Some(success) => {
            json!({"format":"opencode.tool-result.v1","output":output,"success":success})
                .to_string()
        }
        None => output.to_owned(),
    };
    let shorten = |text: &str| {
        let mut end = text.len() / 2;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text[..end].to_owned()
    };
    if let Ok(mut edit) = serde_json::from_str::<Value>(raw) {
        if edit["format"] == "opencode.file-edit.v1" && edit["diff"].is_string() {
            edit["output"] = json!("Edit applied");
            for field in ["diff", "path"] {
                loop {
                    let output = edit.to_string();
                    let payload = encode(&output);
                    if output.len() <= MAX_TOOL_OUTPUT_CHARS
                        && payload.len() <= opencode_rk_contracts::MAX_INLINE_PAYLOAD_BYTES
                    {
                        return (output, payload);
                    }
                    let Some(text) = edit[field].as_str().filter(|text| text.len() > 64) else {
                        break;
                    };
                    edit[field] = json!(shorten(text));
                    edit[format!("{field}_truncated")] = json!(true);
                }
            }
        }
    }
    let mut output = truncate_tool_output(raw);
    loop {
        let payload = encode(&output);
        if payload.len() <= opencode_rk_contracts::MAX_INLINE_PAYLOAD_BYTES {
            return (output, payload);
        }
        output = format!("{}\n[truncated]", shorten(&output));
    }
}

/// Permission and actual execution share the executor's exact normalized intent.
async fn execute_native_tool(
    call: opencode_rk_tools::executor::ToolCall,
    executor: &ToolExecutor,
    broker: &PermissionBroker,
    owner: &Arc<RunOwner>,
    message_id: MessageId,
) -> NativeToolOutcome {
    let failed = |message: String, stop| NativeToolOutcome {
        output: message,
        success: false,
        stop_after_outputs: stop,
    };
    let intent = match executor.intent(&call) {
        Ok(intent) => intent,
        Err(error) => return failed(format!("error: {error}"), false),
    };
    let resource = match serde_json::to_string(&intent) {
        Ok(resource) => resource,
        Err(_) => return failed("error: invalid tool intent".to_owned(), false),
    };
    if permission_rules::denied(&call.name,&owner.session_id.to_string(),&intent,broker) {
        return failed("error: denied by saved/session permission rule".into(),false);
    }
    let approval = match broker.authorize(&intent) {
        Decision::Allow => None,
        Decision::Deny { reason } => {
            return failed(format!("error: {} denied: {reason}", call.name), false);
        }
        Decision::RequireHuman { .. } => {
            let request = PermissionRequest {
                id: format!("per_{}", opencode_rk_contracts::ApprovalId::new()),
                session_id: owner.session_id.to_string(),
                action: call.name.clone(),
                resources: vec![resource],
                source: PermissionSource {
                    kind: "tool",
                    message_id: message_id.to_string(),
                    call_id: call.tool_id.clone(),
                },
            };
            let request_id = request.id.clone();
            let receiver = match owner.install_pending(request, intent.clone(), broker) {
                Ok(receiver) => receiver,
                Err(_) => return failed("error: permission request unavailable".to_owned(), true),
            };
            let _pending = PendingPermissionGuard::new(Arc::clone(owner), request_id);
            let mut cancelled = owner.cancellation_receiver();
            let reply = tokio::select! {
                biased;
                _=cancelled.changed()=>ApprovedPermission{reply:PermissionReply::Cancelled,capability:None},
                reply=receiver=>reply.unwrap_or(ApprovedPermission{reply:PermissionReply::Cancelled,capability:None}),
            };
            match reply.reply {
                PermissionReply::Reject => return failed(format!("{} rejected", call.name), true),
                PermissionReply::Cancelled => {
                    return failed(format!("{} cancelled", call.name), true);
                }
                PermissionReply::Once | PermissionReply::Always => match reply.capability {
                    Some(capability) => Some(capability),
                    None => return failed("error: permission proof missing".to_owned(), true),
                },
            }
        }
    };
    if matches!(call.name.as_str(), "read" | "write" | "edit") {
        let executor = executor.clone();
        let owner = Arc::clone(owner);
        let result = tokio::task::spawn_blocking(move || {
            let Some(_effect) = owner.begin_effect() else {
                return None;
            };
            let identity = call.tool_id.clone();
            let approval =
                approval.map(|proof| (proof, owner.session_id.to_string(), owner.run_id, identity));
            Some(executor.execute_file_operation(call, approval))
        })
        .await;
        return match result {
            Ok(Some(result)) => NativeToolOutcome {
                output: if result.success {
                    result.output
                } else {
                    format!(
                        "error: {}",
                        result.error.unwrap_or_else(|| "tool failed".to_owned())
                    )
                },
                success: result.success,
                stop_after_outputs: false,
            },
            Ok(None) => failed("tool cancelled".to_owned(), true),
            Err(_) => failed("tool execution worker failed".to_owned(), true),
        };
    }
    let Some(_effect) = owner.begin_effect() else {
        return failed("tool cancelled".to_owned(), true);
    };
    // Admission is linearized before cancellation, but a blocking mutex guard
    // must never cross an async process wait. The biased cancellation branch
    // below prevents polling an execution cancelled before it starts.
    drop(_effect);
    let call_id = call.tool_id.clone();
    let executor = executor
        .clone()
        .with_cancellation(owner.cancellation_receiver());
    let execution = async {
        match approval {
            Some(approval) => {
                executor
                    .execute_human_approved(
                        call,
                        approval,
                        &owner.session_id.to_string(),
                        owner.run_id,
                        &call_id,
                    )
                    .await
            }
            None => executor.execute(call).await,
        }
    };
    // The process executor observes cancellation and kills/awaits its owned
    // process group. Dropping its future here would skip that cleanup receipt.
    let result = execution.await;
    NativeToolOutcome {
        output: if result.success {
            result.output
        } else {
            format!(
                "error: {}",
                result.error.unwrap_or_else(|| "tool failed".to_owned())
            )
        },
        success: result.success,
        stop_after_outputs: false,
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
        return Err(ApiFailure { status: StatusCode::BAD_GATEWAY, code: "cancelled", message: "turn cancelled".into() });
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
    app: AppState,
    children: task_children::Children,
    qualified_model: String,
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
    agent_prompt: Option<String>,
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
/// Advertise working native tools by default, matching the interactive product.
/// Every effect still crosses the broker; an explicit environment list narrows
/// the available tool set but never grants permissions.
fn turn_tool_config() -> Vec<String> {
    std::env::var("OPENCODE_RK_TURN_TOOLS")
        .unwrap_or_else(|_| "bash,shell,read,write,edit,grep,glob,websearch,webfetch,task".to_owned())
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|name| {
            matches!(
                *name,
                "bash" | "shell" | "read" | "write" | "edit" | "grep" | "glob" | "websearch" | "webfetch" | "task"
            )
        })
        .map(str::to_owned)
        .collect()
}

/// One executed (or explicitly denied) tool output awaiting persistence.
#[derive(Clone)]
struct CallOutputItem {
    call_id: String,
    name: String,
    output: String,
    success: Option<bool>,
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
    if permission_rules::denied("write",&owner.session_id.to_string(),&intent,broker) {
        return outcome("error: write denied by permission rule".into(),false);
    }
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

fn create_turn_stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(events): Extension<event_bus::EventBus>,
    Extension(children): Extension<task_children::Children>,
    Json(body): Json<CreateTurnBody>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, ApiFailure>> + Send>> {
    Box::pin(async move {
        let (response, _, _) = create_turn_stream_with_message(State(state), Path(id), Extension(events), Extension(children), Json(body), None, None).await?;
        Ok(response)
    })
}

pub(crate) fn create_turn_stream_retry(
    state: AppState,
    id: SessionId,
    events: event_bus::EventBus,
    children: task_children::Children,
    body: CreateTurnBody,
    user_message_id: MessageId,
    generation: u32,
    stopped: tokio::sync::watch::Receiver<bool>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(Response, MessageId, Arc<RunOwner>), ApiFailure>> + Send>> {
    Box::pin(async move {
        children.validate_retry_context(id, generation, user_message_id)?;
        create_turn_stream_with_message(State(state), Path(id.to_string()), Extension(events), Extension(children), Json(body), Some(RetryTurnContext { user_message_id, child_id: id, generation }), Some(stopped)).await
    })
}

pub(crate) fn create_turn_stream_initial(
    state: AppState,
    id: SessionId,
    events: event_bus::EventBus,
    children: task_children::Children,
    body: CreateTurnBody,
    stopped: tokio::sync::watch::Receiver<bool>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(Response, MessageId, Arc<RunOwner>), ApiFailure>> + Send>> {
    create_turn_stream_with_message(State(state), Path(id.to_string()), Extension(events), Extension(children), Json(body), None, Some(stopped))
}

pub(crate) fn create_turn_stream_with_message(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(events): Extension<event_bus::EventBus>,
    Extension(children): Extension<task_children::Children>,
    Json(body): Json<CreateTurnBody>,
    retry: Option<RetryTurnContext>,
    child_stop: Option<tokio::sync::watch::Receiver<bool>>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(Response, MessageId, Arc<RunOwner>), ApiFailure>> + Send>> {
    Box::pin(async move {
    let id = parse_session_id(&id)?;
    // Child-lane admission only for trusted child sessions (the child
    // registry treats unknown/poisoned lookups as non-child); legacy create_turn
    // and shell call sites stay on the root lane.
    let permit = if children.is_child(id) {
        TurnExecutionPermit::acquire_child(id).map_err(turn_admission_failure)?
    } else {
        TurnExecutionPermit::acquire(id).map_err(turn_admission_failure)?
    };
    let owner = permit.owner();
    // Hold this session's exclusive permit across the durable replay check
    // and prompt admission; another client cannot insert an effect between.
    if body.replay_requires_no_tools
        && state.sessions.has_tool_admissions(id).await.map_err(ApiFailure::internal)? {
        return Err(ApiFailure::bad_request("retry refused: child session has tool admissions"));
    }
    let project_root = session_workdir(&state.sessions, id, body.workdir.as_deref()).await?;
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
    let user_message = if let Some(retry) = retry {
        if retry.child_id != id || retry.generation == 0 || state.sessions.has_tool_admissions(id).await.map_err(ApiFailure::internal)? {
            return Err(ApiFailure::bad_request("retry refused: child session has tool admissions"));
        }
        let message = state.sessions.messages(id, MAX_RESPONSES_INPUT_MESSAGES).await
            .map_err(ApiFailure::internal)?
            .into_iter()
            .find(|message| message.id == retry.user_message_id)
            .ok_or_else(|| ApiFailure::bad_request("retry message is not a durable session message"))?;
        if message.role != MessageRole::User {
            return Err(ApiFailure::bad_request("retry message is not a user message"));
        }
        let PayloadRef::Inline { text } = &message.body else {
            return Err(ApiFailure::bad_request("retry message payload is not inline"));
        };
        if text.as_str() != body.text.as_str() {
            return Err(ApiFailure::bad_request("retry prompt does not match durable user message"));
        }
        message
    } else {
        let message = state.sessions.append_text(id, MessageRole::User, body.text).await.map_err(ApiFailure::internal)?;
        let _ = events.publish(event_bus::ServerEvent::MessageStored { session: id, message_id: message.id });
        message
    };
    let user_message_id = user_message.id;
    let mut input = bounded_typed_history(
        &state.sessions,
        id,
        MAX_RESPONSES_INPUT_MESSAGES - usize::from(body.agent_prompt.is_some()),
        MAX_RESPONSES_INPUT_BYTES.saturating_sub(body.agent_prompt.as_ref().map_or(0, String::len)),
    )
    .await
    .map_err(ApiFailure::internal)?;
    if let Some(prompt) = body.agent_prompt.as_ref() {
        input.insert(0, ResponsesItem::Text { role: ResponsesRole::System, content: prompt.clone() });
    }

    // Agentic loop setup: advertise tools only when turn-tool execution is
    // explicitly enabled (OPENCODE_RK_TURN_TOOLS). The list is deny-by-default:
    // an advertised tool is executable, an unadvertised one is not.
    let mut turn_tools = turn_tool_config();
    if children.is_child(id) { turn_tools.retain(|name| name != "task"); }
    if body.read_only {
        turn_tools.retain(|name| matches!(name.as_str(), "read" | "grep" | "glob" | "websearch" | "webfetch"));
    }
    let registry = ToolRegistry::new();
    let mut tools: Vec<ResponsesTool> = registry
        .list()
        .into_iter()
        .filter(|tool| turn_tools.contains(&tool.id))
        .map(|tool| {
            ResponsesTool::function(
                tool.id.clone(),
                tool.description.clone(),
                tool.input_schema.clone(),
            )
        })
        .collect();
    if turn_tools.iter().any(|name| name == "task") { tools.push(task_children::schema()); }
    let max_steps = std::env::var("OPENCODE_RK_TURN_MAX_STEPS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|steps| *steps > 0)
        .unwrap_or(MAX_TURN_STEPS);
    let history_items = input;

    let mut child_stop = child_stop;
    let mut owner_cancelled = owner.cancellation_receiver();
    if child_stop.as_mut().is_some_and(|stop| *stop.borrow_and_update())
        || *owner_cancelled.borrow_and_update()
    {
        return Err(ApiFailure { status: StatusCode::BAD_GATEWAY, code: "cancelled", message: "turn cancelled".into() });
    }
    let provider_client = client.clone();
    let provider_result = tokio::select! {
        biased;
        _ = async { if let Some(stop) = child_stop.as_mut() { let _ = stop.changed().await; } else { std::future::pending::<()>().await; } } => return Err(ApiFailure { status: StatusCode::BAD_GATEWAY, code: "cancelled", message: "turn cancelled".into() }),
        _ = owner_cancelled.changed() => return Err(ApiFailure { status: StatusCode::BAD_GATEWAY, code: "cancelled", message: "turn cancelled".into() }),
        result = provider_client.stream_with_tools(model_id, &body.reasoning_effort, &history_items, &tools) => result,
    };
    let provider = provider_result.map_err(provider_failure)?;

    let stream = stream::unfold(
        TurnStreamState {
            app: state.clone(),
            children,
            qualified_model: body.model.clone(),
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
            owner: Arc::clone(&owner),
            events,
            tools,
            enabled_tools: turn_tools,
            broker,
            file_tool,
            history_items,
            agent_prompt: body.agent_prompt,
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
                                                "usage": state.provider.usage(),
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
                                    let code = provider_stream_code(&error);
                                    let failure = provider_failure(error);
                                state.stage = TurnStreamStage::Done;
                                return Some((
                                    Ok::<Bytes, Infallible>(stream_error(
                                        code,
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
                        let executor = ToolExecutor::new()
                            .with_cwd(
                                state
                                    .file_tool
                                    .project_root_path()
                                    .expect("turn file capability is rooted"),
                            )
                            .with_broker(state.broker.clone());
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
                            let mut explicit_success = None;
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
                                if call.name == "task" {
                                    let outcome = task_children::execute(
                                        state.app.clone(), state.children.clone(), state.events.clone(),
                                        &state.owner, &state.broker, state.turn_message_id, &call,
                                        &state.qualified_model,
                                        state.file_tool.project_root_path().expect("turn file capability is rooted"),
                                    ).await;
                                    stop_after_output = outcome.stop_after_outputs;
                                    explicit_success = Some(outcome.success);
                                    outcome.output
                                } else if call.name == "write" {
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
                                    match serde_json::from_str::<Value>(&call.arguments) {
                                        Ok(arguments) => {
                                            let outcome = execute_native_tool(
                                                opencode_rk_tools::executor::ToolCall::new(
                                                    &call.call_id,
                                                    &call.name,
                                                    arguments,
                                                ),
                                                &executor,
                                                &state.broker,
                                                &state.owner,
                                                state.turn_message_id,
                                            )
                                            .await;
                                            stop_after_output = outcome.stop_after_outputs;
                                            explicit_success = Some(outcome.success);
                                            outcome.output
                                        }
                                        Err(_) => {
                                            explicit_success = Some(false);
                                            "error: invalid tool JSON arguments".to_owned()
                                        }
                                    }
                                }
                            } else {
                                format!(
                                    "error: tool '{}' is not permitted by turn policy (enable via OPENCODE_RK_TURN_TOOLS)",
                                    call.name
                                )
                            };
                            let (bounded, persisted) =
                                native_result_payload(&raw_output, explicit_success);
                            // Persist the real output before exposing its event:
                            // a disconnected reader cannot discard a completed pair.
                            if let Err(error) = state
                                .sessions
                                .persist_tool_output(
                                    round.clone(),
                                    index as u64,
                                    persisted,
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
                                    success: explicit_success,
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
                                     "success": item.success,
                                     "is_error": item.success.map(|success|!success),
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
                        let mut history_items = match bounded_typed_history(
                            &state.sessions,
                            state.session_id,
                            MAX_RESPONSES_INPUT_MESSAGES - usize::from(state.agent_prompt.is_some()),
                            MAX_RESPONSES_INPUT_BYTES.saturating_sub(state.agent_prompt.as_ref().map_or(0, String::len)),
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
                        if let Some(prompt) = state.agent_prompt.as_ref() {
                            history_items.insert(0, ResponsesItem::Text { role: ResponsesRole::System, content: prompt.clone() });
                        }
                        state.history_items = history_items;
                        // A disconnected child response is still drained by the
                        // owned producer, but explicit cancellation must also
                        // interrupt a provider request between tool rounds.
                        // Otherwise task_children::stop waits forever for this
                        // producer to release its TurnExecutionPermit.
                        // Create the receiver BEFORE the is_cancelled check: a
                        // new watch receiver sees only subsequent changes, so
                        // a cancellation arriving in the TOCTOU gap between a
                        // separate is_cancelled() call and receiver creation
                        // would otherwise start stream_with_tools unguarded.
                        let mut cancelled = state.owner.cancellation_receiver();
                        if *cancelled.borrow_and_update() {
                            state.stage = TurnStreamStage::Done;
                            return None;
                        }
                        let next_provider = tokio::select! {
                            biased;
                            _ = cancelled.changed() => None,
                            result = state.client.stream_with_tools(
                                &state.model,
                                &state.reasoning_effort,
                                &state.history_items,
                                &state.tools,
                            ) => Some(result),
                        };
                        let Some(next_provider) = next_provider else {
                            state.stage = TurnStreamStage::Done;
                            return None;
                        };
                        match next_provider {
                            Ok(next_stream) => {
                                if *cancelled.borrow_and_update() {
                                    return None;
                                }
                                state.provider = next_stream;
                                state.stage = TurnStreamStage::Provider;
                                continue;
                            }
                            Err(error) => {
                                state.stage = TurnStreamStage::Done;
                                let code = provider_stream_code(&error);
                                let failure = provider_failure(error);
                                return Some((
                                    Ok::<Bytes, Infallible>(stream_error(
                                        code,
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

    let response = (
        StatusCode::CREATED,
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(response_stream),
    )
        .into_response();
    Ok((response, user_message_id, owner))
    })
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
        | ResponsesError::StreamFailed { .. }
        | ResponsesError::UnexpectedStreamEnd => ApiFailure::bad_gateway(error.to_string()),
    }
}

fn provider_stream_code(error: &ResponsesError) -> &'static str {
    match error {
        ResponsesError::StreamFailed { code, .. } if code == "bad_gateway" => "bad_gateway",
        ResponsesError::StreamFailed { code, .. } if code == "service_unavailable" => "service_unavailable",
        ResponsesError::StreamFailed { code, .. } if code == "timeout" => "timeout",
        ResponsesError::StreamFailed { code, .. } if code == "rate_limit" || code == "rate_limited" => "rate_limited",
        ResponsesError::StreamFailed { .. } => "provider_stream_failed",
        ResponsesError::UnexpectedStreamEnd => "bad_gateway",
        _ => "bad_gateway",
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
