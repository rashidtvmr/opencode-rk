//! Authenticated bounded search of local session titles and messages.
use crate::{ApiFailure, AppState};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub(crate) struct SearchQuery {
    q: String,
}

pub(crate) async fn get(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Value>, ApiFailure> {
    let trimmed = query.q.trim();
    if trimmed.len() < 2 || trimmed.len() > 120 || trimmed.chars().any(char::is_control) {
        return Err(ApiFailure::bad_request(
            "search query must contain 2..120 safe characters",
        ));
    }
    let data = state
        .sessions
        .search_content(trimmed.to_owned())
        .await
        .map_err(ApiFailure::internal)?;
    Ok(Json(
        json!({"sessions":data.sessions,"messages":data.messages,
        "limitations":"Search covers local inline user and assistant text, not binary or blob content"}),
    ))
}
