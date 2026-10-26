//! Bounded, session-scoped inspection and non-destructive context selection.
use crate::{ApiFailure, AppState, TurnExecutionPermit, parse_message_id, parse_session_id};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use opencode_rk_contracts::{MessageRole, PayloadRef, SessionId};
use opencode_rk_sessions::{SessionError, StorageError};
use serde::Deserialize;
use serde_json::{Value, json};

const PREVIEW_BYTES: usize = 96 * 1024;
const ITEM_PREVIEW_BYTES: usize = 4096;
const RESPONSE_BYTES: usize = 512 * 1024;

fn failure(error: SessionError) -> ApiFailure {
    match error {
        SessionError::NotFound(_) | SessionError::Storage(StorageError::SessionNotFound(_)) => {
            ApiFailure::not_found("session not found")
        }
        SessionError::Storage(StorageError::MessageNotFound(_)) => {
            ApiFailure::not_found("context item not found in this session")
        }
        SessionError::Storage(StorageError::TypedHistoryIdentity) => {
            ApiFailure::bad_request("context item identity is invalid or mandatory")
        }
        SessionError::Storage(StorageError::TypedHistoryLimit) => ApiFailure::unprocessable(
            "context selection exceeds its bounded storage or projection budget",
        ),
        _ => ApiFailure::internal("context selection unavailable"),
    }
}

// Call only after SessionService::get established that the session exists.
// A format-2 fork can exist without a legacy storage row; report that boundary
// explicitly instead of calling an existing session missing.
fn selection_failure(error: SessionError) -> ApiFailure {
    match error {
        SessionError::NotFound(_) | SessionError::Storage(StorageError::SessionNotFound(_)) => {
            ApiFailure::unprocessable(
                "context selection is unsupported for this session storage format",
            )
        }
        other => failure(other),
    }
}

fn preview(text: &str, remaining: &mut usize) -> (String, bool) {
    let mut end = text.len().min(ITEM_PREVIEW_BYTES).min(*remaining);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    *remaining -= end;
    (text[..end].to_owned(), end != text.len())
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InspectionQuery {
    before: Option<String>,
    limit: Option<usize>,
}

pub(crate) async fn get(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    Query(query): Query<InspectionQuery>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&raw)?;
    state.sessions.get(id).await.map_err(failure)?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=50).contains(&limit) {
        return Err(ApiFailure::bad_request(
            "context inspection limit must be within 1..=50",
        ));
    }
    let before = query.before.as_deref().map(parse_message_id).transpose()?;
    let selection = state
        .sessions
        .context_selection(id)
        .await
        .map_err(selection_failure)?;
    let checkpoint = state
        .sessions
        .compaction_checkpoint(id)
        .await
        .map_err(failure)?;
    let (messages, next_before) = state
        .sessions
        .history_page(id, before, limit)
        .await
        .map_err(failure)?;
    let mut remaining = PREVIEW_BYTES;
    let mut items = Vec::new();
    let mut shown_bytes = 0u64;
    let checkpoint_included = checkpoint.as_ref().is_some_and(|checkpoint| {
        !selection
            .checkpoints
            .contains(&checkpoint.boundary_message_id)
    });
    let checkpoint_metadata = if let Some(checkpoint) = checkpoint.as_ref() {
        let object: Value = serde_json::from_str(&checkpoint.window_json)
            .map_err(|_| ApiFailure::internal("compaction checkpoint metadata unavailable"))?;
        let strategy = match object["object"].as_str() {
            Some("response.compaction") => "native",
            Some("opencode.fallback_compaction") => "fallback",
            _ => return Err(ApiFailure::internal("unrecognized compaction checkpoint")),
        };
        let checkpoint_id = format!("checkpoint:{}", checkpoint.boundary_message_id);
        let content = "Indivisible compaction checkpoint. Selecting it changes the whole checkpoint and its retained context; raw durable history is preserved.";
        items.push(json!({"id":checkpoint_id,"kind":"checkpoint","role":"context",
            "included":checkpoint_included,"selectable":true,"source":"compaction_checkpoint",
            "content":content,"content_available":false,"opaque":strategy=="native",
            "bytes":checkpoint.window_json.len(),"estimated_tokens":(checkpoint.window_json.len()+3)/4,
            "count_provenance":"stored checkpoint JSON bytes / 4; not measured provider usage"}));
        json!({"id":checkpoint_id,"strategy":strategy,"included":checkpoint_included,
            "boundary_message_id":checkpoint.boundary_message_id,"provider_id":checkpoint.provider_id,
            "model_id":checkpoint.model_id,"retention":"checkpoint is indivisible"})
    } else {
        Value::Null
    };
    for message in messages {
        let compacted = match checkpoint.as_ref() {
            Some(checkpoint) => state
                .sessions
                .context_message_compacted(id, message.id, checkpoint.boundary_message_id)
                .await
                .map_err(failure)?,
            None => false,
        };
        let excluded = selection.messages.contains(&message.id);
        let mandatory = message.role == MessageRole::System;
        let (content, truncated, content_available, bytes) = match &message.body {
            PayloadRef::Inline { text } => {
                let (content, truncated) = preview(text, &mut remaining);
                (content, truncated, true, text.len() as u64)
            }
            PayloadRef::Blob { bytes, .. } => (
                format!("Blob-backed content ({bytes} bytes); body not loaded in bounded preview"),
                true,
                false,
                *bytes,
            ),
        };
        shown_bytes = shown_bytes.saturating_add(bytes);
        let mut item = json!({
            "id":message.id,"kind":if message.role==MessageRole::Tool {"tool_pair"} else {"message"},
            "role":message.role,"source":"durable_history","included":!excluded && !compacted,
            "selection_excluded":excluded,"selectable":!mandatory && !compacted,
            "represented_by_checkpoint":compacted,"mandatory":mandatory,
            "reason":if mandatory {Some("mandatory_system")} else if compacted {Some("checkpoint_is_indivisible")}
                else if excluded {Some("session_excluded")} else {None},
            "content":content,"truncated":truncated,"content_available":content_available,
            "bytes":bytes,"estimated_tokens":bytes.saturating_add(3)/4,
            "count_provenance":"stored UTF-8 bytes / 4; not measured tokenizer usage",
            "created_at":message.created_at,
        });
        if message.role == MessageRole::Tool {
            let records = state
                .sessions
                .context_tool_preview(id, message.id)
                .await
                .map_err(failure)?;
            let records: Vec<Value> = records
                .into_iter()
                .map(|record| {
                    let (payload, clipped) = preview(&record.payload, &mut remaining);
                    json!({"kind":record.kind,"name":record.name,"call_id":record.call_id,
                    "pair_index":record.pair_index,"content":payload,"bytes":record.bytes,
                    "truncated":record.truncated || clipped})
                })
                .collect();
            item["records"] = json!(records);
            item["atomic_selection"] = json!("function call and output share one selection");
        }
        items.push(item);
    }
    // The same bounded loader supplies the actual next turn and compaction.
    // An over-budget projection is reported honestly while inspection remains
    // usable, so exclusions can repair admission rather than hiding old rows.
    let boundary = checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.boundary_message_id);
    let projected = state
        .sessions
        .bounded_context_history(
            id,
            boundary,
            crate::MAX_RESPONSES_INPUT_MESSAGES,
            crate::MAX_RESPONSES_INPUT_BYTES,
        )
        .await;
    let projection = match projected {
        Ok(history) => {
            let entries: Vec<Value> = history
                .into_iter()
                .map(crate::history_item_to_responses_item)
                .collect::<Result<Vec<_>, _>>()
                .map_err(failure)?
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(ApiFailure::internal)?;
            let bytes = serde_json::to_vec(&entries)
                .map_err(ApiFailure::internal)?
                .len()
                .saturating_add(if checkpoint_included {
                    checkpoint
                        .as_ref()
                        .map_or(0, |checkpoint| checkpoint.window_json.len())
                } else {
                    0
                });
            json!({"status":"available","typed_tail_items":entries.len(),"bytes":bytes,
                "estimated_tokens":bytes.saturating_add(3)/4,
                "provenance":"selected typed tail JSON plus included checkpoint JSON; excludes prospective user input, rebuilt instructions, tools and request envelope"})
        }
        Err(SessionError::Storage(StorageError::TypedHistoryLimit)) => {
            json!({"status":"over_budget",
            "reason":"selected typed history exceeds the provider admission budget"})
        }
        Err(_) => {
            json!({"status":"unavailable","reason":"typed context integrity could not be established"})
        }
    };
    for source in crate::memory_http::sources(&state, id).await? {
        let source = serde_json::to_value(source).map_err(ApiFailure::internal)?;
        let content = source["content"]
            .as_str()
            .ok_or_else(|| ApiFailure::internal("invalid memory source"))?;
        let (text, truncated) = preview(content, &mut remaining);
        items.push(json!({"id":format!("memory:{}",source["path"].as_str().unwrap_or_default()),
            "kind":"memory","role":"system","source":source["path"],"included":source["included"],
            "selectable":false,"reason":source["reason"],"selection_control":"/memory",
            "content":text,"truncated":truncated,"bytes":content.len(),
            "estimated_tokens":content.len().saturating_add(3)/4,"count_provenance":"instruction source UTF-8 bytes / 4"}));
    }
    let response = json!({"scope":"session","items":items,"next_before":next_before,
        "usage_report":crate::context_accounting::latest_usage_report(id),
        "projection":projection,"checkpoint":checkpoint_metadata,"run_active":crate::run_owner::active(id).is_some(),
        "page_budget":{"stored_message_bytes":shown_bytes,"preview_bytes":PREVIEW_BYTES-remaining,
            "provenance":"this inspection page only; excluded and compacted durable rows remain visible"},
        "input_limits":{"items":crate::MAX_RESPONSES_INPUT_MESSAGES,"bytes":crate::MAX_RESPONSES_INPUT_BYTES},
        "compaction_settings_route":format!("/api/sessions/{id}/compaction/settings"),
        "history_preserved":true});
    if serde_json::to_vec(&response)
        .map_err(ApiFailure::internal)?
        .len()
        > RESPONSE_BYTES
    {
        return Err(ApiFailure::unprocessable(
            "context inspection exceeds the bounded response budget; request a smaller page",
        ));
    }
    Ok(Json(response))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectionBody {
    item_id: String,
}

async fn select(
    state: AppState,
    raw: String,
    body: SelectionBody,
    included: bool,
) -> Result<Json<Value>, ApiFailure> {
    let id: SessionId = parse_session_id(&raw)?;
    state.sessions.get(id).await.map_err(failure)?;
    if body.item_id.len() > 128 {
        return Err(ApiFailure::bad_request(
            "context identity exceeds its byte budget",
        ));
    }
    let (checkpoint, item) = match body.item_id.strip_prefix("checkpoint:") {
        Some(boundary) => (true, parse_message_id(boundary)?),
        None => (false, parse_message_id(&body.item_id)?),
    };
    // Selection and provider/compaction assembly share the existing per-session
    // permit. A concurrent turn cannot bake an inconsistent selection snapshot.
    let _permit = TurnExecutionPermit::acquire(id).map_err(crate::turn_admission_failure)?;
    if !checkpoint {
        if let Some(current) = state
            .sessions
            .compaction_checkpoint(id)
            .await
            .map_err(failure)?
        {
            if state
                .sessions
                .context_message_compacted(id, item, current.boundary_message_id)
                .await
                .map_err(failure)?
            {
                return Err(ApiFailure {
                    status:axum::http::StatusCode::CONFLICT,code:"compacted_context_indivisible",
                    message:"this history is already represented by an indivisible checkpoint; select the checkpoint instead".into(),
                });
            }
        }
    }
    state
        .sessions
        .set_context_included(id, item, checkpoint, included)
        .await
        .map_err(selection_failure)?;
    Ok(Json(
        json!({"scope":"session","item_id":body.item_id,"excluded":!included,
        "atomic":if checkpoint {"whole_checkpoint"} else {"message_or_complete_tool_pair"},"history_preserved":true}),
    ))
}

pub(crate) async fn exclude(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    Json(body): Json<SelectionBody>,
) -> Result<Json<Value>, ApiFailure> {
    select(state, raw, body, false).await
}

pub(crate) async fn include(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    Json(body): Json<SelectionBody>,
) -> Result<Json<Value>, ApiFailure> {
    select(state, raw, body, true).await
}
