//! Native or tool-free summary compaction at the turn boundary.
//!
//! This is deliberately a small boundary policy, rather than a second history
//! implementation.  The session service remains the source of truth for
//! ordering and bounds; this module only decides whether to call the provider.

use super::*;

pub(crate) async fn settings(
    sessions: &SessionService,
    id: SessionId,
) -> Result<opencode_rk_sessions::SessionCompactionSettings, ApiFailure> {
    compaction_settings::load(sessions, id).await
}

/// Perform compaction preflight while the turn permit is held, before a new
/// user row is appended.  A checkpoint is never deleted on provider failure.
pub(crate) async fn before_user_append(
    sessions: &SessionService,
    client: &OpenAiResponsesClient,
    owner: &Arc<RunOwner>,
    session_id: SessionId,
    provider_id: &str,
    model_id: &str,
    next_text: &str,
) -> Result<(), ApiFailure> {
    let cfg = settings(sessions, session_id).await?;
    if !cfg.enabled {
        return Ok(());
    }
    if owner.is_cancelled() {
        return Err(ApiFailure::bad_gateway("turn cancelled"));
    }

    // Capture the head before reading the input. If any writer appends after
    // this point, the existing transactional checkpoint guard rejects the
    // commit rather than treating unseen history as already compacted.
    let boundary = sessions
        .history_page(session_id, None, 1)
        .await
        .map_err(ApiFailure::internal)?
        .0
        .into_iter()
        .last();
    let Some(boundary) = boundary else {
        return Ok(());
    };

    let (history, previous_window) = checkpoint_provider_history(
        sessions,
        session_id,
        &format!("{provider_id}/{model_id}"),
        MAX_RESPONSES_INPUT_MESSAGES,
        MAX_RESPONSES_INPUT_BYTES,
    )
    .await?;
    if history.is_empty() {
        return Ok(());
    }
    let mut canonical = previous_window
        .as_ref()
        .map_or_else(Vec::new, |window| window.output.clone());
    canonical.extend(
        history
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ApiFailure::internal)?,
    );
    let request = opencode_rk_providers::responses::responses_request_payload(
        model_id,
        "",
        &canonical,
        &[],
        false,
    )
    .map_err(provider_failure)?;
    // Include the opaque window and request envelope in this explicitly
    // estimated count; none of it is decoded or presented as measured usage.
    let bytes = serde_json::to_vec(&request)
        .map_err(ApiFailure::internal)?
        .len();
    let estimated_tokens = ((bytes as u64).saturating_add(3) / 4)
        .saturating_add(((next_text.len() as u64).saturating_add(3)) / 4);
    // Reserve the prospective user row, current instruction block, and an
    // optional internal agent prompt. These are rebuilt after compaction.
    if estimated_tokens < cfg.threshold_tokens
        && canonical.len().saturating_add(3) <= MAX_RESPONSES_INPUT_MESSAGES
        && bytes.saturating_add(next_text.len()) < MAX_RESPONSES_INPUT_BYTES
    {
        return Ok(());
    }

    if cfg.strategy != opencode_rk_sessions::CompactionStrategy::Native {
        return fallback_compaction::compact(
            sessions,
            client,
            owner,
            provider_id,
            model_id,
            cfg.keep_recent_turns,
        )
        .await;
    }

    let mut cancelled = owner.cancellation_receiver();
    let window = tokio::select! {
        biased;
        _ = cancelled.changed() => return Err(ApiFailure::bad_gateway("compaction cancelled")),
        result = async {
            match previous_window.as_ref() {
                Some(window) => client.compact_with_compaction_window(model_id, window, &history).await,
                None => client.compact(model_id, &history).await,
            }
        } => result.map_err(provider_failure)?,
    };
    if owner.is_cancelled() {
        return Err(ApiFailure::bad_gateway("compaction cancelled"));
    }
    let window_json = serde_json::to_string(&json!({
        "id": window.id, "object":"response.compaction", "created_at":window.created_at,
        "usage":window.usage, "output":window.output
    }))
    .map_err(ApiFailure::internal)?;
    super::commit_compaction_checkpoint(
        sessions.clone(),
        owner.clone(),
        CompactionCheckpoint {
            boundary_message_id: boundary.id,
            provider_id: provider_id.to_owned(),
            model_id: model_id.to_owned(),
            window_json,
        },
    )
    .await
}
