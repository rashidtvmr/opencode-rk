//! Small HTTP boundary over the native catalog and session services.
#![forbid(unsafe_code)]
pub mod clients;
pub mod daemon;
pub mod event_bus;
pub mod repo_ops;
pub mod remote_ledger;
pub mod app_client;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use opencode_rk_catalog::{Catalog, CatalogQuery};
use opencode_rk_contracts::{SessionId, WIRE_SCHEMA_VERSION};
use opencode_rk_sessions::SessionService;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{str::FromStr, sync::Arc};
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
