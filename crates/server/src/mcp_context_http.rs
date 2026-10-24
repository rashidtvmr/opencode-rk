//! Session-scoped MCP inclusion projection.
use axum::{extract::{Extension, Path, State, Json}};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::{mcp_http::McpStatusRegistry, ApiFailure, AppState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Inclusion { pub included: bool }

async fn projection(state: &AppState, registry: &McpStatusRegistry, id: opencode_rk_contracts::SessionId) -> Result<Json<Value>, ApiFailure> {
    state.sessions.get(id).await.map_err(ApiFailure::internal)?;
    let excluded = state.sessions.mcp_exclusions(id).await.map_err(ApiFailure::internal)?;
    let statuses = registry.status_map().await;
    let included = statuses.keys().map(|name| (name.clone(), json!(!excluded.iter().any(|x| x == name)))).collect::<serde_json::Map<_, _>>();
    Ok(Json(json!({"status": statuses, "included": included})))
}
pub(crate) async fn get(State(state): State<AppState>, Extension(registry): Extension<McpStatusRegistry>, Path(id): Path<String>) -> Result<Json<Value>, ApiFailure> { projection(&state, &registry, super::parse_session_id(&id)?).await }
pub(crate) async fn put(State(state): State<AppState>, Extension(registry): Extension<McpStatusRegistry>, Path((id, name)): Path<(String, String)>, Json(body): Json<Inclusion>) -> Result<Json<Value>, ApiFailure> {
    let id = super::parse_session_id(&id)?;
    state.sessions.get(id).await.map_err(ApiFailure::internal)?;
    if !registry.contains(&name).await { return Err(ApiFailure::not_found("MCP server not found")); }
    state.sessions.set_mcp_included(id, name, body.included).await.map_err(ApiFailure::internal)?;
    projection(&state, &registry, id).await
}
