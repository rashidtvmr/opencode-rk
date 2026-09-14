//! Pure model alias/combo route resolution.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::router::ModelRef;

pub const MAX_COMBO_MODELS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelRoute {
    Direct(ModelRef),
    Combo { name: String, models: Vec<ModelRef> },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelRouteError {
    #[error("invalid model route input")]
    InvalidInput,
    #[error("unknown model route: {0}")]
    UnknownModel(String),
    #[error("combo '{name}' has {actual} models; maximum is {max}")]
    TooManyComboModels {
        name: String,
        max: usize,
        actual: usize,
    },
}

/// Resolve an explicit provider/model target, a named combo, or a named alias.
///
/// Explicit provider/model inputs bypass named lookups. For named inputs,
/// non-empty combos take precedence over aliases, matching the pinned 9router
/// behavior. The caller owns both maps; this function performs no I/O and
/// retains no routing state.
pub fn resolve_model_route(
    input: &str,
    aliases: &BTreeMap<String, ModelRef>,
    combos: &BTreeMap<String, Vec<ModelRef>>,
) -> Result<ModelRoute, ModelRouteError> {
    if input.is_empty() {
        return Err(ModelRouteError::InvalidInput);
    }

    if let Some((provider_id, model_id)) = input.split_once('/') {
        if provider_id.is_empty() || model_id.is_empty() {
            return Err(ModelRouteError::InvalidInput);
        }

        return Ok(ModelRoute::Direct(ModelRef {
            provider_id: provider_id.to_owned(),
            model_id: model_id.to_owned(),
        }));
    }

    if let Some(models) = combos.get(input).filter(|models| !models.is_empty()) {
        if models.len() > MAX_COMBO_MODELS {
            return Err(ModelRouteError::TooManyComboModels {
                name: input.to_owned(),
                max: MAX_COMBO_MODELS,
                actual: models.len(),
            });
        }

        return Ok(ModelRoute::Combo {
            name: input.to_owned(),
            models: models.clone(),
        });
    }

    if let Some(model) = aliases.get(input) {
        return Ok(ModelRoute::Direct(model.clone()));
    }

    Err(ModelRouteError::UnknownModel(input.to_owned()))
}
