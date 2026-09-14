//! Pure model alias/combo route resolution.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::router::ModelRef;

pub const MAX_COMBO_MODELS: usize = 8;
pub const MAX_COMPATIBLE_PROVIDER_NODES: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibleProviderNode {
    pub id: String,
    pub prefix: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompatibleProviderPrefixError {
    InvalidInput,
    UnknownPrefix(String),
    TooManyCompatibleProviderNodes { max: usize, actual: usize },
}

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

/// Resolve an explicit provider/model target while allowing caller-supplied
/// compatible-provider prefixes to override only non-reserved provider names.
///
/// Reserved provider ids/aliases always win over a colliding compatible node,
/// matching pinned 9router model routing. The caller owns both routing tables;
/// this function performs no I/O and retains no global state.
pub fn resolve_compatible_provider_prefix(
    input: &str,
    reserved_aliases: &BTreeMap<String, String>,
    compatible_nodes: &[CompatibleProviderNode],
) -> Result<ModelRef, CompatibleProviderPrefixError> {
    if compatible_nodes.len() > MAX_COMPATIBLE_PROVIDER_NODES {
        return Err(
            CompatibleProviderPrefixError::TooManyCompatibleProviderNodes {
                max: MAX_COMPATIBLE_PROVIDER_NODES,
                actual: compatible_nodes.len(),
            },
        );
    }

    let Some((prefix, model_id)) = input.split_once('/') else {
        return Err(CompatibleProviderPrefixError::InvalidInput);
    };
    if prefix.is_empty() || model_id.is_empty() {
        return Err(CompatibleProviderPrefixError::InvalidInput);
    }

    if let Some(provider_id) = reserved_aliases.get(prefix) {
        return Ok(ModelRef {
            provider_id: provider_id.clone(),
            model_id: model_id.to_owned(),
        });
    }

    if let Some(node) = compatible_nodes.iter().find(|node| node.prefix == prefix) {
        return Ok(ModelRef {
            provider_id: node.id.clone(),
            model_id: model_id.to_owned(),
        });
    }

    Err(CompatibleProviderPrefixError::UnknownPrefix(
        prefix.to_owned(),
    ))
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
