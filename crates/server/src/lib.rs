//! Small HTTP boundary over the native catalog and session services.
#![forbid(unsafe_code)]
pub mod app_client;
pub mod auto_loop;
pub mod auto_report;
pub mod clients;
pub mod daemon;
pub mod desktop_bridge;
pub mod enterprise_link;
pub mod event_bus;
pub mod rel_verify;
pub mod remote_ledger;
pub mod repo_ops;
pub mod web_config;
pub mod web_footer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use opencode_rk_catalog::{Catalog, CatalogQuery};
use opencode_rk_contracts::{MessageRole, PayloadRef, SessionId, WIRE_SCHEMA_VERSION};
use opencode_rk_providers::responses::{
    OpenAiResponsesClient, ResponsesError, ResponsesInput, ResponsesRole,
    MAX_RESPONSES_INPUT_MESSAGES,
};
use opencode_rk_sessions::SessionService;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{str::FromStr, sync::Arc};
use tokio::sync::Semaphore;

static TURN_PERMITS: Semaphore = Semaphore::const_new(2);
#[derive(Clone)]
pub struct AppState {
    pub sessions: SessionService,
    pub catalog: Arc<Catalog>,
}
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/models", get(search_models))
        .route("/api/models/{provider}/{model}", get(get_model))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}", get(get_session).patch(rename_session))
        .route("/api/sessions/{id}/archive", post(archive_session))
        .route(
            "/api/sessions/{id}/messages",
            get(list_messages).post(append_message),
        )
        .route("/api/sessions/{id}/turns", post(create_turn))
        .with_state(state)
}
async fn health() -> Json<Value> {
    Json(json!({"schema_version":WIRE_SCHEMA_VERSION,"status":"ok","runtime":"native-rust"}))
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
struct CreateTurnBody {
    text: String,
    model: String,
    reasoning_effort: String,
}

async fn create_turn(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CreateTurnBody>,
) -> Result<(StatusCode, Json<Value>), ApiFailure> {
    let _permit = TURN_PERMITS
        .try_acquire()
        .map_err(|_| ApiFailure::too_many_requests("too many active turns"))?;
    let id = parse_session_id(&id)?;
    state.sessions.get(id).await.map_err(ApiFailure::internal)?;

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
    let user_message = state
        .sessions
        .append_text(id, MessageRole::User, body.text)
        .await
        .map_err(ApiFailure::internal)?;
    let history = state
        .sessions
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
    let assistant_message = state
        .sessions
        .append_text(id, MessageRole::Assistant, assistant_text)
        .await
        .map_err(ApiFailure::internal)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "user_message": user_message,
            "assistant_message": assistant_message
        })),
    ))
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
        | ResponsesError::OutputTooLarge { .. } => ApiFailure::bad_gateway(error.to_string()),
    }
}
fn parse_session_id(value: &str) -> Result<SessionId, ApiFailure> {
    SessionId::from_str(value).map_err(|_| ApiFailure::bad_request("invalid session id"))
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
