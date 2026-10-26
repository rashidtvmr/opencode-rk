//! Instruction inspection and provider assembly share one bounded discovery path.
use crate::{parse_session_id, ApiFailure, AppState};
use axum::{
    extract::{Path, State},
    Json,
};
use opencode_rk_contracts::SessionId;
use opencode_rk_providers::responses::{ResponsesItem, ResponsesRole};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const PROMPT_BYTES: usize = 128 * 1024;

#[derive(Serialize)]
pub(crate) struct MemorySource {
    path: String,
    content: String,
    included: bool,
    reason: Option<&'static str>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MemoryPromptUsage {
    pub paths: Vec<String>,
    pub paths_truncated: bool,
}

const MAX_USAGE_MEMORY_PATHS: usize = crate::context_accounting::MAX_USAGE_DETAILS;

fn failure(error: opencode_rk_sessions::SessionError) -> ApiFailure {
    match error {
        opencode_rk_sessions::SessionError::NotFound(_) => {
            ApiFailure::not_found("session not found")
        }
        _ => ApiFailure::internal("instruction selection unavailable"),
    }
}

pub(crate) async fn sources(
    state: &AppState,
    id: SessionId,
) -> Result<Vec<MemorySource>, ApiFailure> {
    let excluded = state
        .sessions
        .memory_exclusions(id)
        .await
        .map_err(failure)?;
    let Some(root) = state.sessions.directory(id).await.map_err(failure)? else {
        return Ok(Vec::new());
    };
    let snapshot = tokio::task::spawn_blocking(move || crate::rules_loader::load_rules(&root))
        .await
        .map_err(|_| ApiFailure::internal("instruction loader unavailable"))?
        .map_err(|_| ApiFailure::internal("instruction sources could not be loaded"))?;
    let mut remaining = PROMPT_BYTES;
    let mut sources = Vec::new();
    for entry in snapshot.entries {
        let path = entry
            .path
            .to_str()
            .ok_or_else(|| ApiFailure::bad_request("instruction path is not UTF-8"))?
            .to_owned();
        let content = String::from_utf8(entry.bytes)
            .map_err(|_| ApiFailure::bad_request("instruction source is not UTF-8"))?;
        let reason = if excluded.contains(&path) {
            Some("session_excluded")
        } else if entry.glob.is_some() {
            Some("conditional_rule_not_selected")
        } else if path.len() + content.len() + 32 > remaining {
            Some("prompt_budget")
        } else {
            None
        };
        if reason.is_none() {
            remaining -= path.len() + content.len() + 32;
        }
        sources.push(MemorySource {
            path,
            content,
            included: reason.is_none(),
            reason,
        });
    }
    Ok(sources)
}

pub(crate) async fn prompt_with_usage(
    state: &AppState,
    id: SessionId,
) -> Result<(String, MemoryPromptUsage), ApiFailure> {
    let mut prompt = String::new();
    let mut usage = MemoryPromptUsage::default();
    for source in sources(state, id).await?.into_iter().filter(|source| source.included) {
        if usage.paths.len() < MAX_USAGE_MEMORY_PATHS {
            usage.paths.push(source.path.clone());
        } else {
            usage.paths_truncated = true;
        }
        prompt.push_str("Instructions from: ");
        prompt.push_str(&source.path);
        prompt.push('\n');
        prompt.push_str(&source.content);
        prompt.push_str("\n\n");
    }
    Ok((prompt, usage))
}

pub(crate) async fn prompt(state: &AppState, id: SessionId) -> Result<String, ApiFailure> {
    prompt_with_usage(state, id).await.map(|(prompt, _)| prompt)
}

pub(crate) async fn prepend_with_usage(
    state: &AppState,
    id: SessionId,
    model: &str,
    items: &mut Vec<ResponsesItem>,
) -> Result<MemoryPromptUsage, ApiFailure> {
    let (prompt, usage) = prompt_with_usage(state, id).await?;
    if !prompt.is_empty() {
        items.insert(0, ResponsesItem::Text { role: ResponsesRole::System, content: prompt });
    }
    match opencode_rk_providers::responses::responses_request_payload(model, "", items, &[], false) {
        Ok(_) => {}
        Err(opencode_rk_providers::responses::ResponsesError::TooManyMessages { .. }
            | opencode_rk_providers::responses::ResponsesError::InputTooLarge { .. }) => {
            return Err(ApiFailure {
                status: axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                code: "context_budget_exceeded",
                message: "selected context exceeds the provider input budget".into(),
            });
        }
        Err(error) => return Err(crate::provider_failure(error)),
    }
    Ok(usage)
}

pub(crate) async fn prepend(
    state: &AppState,
    id: SessionId,
    model: &str,
    items: &mut Vec<ResponsesItem>,
) -> Result<(), ApiFailure> {
    prepend_with_usage(state, id, model, items).await.map(|_| ())
}

pub(crate) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    Ok(Json(
        json!({"sources":sources(&state, parse_session_id(&id)?).await?}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectSource {
    path: String,
    #[serde(default)]
    scope: Option<String>,
}

async fn select(
    state: AppState,
    id: String,
    body: SelectSource,
    included: bool,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&id)?;
    if body.scope.as_deref().is_some_and(|scope| scope != "session") {
        return Err(ApiFailure::bad_request("only session instruction selection is supported"));
    }
    let requested = std::path::PathBuf::from(&body.path);
    let canonical = tokio::task::spawn_blocking(move || requested.canonicalize())
        .await.map_err(|_| ApiFailure::internal("instruction source lookup unavailable"))?
        .map_err(|_| ApiFailure::bad_request("instruction source is not available"))?;
    let path = canonical.to_str().ok_or_else(|| ApiFailure::bad_request("instruction path is not UTF-8"))?.to_owned();
    if !sources(&state, id)
        .await?
        .iter()
        .any(|source| source.path == path)
    {
        return Err(ApiFailure::bad_request(
            "instruction source is not part of this workspace",
        ));
    }
    state
        .sessions
        .set_memory_included(id, path, included)
        .await
        .map_err(failure)?;
    Ok(Json(json!({"sources":sources(&state, id).await?})))
}

pub(crate) async fn exclude(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SelectSource>,
) -> Result<Json<Value>, ApiFailure> {
    select(state, id, body, false).await
}
pub(crate) async fn include(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SelectSource>,
) -> Result<Json<Value>, ApiFailure> {
    select(state, id, body, true).await
}
