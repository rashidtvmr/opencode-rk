//! Authenticated session todos; replacement is durable before publication.
use crate::event_bus::{EventBus, ServerEvent};
use crate::{parse_session_id, ApiFailure, AppState};
use axum::{
    extract::{Path, State},
    Extension, Json,
};
use opencode_rk_sessions::{validate_todos, Todo};
use serde::Deserialize;
use serde_json::{json, Value};

fn map_session_error(error: opencode_rk_sessions::SessionError) -> ApiFailure {
    match error {
        opencode_rk_sessions::SessionError::NotFound(_) => ApiFailure::not_found(error.to_string()),
        other => ApiFailure::internal(other),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplaceTodos {
    todos: Vec<Todo>,
}

pub(crate) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    let todos = state.sessions.todos(id).await.map_err(map_session_error)?;
    Ok(Json(json!({"todos": todos})))
}

pub(crate) async fn replace(
    State(state): State<AppState>,
    Extension(events): Extension<EventBus>,
    Path(id): Path<String>,
    Json(body): Json<ReplaceTodos>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    validate_todos(&body.todos).map_err(|_| ApiFailure::bad_request("invalid todo collection"))?;
    state
        .sessions
        .replace_todos(id, body.todos.clone())
        .await
        .map_err(map_session_error)?;
    let _ = events.publish(ServerEvent::TodoUpdated {
        session: id,
        todos: body.todos.clone(),
    });
    Ok(Json(json!({"todos": body.todos})))
}
