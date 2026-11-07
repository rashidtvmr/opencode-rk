//! Local quota snapshot used by the native sidebar.
//!
//! The Console endpoint is not available to the daemon, so the native server
//! exposes the local, explicitly configured allowance without pretending that
//! an absent allowance is zero.  Values are read at request time to keep the
//! snapshot deterministic for a daemon process and easy to configure in tests.

use axum::{extract::{Query, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{ApiFailure, AppState};

#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    pub provider: String,
    pub model: String,
}

/// Return the configured local allowance for one model.  An absent or invalid
/// `OPENCODE_USAGE_LIMIT` is intentionally `data: null` (unlimited/unknown),
/// rather than an invented zero budget.
pub async fn limits(
    State(state): State<AppState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<Value>, ApiFailure> {
    let model = state.catalog.model(&query.provider, &query.model)
        .map_err(|_| ApiFailure::not_found("model not found"))?;
    let limit = std::env::var("OPENCODE_USAGE_LIMIT").ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0);
    let Some(limit) = limit else { return Ok(Json(json!({"data": null}))); };
    let spent = std::env::var("OPENCODE_USAGE_SPENT").ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(0.0);
    Ok(Json(json!({
        "data": {"limit": limit, "spent": spent, "model": model.model_id},
    })))
}
