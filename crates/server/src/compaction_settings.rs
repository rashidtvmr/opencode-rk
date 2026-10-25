//! Session-scoped compaction policy, with explicit environment overrides.
use super::*;
use opencode_rk_sessions::{CompactionStrategy, SessionCompactionSettings};

const OVERRIDES: [(&str, &str); 4] = [
    ("ENABLED", "enabled"),
    ("THRESHOLD_TOKENS", "threshold_tokens"),
    ("KEEP_RECENT_TURNS", "keep_recent_turns"),
    ("STRATEGY", "strategy"),
];

fn failure(error: SessionError) -> ApiFailure {
    if matches!(error, SessionError::NotFound(_))
        || error.to_string().starts_with("session not found:")
    {
        ApiFailure::not_found("session not found")
    } else {
        ApiFailure::internal("compaction settings unavailable")
    }
}

fn setting(name: &str) -> Result<Option<String>, ApiFailure> {
    match std::env::var(format!("OPENCODE_RK_COMPACTION__{name}")) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => Err(ApiFailure::bad_request(
            "compaction environment setting must be UTF-8",
        )),
    }
}

fn validate(policy: &SessionCompactionSettings) -> Result<(), ApiFailure> {
    policy.validate().map_err(|_| ApiFailure::bad_request(
        "compaction threshold_tokens must be within 1..=10000000 and keep_recent_turns within 1..=1000",
    ))
}

fn apply_environment(
    mut policy: SessionCompactionSettings,
) -> Result<SessionCompactionSettings, ApiFailure> {
    if let Some(value) = setting("ENABLED")? {
        policy.enabled = match value.as_str() {
            "true" => true,
            "false" => false,
            _ => {
                return Err(ApiFailure::bad_request(
                    "invalid compaction enabled configuration",
                ));
            }
        };
    }
    if let Some(value) = setting("THRESHOLD_TOKENS")? {
        policy.threshold_tokens = value
            .parse()
            .map_err(|_| ApiFailure::bad_request("invalid compaction threshold_tokens"))?;
    }
    if let Some(value) = setting("KEEP_RECENT_TURNS")? {
        policy.keep_recent_turns = value
            .parse()
            .map_err(|_| ApiFailure::bad_request("invalid compaction keep_recent_turns"))?;
    }
    if let Some(value) = setting("STRATEGY")? {
        policy.strategy = match value.as_str() {
            "native" => CompactionStrategy::Native,
            "fallback" => CompactionStrategy::Fallback,
            "auto" => CompactionStrategy::Auto,
            _ => return Err(ApiFailure::bad_request("unsupported compaction strategy")),
        };
    }
    validate(&policy)?;
    Ok(policy)
}

pub(crate) async fn load(
    sessions: &SessionService,
    id: SessionId,
) -> Result<SessionCompactionSettings, ApiFailure> {
    let policy = sessions
        .compaction_settings(id)
        .await
        .map_err(failure)?
        .unwrap_or_default();
    apply_environment(policy)
}

async fn projection(
    state: &AppState,
    id: SessionId,
    configured: &SessionCompactionSettings,
    effective: &SessionCompactionSettings,
) -> Result<Value, ApiFailure> {
    let checkpoint = state
        .sessions
        .compaction_checkpoint(id)
        .await
        .map_err(failure)?;
    let mut native_model = None;
    let checkpoint = match checkpoint {
        None => Value::Null,
        Some(checkpoint) => {
            let value: Value = serde_json::from_str(&checkpoint.window_json)
                .map_err(|_| ApiFailure::internal("compaction checkpoint metadata unavailable"))?;
            let strategy = match value["object"].as_str() {
                Some("response.compaction") => {
                    native_model = Some(format!(
                        "{}/{}",
                        checkpoint.provider_id, checkpoint.model_id
                    ));
                    "native"
                }
                Some("opencode.fallback_compaction") => "fallback",
                _ => "unrecognized",
            };
            json!({"strategy":strategy,"boundary_message_id":checkpoint.boundary_message_id,
                "provider_id":checkpoint.provider_id,"model_id":checkpoint.model_id})
        }
    };
    let overrides: Vec<&str> = OVERRIDES
        .iter()
        .filter_map(|(name, key)| {
            std::env::var_os(format!("OPENCODE_RK_COMPACTION__{name}")).map(|_| *key)
        })
        .collect();
    Ok(json!({
        "scope":"session", "configured":configured, "effective":effective,
        "overridden_by_environment":overrides,
        "capabilities": {
            "native":{"availability":if native_model.is_some() {"verified-for-checkpoint-model"} else {"unverified"},
                "model":native_model,"retention":"provider-owned"},
            "fallback":{"availability":"responses-summary","retention":"keep_recent_turns"},
            "auto":{"resolution":"explicit-native-policy-otherwise-summary-fallback"}
        },
        "threshold_unit":"estimated_tokens", "enabled_scope":"automatic_compaction",
        "estimate_provenance":"UTF-8 bytes divided by four, including request envelope and prospective user text; not measured tokenizer usage",
        "input_limits":{"messages":MAX_RESPONSES_INPUT_MESSAGES,"bytes":MAX_RESPONSES_INPUT_BYTES},
        "checkpoint":checkpoint,"run_active":run_owner::active(id).is_some()
    }))
}

pub(crate) async fn get(
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&raw)?;
    let configured = state
        .sessions
        .compaction_settings(id)
        .await
        .map_err(failure)?
        .unwrap_or_default();
    let effective = apply_environment(configured.clone())?;
    Ok(Json(projection(&state, id, &configured, &effective).await?))
}

pub(crate) async fn put(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    body: Result<Json<SessionCompactionSettings>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&raw)?;
    let Json(configured) = body.map_err(|error| ApiFailure {
        status: error.status(),
        code: "invalid_compaction_settings",
        message: "invalid or oversized compaction settings".into(),
    })?;
    validate(&configured)?;
    // Validate overrides and metadata before the write, so an error response
    // cannot conceal an already-committed preference change.
    let effective = apply_environment(configured.clone())?;
    state.sessions.get(id).await.map_err(failure)?;
    let response = projection(&state, id, &configured, &effective).await?;
    state
        .sessions
        .set_compaction_settings(id, configured)
        .await
        .map_err(failure)?;
    Ok(Json(response))
}
