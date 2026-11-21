//! Catalogue-backed native model choices and upstream-compatible model recents.
use serde_json::{json, Value};
use crate::native_palette::pickers;
use std::{
    env, fs,
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_MODELS: usize = 5000;
const MAX_STATE_BYTES: usize = 64 * 1024;
const MAX_RECENT: usize = 32;
const MAX_FAVORITE: usize = 32;
// Keep aligned with the private own-config limit in providers::imported_config.
const MAX_OWN_CONFIG_BYTES: usize = 4 * 1024 * 1024;

/// Per-1M-token USD pricing from the catalogue entry's first `cost` row
/// (`entry["cost"][0]`, upstream `ModelInfo.cost`). `input`/`output` are USD
/// per 1,000,000 tokens; only finite non-negative values are admitted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ModelCost {
    pub input: f64,
    pub output: f64,
}

impl ModelCost {
    /// Session cost for one usage observation: `(input*in + output*out)/1e6`
    /// (upstream `cost` semantics). `None` unless the result is finite and
    /// positive, so a zero-priced turn never claims a money row.
    #[must_use]
    pub fn price(&self, input_tokens: u64, output_tokens: u64) -> Option<f64> {
        let cost =
            (input_tokens as f64 * self.input + output_tokens as f64 * self.output) / 1_000_000.0;
        (cost.is_finite() && cost > 0.0).then_some(cost)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Model {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    pub context_window: Option<u64>,
    pub free: bool,
    /// First `cost` row of the catalogue entry, driving the live sidebar USD
    /// session cost (`context.tsx:14/36-38`). `None` when the entry omits cost
    /// or the row is not finite/non-negative.
    pub cost: Option<ModelCost>,
    /// Available named variants for this model, from the catalogue entry's
    /// `variants[].id` (`ModelInfo.variants`, `dialog-variant.tsx:9-42`). Drives
    /// the post-select transition (`dialog-model.tsx:25-38`).
    pub variants: Vec<String>,
    /// Optional human-readable provider display label from the catalogue
    /// entry's `provider_name` (`dialog-model.tsx:120,141` shows
    /// `provider?.name ?? model.providerID`). Bounded and control-character
    /// filtered at assignment; `None` when absent/empty/malformed.
    /// Presentation only: it never participates in `qualified()` and therefore
    /// never changes provider/model identity, recents or submission.
    pub provider_name: Option<String>,
    /// Live provider display label from the `/api/providers` registry, refreshed
    /// on every enrichment pass so a provider rename updates the label instead
    /// of retaining a stale one. A catalogue-provided `provider_name` is honored
    /// ahead of this; `provider_display()` falls back to the exact
    /// `provider_id` when neither is available. Presentation only.
    pub provider_registry_name: Option<String>,
}

/// Local daemon quota projection. `None` means no applicable allowance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct UsageLimit {
    pub limit: f64,
    pub spent: f64,
}

pub(super) fn usage_limit(body: &str) -> Option<UsageLimit> {
    let value: Value = serde_json::from_str(body).ok()?;
    let data = value.get("data")?.as_object()?;
    let limit = data.get("limit")?.as_f64()?;
    let spent = data.get("spent")?.as_f64()?;
    (limit.is_finite() && limit > 0.0 && spent.is_finite() && spent >= 0.0)
        .then_some(UsageLimit { limit, spent })
}

#[cfg(test)]
mod usage_limit_tests {
    use super::usage_limit;
    #[test]
    fn parses_positive_and_rejects_hidden_or_invalid_states() {
        assert_eq!(usage_limit(r#"{"data":{"limit":10,"spent":4}}"#).map(|u| (u.limit,u.spent)), Some((10.0,4.0)));
        assert!(usage_limit(r#"{"data":null}"#).is_none());
        assert!(usage_limit(r#"{"data":{"limit":0,"spent":0}}"#).is_none());
        assert!(usage_limit(r#"{"data":{"limit":10,"spent":-1}}"#).is_none());
    }
}

impl Model {
    pub fn qualified(&self) -> String { format!("{}/{}", self.provider_id, self.id) }
    /// Provider label for display and search: the catalogue-provided display
    /// name when present, then the live registry name, otherwise the exact
    /// `provider_id` (`dialog-model.tsx:120,141` `provider?.name ??
    /// model.providerID`).
    pub fn provider_display(&self) -> &str {
        self.provider_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .or_else(|| {
                self.provider_registry_name
                    .as_deref()
                    .filter(|name| !name.is_empty())
            })
            .unwrap_or(&self.provider_id)
    }
    /// Presentation form of `qualified()`: the provider *display* label plus
    /// the exact model id. Identity (`qualified()`) is unchanged and still
    /// drives submission, recents and persisted configuration.
    pub fn display_qualified(&self) -> String {
        format!("{}/{}", self.provider_display(), self.id)
    }
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty() || [self.id.to_lowercase(), self.name.to_lowercase(), self.qualified().to_lowercase(), self.provider_display().to_lowercase()]
            .iter()
            .any(|value| value.contains(&query) || fuzzy_contains(value, &query))
    }
}

fn fuzzy_contains(value: &str, query: &str) -> bool {
    let mut remaining = value.chars();
    query.chars().all(|needle| remaining.by_ref().any(|candidate| candidate == needle))
}

/// Maximum UTF-8 byte length of a provider display label. Kept strictly at the
/// downstream `NativeView::set_composer_model` bound (256 bytes) so a
/// multi-byte label is truncated here instead of making the setter reject the
/// whole selected metadata. `bounded_provider_name` truncates on a character
/// boundary, so the result is always valid UTF-8 and `<= MAX_PROVIDER_NAME_BYTES`.
const MAX_PROVIDER_NAME_BYTES: usize = 256;

/// Bound and sanitize a provider display label: drop control characters, cap
/// the UTF-8 byte length, trim, and treat an empty result as absent. Applied to
/// both the catalogue `provider_name` and the live `/api/providers` registry so
/// a display label is always safe to paint, bounded, and never fabricated.
fn bounded_provider_name(raw: &str) -> Option<String> {
    let mut bounded = String::new();
    for ch in raw.chars().filter(|c| !c.is_control()) {
        if bounded.len() + ch.len_utf8() > MAX_PROVIDER_NAME_BYTES {
            break;
        }
        bounded.push(ch);
    }
    let trimmed = bounded.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Resolve the bounded registry display label for one provider id. The picker
/// sentinel `custom` is not a configured provider and never contributes a label.
fn registry_name_for(provider_id: &str, providers: &[(String, String, String)]) -> Option<String> {
    providers
        .iter()
        .find(|row| row.0 == provider_id && row.0 != "custom")
        .and_then(|(_, name, _)| bounded_provider_name(name))
}

/// Fill provider display labels from the live provider registry
/// (`/api/providers` rows as `(id, name, base_url)`). This runs on every
/// catalogue load/refresh and recomputes the registry label from scratch, so a
/// provider rename is picked up and a removed provider falls back to its exact
/// id rather than retaining a stale label. A catalogue-provided `provider_name`
/// is honored ahead of the registry label (`provider_display()`). Identity is
/// never touched.
pub(super) fn enrich_provider_names(models: &mut [Model], providers: &[(String, String, String)]) {
    for model in models.iter_mut() {
        model.provider_registry_name = registry_name_for(&model.provider_id, providers);
    }
}

/// Production ordering helper: enrich `models` from the CURRENT provider
/// registry, then re-derive the selected model by exact qualified identity so
/// its display label reflects the current facts (initial load, rename or
/// removal) while provider/model identity is unchanged. A `None` selection
/// stays `None`; a selection no longer catalogued still has its registry label
/// refreshed from the current rows.
pub(super) fn reenrich_selection(
    models: &mut [Model],
    providers: &[(String, String, String)],
    selected: Option<&Model>,
) -> Option<Model> {
    enrich_provider_names(models, providers);
    let current = selected?;
    if let Some(refreshed) = models.iter().find(|row| row.qualified() == current.qualified()) {
        return Some(refreshed.clone());
    }
    let mut fallback = current.clone();
    fallback.provider_registry_name = registry_name_for(&fallback.provider_id, providers);
    Some(fallback)
}

/// Rows are catalogue indices, never arbitrary query text.
pub(super) fn visible_rows(models: &[Model], query: &str) -> Vec<usize> {
    models.iter().enumerate().filter_map(|(index, model)| model.matches(query).then_some(index)).collect()
}
pub(super) fn clamp_row(row: usize, visible_len: usize) -> usize { row.min(visible_len.saturating_sub(1)) }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PickerViewport { pub start: usize, pub len: usize }

pub(super) fn picker_viewport(selected: usize, visible_len: usize, capacity: usize) -> PickerViewport {
    let selected = clamp_row(selected, visible_len);
    let start = selected.saturating_sub(capacity.saturating_sub(1));
    PickerViewport { start, len: visible_len.saturating_sub(start).min(capacity) }
}

pub(super) fn picker_capacity(content_height: usize, list_start: usize, footer_rows: usize) -> usize {
    content_height.saturating_sub(list_start.saturating_add(footer_rows)).min(10)
}

pub(super) fn picker_render_indices(active: usize, visible_len: usize, capacity: usize) -> Vec<usize> {
    let viewport = picker_viewport(active, visible_len, capacity);
    (0..viewport.len).map(|offset| viewport.start + offset).collect()
}

pub(super) fn picker_hit_index(viewport: PickerViewport, row: usize) -> Option<usize> {
    (row < viewport.len).then_some(viewport.start + row)
}

pub(super) fn picker_hit_row(viewport: PickerViewport, content_row: usize, list_start: usize) -> Option<usize> {
    content_row.checked_sub(list_start).and_then(|offset| picker_hit_index(viewport, offset))
}
pub(super) fn move_row(row: usize, visible_len: usize, down: bool) -> usize {
    if visible_len == 0 { return 0; }
    if down { row.saturating_add(1).min(visible_len - 1) } else { row.saturating_sub(1) }
}
pub(super) fn selected_row(models: &[Model], query: &str, row: usize) -> Option<Model> {
    let rows = visible_rows(models, query);
    rows.get(clamp_row(row, rows.len())).and_then(|index| models.get(*index)).cloned()
}

/// Persisted recents (newest first) as picker keys. This is the only wired
/// fact source; favorites/variants/free flags have no native producer yet, so
/// the empty-query picker runs with an empty favorites snapshot and honest
/// `free=false, deprecated=false` candidates (see `picker_rows` above).
pub(super) fn recent_keys() -> Vec<pickers::ModelKey> {
    recent_keys_at(state_path().as_deref())
}

fn recent_keys_at(path: Option<&Path>) -> Vec<pickers::ModelKey> {
    let Some(path) = path else { return Vec::new() };
    let Ok(value) = read_state(path) else { return Vec::new() };
    let Some(recent) = value["recent"].as_array() else { return Vec::new() };
    recent
        .iter()
        .take(MAX_RECENT)
        .filter_map(|entry| {
            let provider = entry["providerID"].as_str()?;
            let id = entry["modelID"].as_str()?;
            Some((provider.to_owned(), id.to_owned()))
        })
        .collect()
}

/// Persisted favorites (newest first) as picker keys. Mirrors `recent_keys`:
/// `save_model`/`toggle_favorite` write the `favorite` array, upstream
/// `local.model.favorite()` reads it (`model-preference.ts:94-100`).
pub(super) fn favorite_keys() -> Vec<pickers::ModelKey> {
    favorite_keys_at(state_path().as_deref())
}

fn favorite_keys_at(path: Option<&Path>) -> Vec<pickers::ModelKey> {
    let Some(path) = path else { return Vec::new() };
    let Ok(value) = read_state(path) else { return Vec::new() };
    let Some(favorite) = value["favorite"].as_array() else { return Vec::new() };
    favorite
        .iter()
        .take(MAX_FAVORITE)
        .filter_map(|entry| {
            let provider = entry["providerID"].as_str()?;
            let id = entry["modelID"].as_str()?;
            Some((provider.to_owned(), id.to_owned()))
        })
        .collect()
}

/// Persisted variant for one model (`variant[provider/model]`, upstream
/// `model-preference.ts:69-74`). An explicit `"default"` normalizes to `None`.
pub(super) fn variant_for(model: &Model) -> Option<String> {
    variant_for_at(state_path().as_deref(), &model.provider_id, &model.id)
}

fn variant_for_at(path: Option<&Path>, provider: &str, id: &str) -> Option<String> {
    let path = path?;
    let value = read_state(path).ok()?;
    let key = format!("{provider}/{id}");
    variant_at(&value, &key)
}

fn variant_at(value: &Value, key: &str) -> Option<String> {
    let variant = value["variant"].get(key)?.as_str()?;
    (!variant.is_empty() && variant != "default").then(|| variant.to_owned())
}

/// Live model-dialog rows in `picker_rows` order (favorites, then recents, for
/// an empty query), ordered against the dialog's frozen-at-open favorites
/// snapshot (`dialog-model.tsx:79-81,105`): favoriting mid-dialog never
/// reorders the list before the dialog is reopened. Recents stay live.
/// Non-empty queries fall through to the fuzzy `visible_rows` filter unchanged.
pub(super) fn dialog_rows_state(
    models: &[Model],
    query: &str,
    picker: &pickers::ModelPickerState,
) -> Vec<usize> {
    picker_rows(models, query, picker, &recent_keys())
}

/// Pure core mirroring `dialog_rows_state` with explicit favorites and recents.
pub(super) fn dialog_rows_with(
    models: &[Model],
    query: &str,
    favorites: &[pickers::ModelKey],
    recents: &[pickers::ModelKey],
) -> Vec<usize> {
    let picker = pickers::ModelPickerState::open(favorites);
    picker_rows(models, query, &picker, recents)
}

/// Commit mapping: the focused picker index (recents-first) -> the model it
/// names. The index is resolved against the dialog's frozen-at-open picker
/// order, never against the raw catalogue, so reordering cannot desync the
/// committed model.
pub(super) fn dialog_selected_row_state(
    models: &[Model],
    query: &str,
    row: usize,
    picker: &pickers::ModelPickerState,
) -> Option<Model> {
    let rows = dialog_rows_state(models, query, picker);
    rows.get(clamp_row(row, rows.len())).and_then(|index| models.get(*index)).cloned()
}

pub(super) fn dialog_selected_row_with(
    models: &[Model],
    query: &str,
    row: usize,
    favorites: &[pickers::ModelKey],
    recents: &[pickers::ModelKey],
) -> Option<Model> {
    let rows = dialog_rows_with(models, query, favorites, recents);
    rows.get(clamp_row(row, rows.len())).and_then(|index| models.get(*index)).cloned()
}

/// DIALOG-12: the selected-model detail line facts
/// (`model-options.tsx:48-105`): price or `Free`, input modalities, context
/// window and `Reasoning` for models advertising variants. The live wire
/// `Model` carries no input-modality list, so that fact is honestly omitted;
/// `reasoning` maps from the advertised named variants (upstream
/// `ModelInfo.variants.length > 0`). No live usage-limits source exists, so
/// `None` is passed (no amount row).
#[must_use]
pub(super) fn model_detail_row(model: &Model) -> pickers::ModelDetailRow {
    pickers::ModelDetailFacts {
        free: model.free,
        input_price_per_million: model.cost.as_ref().map(|cost| cost.input),
        output_price_per_million: model.cost.as_ref().map(|cost| cost.output),
        input_modalities: Vec::new(),
        context_limit: model.context_window.unwrap_or(0),
        reasoning: !model.variants.is_empty(),
    }
    .row(None)
}

/// The dialog to open after committing a model (`dialog-model.tsx:25-38`):
/// a model with variants and no still-valid current variant opens the flat
/// variant picker, otherwise the model dialog simply closes.
pub(super) fn post_select_dialog(model: &Model) -> pickers::ModelSelectionDialog {
    let current = variant_for(model);
    pickers::next_dialog_after_model_select(&model.variants, current.as_deref())
}

/// Flat variant-select options (`dialog-variant.tsx:9-42`) for a committed
/// model. `new_designs` mirrors upstream `useNewDesigns()`.
pub(super) fn variant_options(model: &Model, new_designs: bool) -> Vec<String> {
    let current = variant_for(model);
    pickers::ModelVariantPicker::new(&model.variants, current.as_deref(), new_designs)
        .options()
        .to_vec()
}

/// Persist the chosen variant for a model. `None` (the `default` row) writes
/// an explicit `"default"` so it can override a configured agent variant
/// (`model-preference.ts:60-66,150-155`).
pub(super) fn save_variant(model: &Model, value: Option<&str>) -> Result<(), String> {
    let path = state_path().ok_or("unable to resolve model state directory")?;
    save_variant_at(&path, model, value)
}

fn save_variant_at(path: &Path, model: &Model, value: Option<&str>) -> Result<(), String> {
    let mut value_json = read_state(path)?;
    if value_json.get("variant").is_none() || !value_json["variant"].is_object() {
        value_json["variant"] = json!({});
    }
    let key = format!("{}/{}", model.provider_id, model.id);
    value_json["variant"][key] = json!(value.unwrap_or("default"));
    persist_state(path, &value_json)
}

/// Toggle favorite status for a model, returning the new state. Upstream
/// `toggleFavorite` prepends a newly favorited model and drops an existing one
/// (`local.tsx:465-472`, `model-preference.ts:94-100`), which is exactly what
/// `model.dialog.favorite` (ctrl+f, `config/keybind.ts:165`) invokes.
pub(super) fn toggle_favorite(model: &Model) -> Result<bool, String> {
    let path = state_path().ok_or("unable to resolve model state directory")?;
    toggle_favorite_at(&path, model)
}

fn toggle_favorite_at(path: &Path, model: &Model) -> Result<bool, String> {
    let mut value = read_state(path)?;
    let favorite = value["favorite"].as_array().cloned().unwrap_or_default();
    let matches = |entry: &Value| {
        entry["providerID"].as_str() == Some(&model.provider_id)
            && entry["modelID"].as_str() == Some(&model.id)
    };
    let was_favorite = favorite.iter().any(matches);
    let next: Vec<Value> = if was_favorite {
        favorite.into_iter().filter(|entry| !matches(entry)).collect()
    } else {
        let mut list = vec![json!({"providerID":model.provider_id, "modelID":model.id})];
        list.extend(favorite.into_iter().take(MAX_FAVORITE - 1));
        list
    };
    value["favorite"] = Value::Array(next);
    persist_state(path, &value)?;
    Ok(!was_favorite)
}

/// Catalogue indices in `ModelPickerState` order. An empty query is the full
/// picker order (favorites, then recents, then the remaining catalogue);
/// a non-empty query falls back to the fuzzy `visible_rows` filter. Recents are
/// the only fact source wired today (`save_model`/`recent_selected` above);
/// variants/favorites/free flags have no native source yet, so candidates carry
/// honest `free=false, deprecated=false`.
pub(super) fn picker_rows(
    models: &[Model],
    query: &str,
    picker: &pickers::ModelPickerState,
    recents: &[pickers::ModelKey],
) -> Vec<usize> {
    if !query.trim().is_empty() {
        return visible_rows(models, query);
    }
    let candidates: Vec<pickers::ModelCandidate> = models
        .iter()
        .map(|model| pickers::ModelCandidate {
            provider: model.provider_id.clone(),
            id: model.id.clone(),
            title: model.name.clone(),
            free: model.free,
            deprecated: false,
        })
        .collect();
    pickers::order_models(&candidates, picker.favorites_at_open(), recents, true, false)
        .iter()
        .filter_map(|candidate| {
            models
                .iter()
                .position(|model| model.provider_id == candidate.provider && model.id == candidate.id)
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Dialog {
    None,
    Settings,
    Provider,
    ProviderMethod,
    ProviderId,
    BaseUrl,
    ApiKey,
    Model,
}

/// Interpret an OAuth attempt status payload
/// (`GET /api/integration/{id}/connect/oauth/{attemptID}`; server
/// `AttemptStatus` at `crates/server/src/provider_integration.rs:59-66`).
///
/// Returns `(terminal, message)`: `terminal` is true when the attempt has
/// resolved and the caller should stop polling; `message` is the setup-surface
/// notice to paint (only for a terminal status).
pub(super) fn oauth_status(body: &str) -> (bool, Option<String>) {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    match value["status"].as_str() {
        Some("complete") => (true, Some("OpenAI connected".into())),
        Some("failed") => (
            true,
            Some(format!(
                "OpenAI sign-in failed: {}",
                value["message"].as_str().unwrap_or("unknown error")
            )),
        ),
        Some("expired") => (true, Some("OpenAI sign-in expired; try again".into())),
        // `pending` (and any unrecognized status) keeps the attempt live.
        _ => (false, None),
    }
}


pub(super) fn catalogue(body: &str) -> Result<Vec<Model>, String> {
    if body.len() > 8 * 1024 * 1024 { return Err("model catalogue is too large".into()); }
    let value: Value = serde_json::from_str(body).map_err(|_| "invalid model catalogue")?;
    let entries = value["models"]
        .as_array()
        .ok_or("missing model catalogue")?;
    if entries.len() > MAX_MODELS { return Err("model catalogue exceeds 5000-model limit".into()); }
    let mut models = Vec::new();
    for entry in entries.iter().take(MAX_MODELS) {
        let provider = entry["provider_id"].as_str().unwrap_or("openai");
        if provider.is_empty() || provider.len() > 128 || !provider.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            continue;
        }
        let Some(id) = entry["model_id"].as_str().or_else(|| entry["id"].as_str()) else {
            continue;
        };
        let name = entry["name"].as_str().unwrap_or(id);
        let provider_name = entry["provider_name"].as_str().and_then(bounded_provider_name);
        let costs = entry["cost"].as_array();
        let free = costs
            .map(|cost| !cost.is_empty() && cost.iter().all(|row| row["input"].as_f64() == Some(0.0)))
            .unwrap_or(false);
        let cost = costs
            .and_then(|cost| cost.first())
            .and_then(|row| {
                let input = row["input"].as_f64()?;
                let output = row["output"].as_f64()?;
                if input.is_finite() && output.is_finite() && input >= 0.0 && output >= 0.0 {
                    Some(ModelCost { input, output })
                } else {
                    None
                }
            });
        let variants: Vec<String> = entry["variants"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|variant| {
                        variant["id"].as_str().or_else(|| variant.as_str())
                    })
                    .filter(|variant| !variant.is_empty() && variant.len() <= 128)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if id.is_empty()
            || id.len() > 256
            || name.len() > 256
            || id.chars().any(char::is_control)
            || name.chars().any(char::is_control)
        {
            continue;
        }
        if !models.iter().any(|model: &Model| model.id == id && model.provider_id == provider) {
            models.push(Model {
                id: id.to_owned(),
                name: name.to_owned(),
                provider_id: provider.to_owned(),
                context_window:entry.get("context_window").and_then(Value::as_u64).filter(|value|*value>0),
                free,
                cost,
                variants,
                provider_name,
                provider_registry_name: None,
            });
        }
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

pub(super) fn selected(models: &[Model]) -> Option<Model> {
    selected_with_sources(
        models,
        state_path().as_deref(),
        opencode_rk_providers::imported_config::own_config_path().as_deref(),
    )
}

/// Pure selection core; production `selected()` resolves the canonical own
/// state/config paths, tests pass explicit fixture paths (no env access).
///
/// Precedence: explicit recent state first, then the canonical own config
/// root `model` (exact catalogued `provider/model` identity; malformed,
/// missing, over-limit or unavailable identities are skipped honestly), then
/// the plain catalogue fallback (gpt-5.6, else first row).
pub(super) fn selected_with_sources(
    models: &[Model],
    state: Option<&Path>,
    config: Option<&Path>,
) -> Option<Model> {
    if let Some(path) = state {
        if let Some(model) = recent_selected(models, path) {
            return Some(model);
        }
    }
    if let Some(path) = config {
        if let Some(model) = imported_selected(models, path) {
            return Some(model);
        }
    }
    models
        .iter()
        .find(|model| model.id == "gpt-5.6")
        .or_else(|| models.first())
        .cloned()
}

/// Select the provider to initialize before its model catalogue is loaded.
///
/// Provider ids are deliberately validated against the daemon's provider
/// registry first: a recent entry or an imported model must not turn into an
/// arbitrary network request.  The configured model is only used for its
/// qualified provider portion; model identity itself is checked later against
/// the catalogue returned by that provider.
pub(super) fn startup_provider(
    recent: Option<&str>,
    configured_model: Option<&str>,
    known_providers: &[&str],
) -> String {
    let known = |provider: &str| provider == "openai" || known_providers.iter().any(|id| *id == provider);
    if let Some(provider) = recent.filter(|provider| known(provider)) {
        return provider.to_owned();
    }
    if let Some((provider, _model)) = configured_model.and_then(|model| model.trim().split_once('/')) {
        if known(provider) {
            return provider.to_owned();
        }
    }
    "openai".to_owned()
}

/// Read only the root qualified model identity from the bounded own config.
/// Keeping this reader shared by provider bootstrap and catalogue selection
/// avoids a second unbounded/config-semantic JSON parser.
pub(super) fn configured_model_identity(path: &Path) -> Option<String> {
    let value = read_bounded_json(path, MAX_OWN_CONFIG_BYTES).ok()?;
    let configured = value.get("model")?.as_str()?.trim();
    if configured.is_empty() || configured.len() > 512 || !configured.contains('/') {
        return None;
    }
    Some(configured.to_owned())
}

fn recent_selected(models: &[Model], path: &Path) -> Option<Model> {
    let Ok(value) = read_state(&PathBuf::from(path)) else { return None };
    let recent = value["recent"].as_array()?;
    for entry in recent.iter().take(MAX_RECENT) {
        if let Some(provider) = entry["providerID"].as_str() {
            if let Some(model) = models
                .iter()
                .find(|model| entry["modelID"].as_str() == Some(&model.id) && model.provider_id == provider)
            {
                return Some(model.clone());
            }
        }
    }
    None
}

/// Canonical own config root `model` (`opencode.json`), bounded by its own
/// config limit; only the root `model` key is read, never credential fields.
/// Exact provider/model identity is required; invalid identities fall through.
fn imported_selected(models: &[Model], path: &Path) -> Option<Model> {
    let configured = configured_model_identity(path)?;
    models
        .iter()
        .find(|model| {
            model.qualified() == configured
                || configured
                    .split_once('/')
                    .is_some_and(|(provider, id)| model.provider_id == provider && model.id == id)
        })
        .cloned()
}

pub(super) fn recent_provider() -> Option<String> {
    let value = read_state(&state_path()?).ok()?;
    value["recent"].as_array()?.first()?["providerID"].as_str().map(str::to_owned)
}

pub(super) struct ModelLoad {
    pub provider_id: String,
    rx: tokio::sync::mpsc::Receiver<Result<Vec<Model>, String>>,
    task: tokio::task::JoinHandle<()>,
}

impl ModelLoad {
    pub fn start(origin: &str, auth: Option<&str>, provider: &str) -> Result<Self, String> {
        crate::native_debug::event("models.discovery_start", json!({"provider":provider,"credential_present":auth.is_some()}));
        if !provider.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') || provider.is_empty() {
            return Err("invalid provider id".into());
        }
        let auth = auth.filter(|token| crate::daemon_client::is_wellformed_token(token))
            .ok_or("daemon authentication unavailable")?.to_owned();
        let url = reqwest::Url::parse(origin).map_err(|_| "invalid daemon URL")?;
        if url.scheme() != "http" || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]" | "::1")) || !url.username().is_empty() || url.password().is_some() {
            return Err("model discovery requires authenticated local daemon".into());
        }
        let endpoint = format!("{}/api/providers/{provider}/models", origin.trim_end_matches('/'));
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let task = tokio::spawn(async move {
            let result = async {
                let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none())
                    .timeout(std::time::Duration::from_secs(16)).build().map_err(|_| "unable to create model discovery client")?;
                let mut response = client.get(endpoint).header("Authorization", crate::daemon_client::authorization_header(&auth))
                    .send().await.map_err(|_| "model discovery timed out or daemon unavailable")?;
                if !response.status().is_success() { return Err(format!("model discovery failed (HTTP {}); check provider URL and API key", response.status().as_u16())); }
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(|_| "model discovery response interrupted")? {
                    if bytes.len() + chunk.len() > 8 * 1024 * 1024 { return Err("model list exceeds size limit".into()); }
                    bytes.extend_from_slice(&chunk);
                }
                catalogue(std::str::from_utf8(&bytes).map_err(|_| "invalid model list encoding")?)
            }.await;
            crate::native_debug::event("models.discovery_result", json!({"ok":result.is_ok(),"count":result.as_ref().ok().map(Vec::len)}));
            let _ = tx.send(result).await;
        });
        Ok(Self { rx, task, provider_id: provider.to_owned() })
    }

    pub fn poll(&mut self) -> Option<Result<Vec<Model>, String>> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => Some(Err("model discovery interrupted".into())),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
        }
    }
}

impl Drop for ModelLoad {
    fn drop(&mut self) {
        crate::native_debug::event("models.discovery_owner_drop", json!({"provider":self.provider_id,"finished":self.task.is_finished()}));
        self.task.abort();
    }
}

pub(super) fn save_model(model: &Model) -> Result<(), String> {
    let path = state_path().ok_or("unable to resolve model state directory")?;
    let mut value = read_state(&path)?;
    let mut recent = vec![json!({"providerID":model.provider_id, "modelID":model.id})];
    if let Some(previous) = value["recent"].as_array() {
        recent.extend(
            previous
                .iter()
                .filter(|entry| {
                    entry["providerID"].is_string()
                        && entry["modelID"].is_string()
                        && !(entry["providerID"].as_str() == Some(&model.provider_id)
                            && entry["modelID"].as_str() == Some(&model.id))
                })
                .take(MAX_RECENT - 1)
                .cloned(),
        );
    }
    value["recent"] = Value::Array(recent);
    if value.get("favorite").is_none() {
        value["favorite"] = json!([]);
    }
    if value.get("variant").is_none() {
        value["variant"] = json!({});
    }
    persist_state(&path, &value)
}

/// Shared bounded atomic write for the persisted model state. `save_model`
/// and the DIALOG-12 favorite/variant writers all funnel through here so the
/// 64 KiB cap, `0600` mode and temp-file cleanup stay single-sourced.
fn persist_state(path: &Path, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "unable to encode model state")?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err("model state exceeds size limit".to_owned());
    }
    let parent = path.parent().ok_or("invalid model state path")?;
    fs::create_dir_all(parent).map_err(|_| "unable to create model state directory")?;
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| "unable to create model state")?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        if let Err(error) = fs::remove_file(&temporary) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("model state temporary-file cleanup failed: {error}");
            }
        }
    }
    result.map_err(|_| "unable to persist model selection".to_owned())
}

fn state_path() -> Option<PathBuf> {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".local/state"))
        })
        .map(|path| path.join("opencode/model.json"))
}

fn read_state(path: &Path) -> Result<Value, String> {
    read_bounded_json(path, MAX_STATE_BYTES)
}

fn read_bounded_json(path: &Path, max_bytes: usize) -> Result<Value, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Ok(metadata) if metadata.is_file() && metadata.len() <= max_bytes as u64 => {}
        _ => return Err("model state is not a bounded regular file".to_owned()),
    }
    let file = fs::File::open(path).map_err(|_| "unable to read model state")?;
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unable to read model state")?;
    if bytes.len() > max_bytes {
        return Err("model state exceeds size limit".to_owned());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid model state")?;
    if !value.is_object() {
        return Err("invalid model state".to_owned());
    }
    Ok(value)
}

#[cfg(test)]
mod picker_tests {
    use super::*;

    struct TempDir {
        root: PathBuf,
    }

    impl TempDir {
        fn new(tag: &str) -> Self {
            let root = std::env::temp_dir().join(format!("native-setup-import-{tag}"));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("fixture dir");
            Self { root }
        }
        fn write(&self, name: &str, body: &str) -> PathBuf {
            let path = self.root.join(name);
            std::fs::write(&path, body).expect("fixture file");
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn fixture_models() -> Vec<Model> {
        vec![
            Model { id: "x".into(), name: "Acme X".into(), provider_id: "acme".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
            Model { id: "gpt-5.6".into(), name: "Fallback".into(), provider_id: "openai".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
        ]
    }

    #[test]
    fn picker_rows_orders_recents_first_for_empty_query() {
        let models = vec![
            Model { id: "a".into(), name: "A".into(), provider_id: "p".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
            Model { id: "b".into(), name: "B".into(), provider_id: "p".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
            Model { id: "c".into(), name: "C".into(), provider_id: "p".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
        ];
        let picker = pickers::ModelPickerState::open(&[]);
        let recents = vec![("p".to_owned(), "c".to_owned()), ("p".to_owned(), "a".to_owned())];
        assert_eq!(picker_rows(&models, "", &picker, &recents), vec![2, 0, 1]);
        assert_eq!(picker_rows(&models, "  ", &picker, &recents), vec![2, 0, 1]);
        assert_eq!(picker_rows(&models, "b", &picker, &recents), vec![1]);
        assert_eq!(picker_rows(&models, "B", &picker, &[]), vec![1]);
    }
    #[test]
    fn dialog_rows_resolve_recents_first_index_to_committed_model() {
        let models = vec![
            Model { id: "a".into(), name: "A".into(), provider_id: "p".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
            Model { id: "b".into(), name: "B".into(), provider_id: "p".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
            Model { id: "c".into(), name: "C".into(), provider_id: "p".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
        ];
        let recents = vec![("p".to_owned(), "c".to_owned())];
        // Empty query -> recents-first picker order, not raw catalogue order.
        assert_eq!(dialog_rows_with(&models, "", &[], &recents), vec![2, 0, 1]);
        // Index 0 in picker order is the recent C, not catalogue index 0 (A).
        assert_eq!(dialog_selected_row_with(&models, "", 0, &[], &recents).unwrap().id, "c");
        assert_eq!(dialog_selected_row_with(&models, "", 1, &[], &recents).unwrap().id, "a");
        // Non-empty query keeps the fuzzy `visible_rows` mapping.
        assert_eq!(dialog_rows_with(&models, "b", &[], &recents), vec![1]);
        assert_eq!(dialog_selected_row_with(&models, "b", 0, &[], &recents).unwrap().id, "b");
        // A favorite hoists above the recent for an empty query.
        let favorites = vec![("p".to_owned(), "a".to_owned())];
        assert_eq!(dialog_rows_with(&models, "", &favorites, &recents), vec![0, 2, 1]);
        assert_eq!(dialog_selected_row_with(&models, "", 0, &favorites, &recents).unwrap().id, "a");
    }

    // DIALOG-12: the LIVE model dialog holds ONE `ModelPickerState` refreshed
    // only at the dialog-open sites, so favoriting mid-dialog
    // (`config/keybind.ts:165`) must not reorder the list before it reopens
    // (`dialog-model.tsx:79-81,105`).
    #[test]
    fn dialog12_rows_hold_the_frozen_at_open_favorites_snapshot() {
        let models = vec![
            Model { id: "a".into(), name: "A".into(), provider_id: "p".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
            Model { id: "b".into(), name: "B".into(), provider_id: "p".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
            Model { id: "c".into(), name: "C".into(), provider_id: "p".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None },
        ];
        // Dialog opens while `c` is favorite: order freezes as c, a, b.
        let at_open = pickers::ModelPickerState::open(&[("p".to_owned(), "c".to_owned())]);
        assert_eq!(dialog_rows_state(&models, "", &at_open), vec![2, 0, 1]);
        assert_eq!(dialog_selected_row_state(&models, "", 0, &at_open).unwrap().id, "c");
        // Mid-dialog ctrl+f moves the favorite to `a`; the held picker must
        // still order against the open-time snapshot...
        let mid_dialog_favorites = vec![("p".to_owned(), "a".to_owned())];
        assert_eq!(dialog_rows_state(&models, "", &at_open), vec![2, 0, 1]);
        assert_eq!(dialog_selected_row_state(&models, "", 0, &at_open).unwrap().id, "c");
        // ...while a FRESH open snapshot picks up the new favorite order.
        let reopened = pickers::ModelPickerState::open(&mid_dialog_favorites);
        assert_eq!(dialog_rows_state(&models, "", &reopened), vec![0, 1, 2]);
        assert_eq!(dialog_selected_row_state(&models, "", 0, &reopened).unwrap().id, "a");
        // Non-empty queries keep the fuzzy filter under both snapshots.
        assert_eq!(dialog_rows_state(&models, "b", &at_open), vec![1]);
        assert_eq!(dialog_rows_state(&models, "b", &reopened), vec![1]);
    }

    // DIALOG-12: the selected-model detail line (`model-options.tsx:48-105`).
    // Live `Model` has no input modalities (omitted honestly) and no usage
    // limits (`None` amount override); `Reasoning` maps from advertised
    // named variants (upstream `ModelInfo.variants.length > 0`).
    #[test]
    fn dialog12_detail_row_paints_price_context_and_reasoning() {
        let priced = Model { id: "x".into(), name: "X".into(), provider_id: "p".into(), context_window: Some(2_000_000), free: false, cost: Some(ModelCost { input: 5.0, output: 10.0 }), variants: vec!["thinking".into()], provider_name: None, provider_registry_name: None };
        let detail = model_detail_row(&priced);
        assert_eq!(detail.amount, "$5/$10");
        assert_eq!(detail.facts, vec!["2M".to_owned(), "Reasoning".to_owned()]);
        let free = Model { id: "y".into(), name: "Y".into(), provider_id: "p".into(), context_window: Some(200_000), free: true, cost: Some(ModelCost { input: 0.0, output: 0.0 }), variants: Vec::new(), provider_name: None, provider_registry_name: None };
        let detail = model_detail_row(&free);
        assert_eq!(detail.amount, "Free");
        assert_eq!(detail.facts, vec!["200K".to_owned()]);
        // No cost / no context / no variants: the amount degrades to empty and
        // the context fact degrades to `tokens(0)` = "0". Upstream `ModelDetail`
        // always includes `tokens(model.limit.context)` and `.filter(Boolean)`
        // keeps the non-empty "0", so the fact list is never empty
        // (`model-options.tsx:64-69,117-121`; `limit.context` is a required
        // number in `ModelInfo`).
        let bare = Model { id: "z".into(), name: "Z".into(), provider_id: "p".into(), context_window: None, free: false, cost: None, variants: Vec::new(), provider_name: None, provider_registry_name: None };
        let detail = model_detail_row(&bare);
        assert!(detail.amount.is_empty());
        assert_eq!(detail.facts, vec!["0".to_owned()]);
    }

    #[test]
    fn favorite_and_variant_state_parse_and_roundtrip() {
        let dir = TempDir::new("favorite-variant");
        let state = dir.write(
            "model.json",
            r#"{"favorite":[{"providerID":"p","modelID":"c"},{"providerID":"p","modelID":"a"}],
                "variant":{"p/a":"thinking","p/b":"default","p/c":""},
                "recent":[{"providerID":"p","modelID":"c"}]}"#,
        );
        assert_eq!(
            favorite_keys_at(Some(&state)),
            vec![("p".to_owned(), "c".to_owned()), ("p".to_owned(), "a".to_owned())]
        );
        // Explicit "default" and empty strings normalize to None.
        assert_eq!(variant_for_at(Some(&state), "p", "a"), Some("thinking".to_owned()));
        assert_eq!(variant_for_at(Some(&state), "p", "b"), None);
        assert_eq!(variant_for_at(Some(&state), "p", "c"), None);
        assert_eq!(variant_for_at(Some(&state), "p", "missing"), None);
        assert_eq!(favorite_keys_at(None), Vec::<pickers::ModelKey>::new());
    }

    #[test]
    fn catalogue_parses_variants_from_ids_and_strings() {
        let models = catalogue(r#"{"models":[
            {"provider_id":"acme","id":"x","name":"X","variants":[{"id":"thinking"},{"id":"fast"}]},
            {"provider_id":"acme","id":"y","name":"Y","variants":["low","high"]},
            {"provider_id":"acme","id":"z","name":"Z"}
        ]}"#).unwrap();
        let find = |id: &str| models.iter().find(|m| m.id == id).unwrap();
        assert_eq!(find("x").variants, vec!["thinking".to_owned(), "fast".to_owned()]);
        assert_eq!(find("y").variants, vec!["low".to_owned(), "high".to_owned()]);
        assert!(find("z").variants.is_empty());
    }

    // UIKIT-PROVIDER-DISPLAY: a catalogue-provided `provider_name` is a
    // presentation label only. Identity (`qualified()`), and therefore
    // submission/recents/persisted config, stays the exact provider/model id
    // (`dialog-model.tsx:120,141` `provider?.name ?? model.providerID`).
    #[test]
    fn catalogue_provider_name_is_display_only_and_identity_is_unchanged() {
        let models = catalogue(r#"{"models":[
            {"provider_id":"acme","id":"x","name":"X","provider_name":"Acme Corp"},
            {"provider_id":"beta","id":"x","name":"Other X"}
        ]}"#).unwrap();
        let acme = models.iter().find(|m| m.provider_id == "acme").unwrap();
        assert_eq!(acme.provider_display(), "Acme Corp");
        assert_eq!(acme.qualified(), "acme/x");
        assert_eq!(acme.display_qualified(), "Acme Corp/x");
        // Constructor coherence: the catalogue fill sets the catalogue label and
        // leaves the live registry label unset until enrichment runs.
        assert_eq!(acme.provider_name.as_deref(), Some("Acme Corp"));
        assert_eq!(acme.provider_registry_name, None);
        // A missing `provider_name` degrades to the exact provider id.
        let beta = models.iter().find(|m| m.provider_id == "beta").unwrap();
        assert_eq!(beta.provider_display(), "beta");
        assert_eq!(beta.qualified(), "beta/x");
        assert_eq!(beta.display_qualified(), "beta/x");
        assert_eq!(beta.provider_name, None);
        // Duplicate model ids across providers keep distinct identity and labels.
        assert_ne!(acme.qualified(), beta.qualified());
        assert_ne!(acme.provider_display(), beta.provider_display());
    }

    #[test]
    fn catalogue_provider_name_is_sanitized_and_bounded() {
        let models = catalogue(r#"{"models":[
            {"provider_id":"acme","id":"a","name":"A","provider_name":"  Ac\u0007me  "},
            {"provider_id":"beta","id":"b","name":"B","provider_name":"\u0000\u0007"},
            {"provider_id":"gamma","id":"c","name":"C","provider_name":"   "}
        ]}"#).unwrap();
        let find = |p: &str| models.iter().find(|m| m.provider_id == p).unwrap();
        // Control characters are dropped and whitespace trimmed; an all-control
        // or blank name degrades to the exact provider id (never fabricated).
        assert_eq!(find("acme").provider_display(), "Acme");
        assert_eq!(find("beta").provider_display(), "beta");
        assert_eq!(find("gamma").provider_display(), "gamma");
        // An oversized ASCII name is capped at the byte bound.
        let long = "P".repeat(300);
        let body = format!(r#"{{"models":[{{"provider_id":"delta","id":"d","name":"D","provider_name":"{long}"}}]}}"#);
        let models = catalogue(&body).unwrap();
        assert_eq!(models[0].provider_display().len(), MAX_PROVIDER_NAME_BYTES);
        // A multi-byte name is truncated on a character boundary to at most the
        // byte bound, so the downstream `NativeView::set_composer_model` byte
        // check accepts it instead of clearing the selected metadata.
        let wide = "供".repeat(200); // 3 bytes each -> 600 bytes
        let body = format!(r#"{{"models":[{{"provider_id":"epsilon","id":"e","name":"E","provider_name":"{wide}"}}]}}"#);
        let models = catalogue(&body).unwrap();
        let label = models[0].provider_display();
        assert!(label.len() <= MAX_PROVIDER_NAME_BYTES, "{} bytes", label.len());
        assert!(label.chars().count() < 200);
        assert!(label.chars().all(|c| c == '供'));
        assert!(std::str::from_utf8(label.as_bytes()).is_ok());
        // A malformed (non-string) `provider_name` is ignored, not fabricated.
        let models = catalogue(r#"{"models":[{"provider_id":"zeta","id":"f","name":"F","provider_name":42}]}"#).unwrap();
        assert_eq!(models[0].provider_display(), "zeta");
    }

    // UIKIT-PROVIDER-DISPLAY: the live `/api/providers` registry fills a
    // provider label when `/api/models` omitted `provider_name`, and every
    // enrichment pass recomputes from the CURRENT rows so a rename/removal is
    // reflected instead of retaining a stale label. Identity never changes.
    #[test]
    fn enrich_provider_names_honors_catalogue_and_refreshes_registry_renames() {
        let mut models = catalogue(r#"{"models":[
            {"provider_id":"acme","id":"x","name":"X","provider_name":"Catalogue Acme"},
            {"provider_id":"beta","id":"y","name":"Y"}
        ]}"#).unwrap();
        let registry = vec![
            ("acme".to_owned(), "Registry Acme".to_owned(), String::new()),
            ("beta".to_owned(), "Beta Inc".to_owned(), String::new()),
        ];
        enrich_provider_names(&mut models, &registry);
        // A catalogue-provided name is honored ahead of the registry label.
        assert_eq!(models.iter().find(|m| m.provider_id == "acme").unwrap().provider_display(), "Catalogue Acme");
        // A missing catalogue name is filled from the live registry.
        assert_eq!(models.iter().find(|m| m.provider_id == "beta").unwrap().provider_display(), "Beta Inc");
        // A provider rename refreshes the registry label on the next pass...
        let renamed = vec![("beta".to_owned(), "Beta Renamed".to_owned(), String::new())];
        enrich_provider_names(&mut models, &renamed);
        assert_eq!(models.iter().find(|m| m.provider_id == "beta").unwrap().provider_display(), "Beta Renamed");
        // ...and a provider removed from the registry degrades to its exact id
        // rather than retaining a stale label.
        enrich_provider_names(&mut models, &[]);
        assert_eq!(models.iter().find(|m| m.provider_id == "beta").unwrap().provider_display(), "beta");
        // Identity is never changed by enrichment.
        assert_eq!(models.iter().find(|m| m.provider_id == "beta").unwrap().qualified(), "beta/y");
    }

    #[test]
    fn enrich_provider_names_sanitizes_registry_labels_and_ignores_custom_sentinel() {
        let mut models = catalogue(r#"{"models":[{"provider_id":"acme","id":"x","name":"X"}]}"#).unwrap();
        enrich_provider_names(&mut models, &[("acme".to_owned(), "A\u{7}c\u{0}me".to_owned(), String::new())]);
        assert_eq!(models[0].provider_display(), "Acme");
        let mut models = catalogue(r#"{"models":[{"provider_id":"acme","id":"x","name":"X"}]}"#).unwrap();
        enrich_provider_names(&mut models, &[("acme".to_owned(), "\u{0}\u{7}".to_owned(), String::new())]);
        assert_eq!(models[0].provider_display(), "acme");
        // The picker sentinel `custom` is not a configured provider and never
        // contributes a label.
        let mut models = catalogue(r#"{"models":[{"provider_id":"custom","id":"x","name":"X"}]}"#).unwrap();
        enrich_provider_names(&mut models, &[("custom".to_owned(), "Other — custom".to_owned(), String::new())]);
        assert_eq!(models[0].provider_display(), "custom");
    }

    // UIKIT-PROVIDER-DISPLAY: the display precedence is catalogue name, then the
    // live registry name, then the exact provider id, and an empty value at each
    // level is skipped so a blank catalogue name still surfaces the registry
    // label instead of a blank or fabricated one. Identity is never derived from
    // the display label.
    #[test]
    fn provider_display_skips_empty_labels_and_falls_back_in_order() {
        let base = Model {
            id: "x".into(), name: "X".into(), provider_id: "acme".into(),
            context_window: None, free: false, cost: None, variants: Vec::new(),
            provider_name: None, provider_registry_name: None,
        };
        // No labels -> the exact provider id.
        assert_eq!(base.provider_display(), "acme");
        // Registry only -> the registry label.
        let registry = Model { provider_registry_name: Some("Registry".into()), ..base.clone() };
        assert_eq!(registry.provider_display(), "Registry");
        // Catalogue only -> the catalogue label.
        let cat_label = Model { provider_name: Some("Catalogue".into()), ..base.clone() };
        assert_eq!(cat_label.provider_display(), "Catalogue");
        // Catalogue wins over the registry label.
        let both = Model { provider_name: Some("Catalogue".into()), provider_registry_name: Some("Registry".into()), ..base.clone() };
        assert_eq!(both.provider_display(), "Catalogue");
        // A blank catalogue label falls through to the registry, not a blank.
        let blank = Model { provider_name: Some(String::new()), provider_registry_name: Some("Registry".into()), ..base.clone() };
        assert_eq!(blank.provider_display(), "Registry");
        // A blank registry label degrades to the exact provider id.
        let blank_registry = Model { provider_name: None, provider_registry_name: Some(String::new()), ..base.clone() };
        assert_eq!(blank_registry.provider_display(), "acme");
        // Identity is unaffected by any display label.
        assert_eq!(both.qualified(), "acme/x");
        assert_eq!(both.display_qualified(), "Catalogue/x");
    }

    // UIKIT-PROVIDER-DISPLAY production ordering: the loop clones `selected`
    // before the provider registry is fetched/merged, so the startup render must
    // re-derive the selection from the enriched rows by exact identity to show a
    // real `/api/providers` name when `/api/models` omitted `provider_name`.
    #[test]
    fn reenrich_selection_refreshes_a_pre_enrichment_clone_by_exact_identity() {
        let mut models = catalogue(r#"{"models":[{"provider_id":"acme","id":"x","name":"X"}]}"#).unwrap();
        // Startup: `selected` is cloned before the registry is known.
        let stale = selected_with_sources(&models, None, None).unwrap();
        assert_eq!(stale.provider_display(), "acme");
        let providers = vec![("acme".to_owned(), "Example Provider".to_owned(), String::new())];
        let refreshed = reenrich_selection(&mut models, &providers, Some(&stale)).unwrap();
        assert_eq!(refreshed.provider_display(), "Example Provider");
        assert_eq!(refreshed.qualified(), "acme/x");
        // A rename later refreshes again; identity is stable.
        let renamed = vec![("acme".to_owned(), "Example Renamed".to_owned(), String::new())];
        let refreshed = reenrich_selection(&mut models, &renamed, Some(&refreshed)).unwrap();
        assert_eq!(refreshed.provider_display(), "Example Renamed");
        assert_eq!(refreshed.qualified(), "acme/x");
        // No selection stays no selection.
        assert!(reenrich_selection(&mut models, &renamed, None).is_none());
    }

    // UIKIT-PROVIDER-DISPLAY: a catalogue refresh can drop the selected model
    // (`models.retain(...)` in the live loop) while the selection is still held.
    // The re-enrichment must then refresh the held selection's registry label
    // from the CURRENT rows (and degrade to the exact id when the provider is
    // gone) instead of retaining a stale name. Identity is unchanged.
    #[test]
    fn reenrich_selection_refreshes_a_selection_absent_from_the_catalogue() {
        let mut models = catalogue(r#"{"models":[{"provider_id":"acme","id":"x","name":"X"}]}"#).unwrap();
        // A selection cloned before the refresh, no longer catalogued, carrying a
        // stale registry label.
        let gone = Model {
            id: "gone".into(), name: "Gone".into(), provider_id: "acme".into(),
            context_window: None, free: false, cost: None, variants: Vec::new(),
            provider_name: None, provider_registry_name: Some("Stale Name".into()),
        };
        let providers = vec![("acme".to_owned(), "Example Provider".to_owned(), String::new())];
        let refreshed = reenrich_selection(&mut models, &providers, Some(&gone)).unwrap();
        assert_eq!(refreshed.qualified(), "acme/gone");
        assert_eq!(refreshed.provider_display(), "Example Provider");
        // A later removal of the provider degrades to the exact id, not the
        // stale label, and identity stays stable.
        let refreshed = reenrich_selection(&mut models, &[], Some(&refreshed)).unwrap();
        assert_eq!(refreshed.provider_display(), "acme");
        assert_eq!(refreshed.qualified(), "acme/gone");
    }

    #[test]
    fn provider_display_name_participates_in_search_without_changing_identity() {
        let models = catalogue(r#"{"models":[
            {"provider_id":"acme","id":"x","name":"Example Model","provider_name":"Example Provider"}
        ]}"#).unwrap();
        let model = &models[0];
        // Upstream searches `["title","category"]`; the provider display name is
        // the category, so it participates in search.
        assert!(model.matches("Example Provider"));
        assert!(model.matches("example provider"));
        // The exact provider *id* still matches (identity is preserved).
        assert!(model.matches("acme"));
        assert!(model.matches("Example Model"));
        assert!(model.matches("acme/x"));
        assert!(!model.matches("nonexistent"));
        assert_eq!(model.qualified(), "acme/x");
        // The live registry label participates once enriched when the catalogue
        // omitted `provider_name`.
        let mut enriched = catalogue(r#"{"models":[{"provider_id":"acme","id":"x","name":"Example Model"}]}"#).unwrap();
        enrich_provider_names(&mut enriched, &[("acme".to_owned(), "Registry Provider".to_owned(), String::new())]);
        assert!(enriched[0].matches("Registry Provider"));
        assert_eq!(enriched[0].qualified(), "acme/x");
    }

    // UIKIT-PROVIDER-DISPLAY: the picker search path (`visible_rows`, used by
    // the model dialog) finds a row by its provider display label, and a
    // provider rename/removal refreshes that searchability from the CURRENT
    // registry rows without changing identity or the catalogue order.
    #[test]
    fn picker_search_finds_by_provider_display_label_and_refreshes() {
        let mut models = catalogue(r#"{"models":[
            {"provider_id":"acme","id":"x","name":"Example Model"},
            {"provider_id":"beta","id":"y","name":"Other Model"}
        ]}"#).unwrap();
        // Before enrichment the provider id is the only provider text.
        assert_eq!(visible_rows(&models, "Example Provider"), Vec::<usize>::new());
        assert_eq!(visible_rows(&models, "acme"), vec![0]);
        enrich_provider_names(
            &mut models,
            &[("acme".to_owned(), "Example Provider".to_owned(), String::new())],
        );
        assert_eq!(visible_rows(&models, "Example Provider"), vec![0]);
        assert_eq!(visible_rows(&models, "example provider"), vec![0]);
        // A rename refreshes searchability; the old label no longer matches.
        enrich_provider_names(
            &mut models,
            &[("acme".to_owned(), "Renamed Provider".to_owned(), String::new())],
        );
        assert_eq!(visible_rows(&models, "Example Provider"), Vec::<usize>::new());
        assert_eq!(visible_rows(&models, "Renamed Provider"), vec![0]);
        // A provider removed from the registry degrades to the exact id.
        enrich_provider_names(&mut models, &[]);
        assert_eq!(visible_rows(&models, "Renamed Provider"), Vec::<usize>::new());
        assert_eq!(visible_rows(&models, "acme"), vec![0]);
        // Identity and catalogue order are unchanged by enrichment.
        assert_eq!(models[0].qualified(), "acme/x");
        assert_eq!(models[1].qualified(), "beta/y");
    }

    #[test]
    fn post_select_dialog_opens_variant_picker_only_when_variants_exist() {
        let with_variants = Model {
            id: "x".into(), name: "X".into(), provider_id: "p".into(),
            context_window: None, free: false, cost: None,
            variants: vec!["default".into(), "thinking".into()],
            provider_name: None,
            provider_registry_name: None,
        };
        let no_variants = Model { variants: Vec::new(), ..with_variants.clone() };
        assert_eq!(post_select_dialog(&with_variants), pickers::ModelSelectionDialog::VariantPicker);
        assert_eq!(post_select_dialog(&no_variants), pickers::ModelSelectionDialog::Closed);
        // Variant options are `default` first, then the named variants.
        assert_eq!(variant_options(&with_variants, false), vec!["default".to_owned(), "thinking".to_owned()]);
    }

    #[test]
    fn toggle_favorite_prepends_new_and_drops_existing() {
        let dir = TempDir::new("favorite-toggle");
        let state = dir.write("model.json", r#"{"favorite":[{"providerID":"p","modelID":"c"}]}"#);
        let model = |id: &str| Model {
            id: id.into(), name: id.into(), provider_id: "p".into(),
            context_window: None, free: false, cost: None, variants: Vec::new(),
            provider_name: None,
            provider_registry_name: None,
        };
        // A new favorite is prepended (upstream `toggleFavorite`).
        assert_eq!(toggle_favorite_at(&state, &model("a")), Ok(true));
        assert_eq!(
            favorite_keys_at(Some(&state)),
            vec![("p".to_owned(), "a".to_owned()), ("p".to_owned(), "c".to_owned())]
        );
        // Toggling again drops it and leaves the rest intact.
        assert_eq!(toggle_favorite_at(&state, &model("a")), Ok(false));
        assert_eq!(favorite_keys_at(Some(&state)), vec![("p".to_owned(), "c".to_owned())]);
    }

    #[test]
    fn save_variant_persists_named_and_normalizes_default() {
        let dir = TempDir::new("variant-save");
        let state = dir.write("model.json", "{}");
        let model = Model {
            id: "a".into(), name: "A".into(), provider_id: "p".into(),
            context_window: None, free: false, cost: None, variants: Vec::new(),
            provider_name: None,
            provider_registry_name: None,
        };
        assert_eq!(save_variant_at(&state, &model, Some("thinking")), Ok(()));
        assert_eq!(variant_for_at(Some(&state), "p", "a"), Some("thinking".to_owned()));
        // The `default` row persists an explicit "default", which reads back as None.
        assert_eq!(save_variant_at(&state, &model, None), Ok(()));
        assert_eq!(variant_for_at(Some(&state), "p", "a"), None);
    }

    #[test]
    fn imported_catalogued_model_is_default_without_recent_selection() {
        let dir = TempDir::new("default");
        let config = dir.write("opencode.json", r#"{"model":"acme/x"}"#);
        let picked = selected_with_sources(&fixture_models(), None, Some(&config));
        assert_eq!(picked.map(|m| m.qualified()), Some("acme/x".into()));
    }

    #[test]
    fn explicit_recent_selection_beats_imported_config_model() {
        let dir = TempDir::new("recent-beats-import");
        let state = dir.write("model.json", r#"{"recent":[{"providerID":"openai","modelID":"gpt-5.6"}]}"#);
        let config = dir.write("opencode.json", r#"{"model":"acme/x"}"#);
        let picked = selected_with_sources(&fixture_models(), Some(&state), Some(&config));
        assert_eq!(picked.map(|m| m.qualified()), Some("openai/gpt-5.6".into()));
    }

    #[test]
    fn imported_model_survives_large_agent_prompt_within_own_config_limit() {
        let dir = TempDir::new("large-own-config");
        let body = json!({
            "model": "acme/x",
            "agent": { "large_prompt": "p".repeat(MAX_STATE_BYTES + 1) }
        })
        .to_string();
        assert!(body.len() > MAX_STATE_BYTES);
        assert!(body.len() <= MAX_OWN_CONFIG_BYTES);
        let config = dir.write("opencode.json", &body);

        let picked = selected_with_sources(&fixture_models(), None, Some(&config));
        assert_eq!(picked.map(|model| model.qualified()), Some("acme/x".into()));
    }

    #[test]
    fn imported_model_over_own_config_limit_falls_back() {
        let dir = TempDir::new("oversized-own-config");
        let body = json!({
            "model": "acme/x",
            "agent": { "large_prompt": "p".repeat(MAX_OWN_CONFIG_BYTES) }
        })
        .to_string();
        assert!(body.len() > MAX_OWN_CONFIG_BYTES);
        let config = dir.write("opencode.json", &body);

        let picked = selected_with_sources(&fixture_models(), None, Some(&config));
        assert_eq!(picked.map(|model| model.qualified()), Some("openai/gpt-5.6".into()));
    }

    #[test]
    fn unknown_or_malformed_imported_model_falls_back_without_panic() {
        let dir = TempDir::new("import-fallbacks");
        let models = fixture_models();
        let malformed = dir.write("malformed.opencode.json", "{");
        let numeric = dir.write("numeric.opencode.json", r#"{"model":123}"#);
        let empty = dir.write("empty.opencode.json", r#"{"model":""}"#);
        let unknown = dir.write("unknown.opencode.json", r#"{"model":"acme/nonexistent"}"#);
        for path in [&malformed, &numeric, &empty, &unknown] {
            let picked = selected_with_sources(&models, None, Some(path));
            assert_eq!(picked.map(|m| m.qualified()), Some("openai/gpt-5.6".into()));
        }
        let picked = selected_with_sources(&models, None, Some(&dir.root.join("absent.opencode.json")));
        assert_eq!(picked.map(|m| m.qualified()), Some("openai/gpt-5.6".into()));
        // No gpt-5.6 row in the catalogue: the unknown import must never
        // conjure an entry; selection falls through to the fallback honestly.
        let fallback = vec![fixture_models()[0].clone()];
        let picked = selected_with_sources(&fallback, None, Some(&unknown));
        assert_eq!(picked.map(|m| m.qualified()), Some("acme/x".into()));
    }

    #[test]
    fn imported_model_requires_exact_provider_identity() {
        let dir = TempDir::new("import-identity");
        // Same catalog id, different provider: provider/model identity only.
        let config = dir.write("opencode.json", r#"{"model":"other/gpt-5.6"}"#);
        let picked = selected_with_sources(&fixture_models(), None, Some(&config));
        assert_eq!(picked.map(|m| m.qualified()), Some("openai/gpt-5.6".into()));
        // A bare model id without provider qualification is never auto-matched.
        let config = dir.write("bare.opencode.json", r#"{"model":"x"}"#);
        let picked = selected_with_sources(&fixture_models(), None, Some(&config));
        assert_eq!(picked.map(|m| m.qualified()), Some("openai/gpt-5.6".into()));
    }

    #[test]
    fn startup_provider_prefers_recent_then_imported_then_openai_and_validates_registry() {
        let known = ["acme", "9router"];
        assert_eq!(startup_provider(Some("9router"), Some("acme/x"), &known), "9router");
        assert_eq!(startup_provider(Some("missing"), Some("acme/x"), &known), "acme");
        assert_eq!(startup_provider(None, Some("missing/x"), &known), "openai");
        assert_eq!(startup_provider(Some("custom"), Some("acme/x"), &known), "acme");
    }

    #[test]
    fn imported_model_identity_drives_actual_startup_provider_before_catalogue() {
        let dir = TempDir::new("startup-import");
        let config = dir.write("opencode.json", r#"{"model":"acme/x"}"#);
        let identity = configured_model_identity(&config);
        assert_eq!(identity.as_deref(), Some("acme/x"));
        // The model is intentionally absent until the provider request
        // completes; only its validated provider is selected at bootstrap.
        assert_eq!(startup_provider(None, identity.as_deref(), &["acme"]), "acme");
        let loaded = catalogue(r#"{"models":[{"provider_id":"acme","id":"x","name":"Acme X"}]}"#).unwrap();
        assert_eq!(selected_with_sources(&loaded, None, Some(&config)).unwrap().qualified(), "acme/x");
    }

    #[test]
    fn catalogue_derives_free_only_from_non_empty_zero_input_costs() {
        let models = catalogue(r#"{"models":[
            {"provider_id":"free","id":"zero","name":"Zero","cost":[{"input":0,"output":0}]},
            {"provider_id":"paid","id":"priced","name":"Priced","cost":[{"input":0.01,"output":0}]},
            {"provider_id":"unknown","id":"missing","name":"Missing"},
            {"provider_id":"empty","id":"empty","name":"Empty","cost":[]}
        ]}"#).unwrap();
        assert!(models.iter().find(|m| m.id == "zero").unwrap().free);
        assert!(!models.iter().find(|m| m.id == "priced").unwrap().free);
        assert!(!models.iter().find(|m| m.id == "missing").unwrap().free);
        assert!(!models.iter().find(|m| m.id == "empty").unwrap().free);
    }

    #[test]
    fn catalogue_parses_first_cost_row_and_prices_usage() {
        let models = catalogue(r#"{"models":[
            {"provider_id":"openai","id":"priced","name":"Priced","cost":[{"input":3.0,"output":12.0}]},
            {"provider_id":"openai","id":"free","name":"Free","cost":[{"input":0,"output":0}]},
            {"provider_id":"openai","id":"nocost","name":"Missing"},
            {"provider_id":"openai","id":"empty","name":"Empty","cost":[]}
        ]}"#).unwrap();
        let cost = |id: &str| models.iter().find(|m| m.id == id).unwrap().cost;
        // First cost row is retained; absent/empty becomes None.
        assert_eq!(cost("priced"), Some(ModelCost { input: 3.0, output: 12.0 }));
        assert_eq!(cost("free"), Some(ModelCost { input: 0.0, output: 0.0 }));
        assert_eq!(cost("nocost"), None);
        assert_eq!(cost("empty"), None);
        // Upstream `cost` semantics: (input*in + output*out)/1e6.
        assert_eq!(cost("priced").unwrap().price(1_000_000, 1_000_000), Some(15.0));
        assert_eq!(cost("priced").unwrap().price(500_000, 0), Some(1.5));
        // A zero-priced or zero-token turn never claims a money row.
        assert_eq!(cost("free").unwrap().price(1_000_000, 1_000_000), None);
        assert_eq!(cost("priced").unwrap().price(0, 0), None);
    }

    #[test]
    fn custom_catalogue_preserves_router_identity_and_complete_list() {
        let rows: Vec<_> = (0..1159).map(|i| json!({"id":format!("route/model-{i}"),"provider_id":"9router","name":format!("Model {i}")})).collect();
        let models = catalogue(&json!({"models": rows}).to_string()).unwrap();
        assert_eq!(models.len(), 1159);
        let index = models.iter().position(|model| model.id == "route/model-1158").unwrap();
        assert_eq!(models[index].qualified(), "9router/route/model-1158");
        assert_eq!(visible_rows(&models, "9router/route/model-1158"), vec![index]);
    }
    fn models() -> Vec<Model> { vec![Model { id: "j-model".into(), name: "Jöhn 🚀".into(), provider_id: "openai".into(),context_window:None, free:false, cost:None, variants:Vec::new(), provider_name:None, provider_registry_name:None }, Model { id: "k-model".into(), name: "Kappa".into(), provider_id: "openai".into(),context_window:None, free:false, cost:None, variants:Vec::new(), provider_name:None, provider_registry_name:None }] }
    #[test] fn query_is_utf8_and_jk_are_search_text() {
        let m = models();
        assert_eq!(visible_rows(&m, "öhn"), vec![0]);
        assert_eq!(visible_rows(&m, "j"), vec![0]);
        assert_eq!(visible_rows(&m, "k"), vec![1]);
    }
    #[test]
    fn query_supports_fuzzy_subsequences_and_keeps_catalogue_order() {
        let m = models();
        assert_eq!(visible_rows(&m, "jh"), vec![0]);
        assert_eq!(visible_rows(&m, "opai"), vec![0, 1]);
        assert_eq!(visible_rows(&m, "kp"), vec![1]);
        assert_eq!(selected_row(&m, "kp", 0).unwrap().id, "k-model");
        assert_eq!(visible_rows(&m, "mo"), vec![0, 1]);
    }
    #[test] fn focused_row_clamps_and_no_result_cannot_select() {
        let m = models();
        assert_eq!(selected_row(&m, "k", 99).unwrap().id, "k-model");
        assert!(selected_row(&m, "no-match", 0).is_none());
        assert_eq!(move_row(99, 0, true), 0);
    }
    #[test]
    fn picker_bounds_reject_empty_short_and_outside_rows() {
        // These are the same list projections consumed by the provider/model
        // render paths: capacity zero paints no rows, while one paints exactly
        // the selected row without any capacity-1 arithmetic.
        let provider_zero = picker_viewport(0, 0, picker_capacity(0, 4, 2));
        let model_zero = picker_viewport(0, 3, picker_capacity(5, 4, 2));
        assert_eq!(provider_zero.len, 0);
        assert_eq!(model_zero.len, 0);
        assert_eq!(picker_viewport(0, 3, 1).len, 1);
        assert_eq!(picker_viewport(99, 3, 1).start, 2);
        assert_eq!(picker_render_indices(0, 3, 0), Vec::<usize>::new());
        assert_eq!(picker_render_indices(0, 3, 1), vec![0]);
        assert_eq!(picker_render_indices(99, 3, 1), vec![2]);
        assert_eq!(picker_render_indices(1, 3, 2), vec![0, 1]);
        assert_eq!(picker_capacity(0, 4, 2), 0);
        assert_eq!(picker_capacity(5, 4, 2), 0);
        assert_eq!(picker_capacity(12, 4, 2), 6);
        let viewport = picker_viewport(99, 3, 4);
        assert_eq!(picker_hit_row(viewport, 3, 4), None);
        assert_eq!(picker_hit_row(viewport, 4, 4), Some(0));
        assert_eq!(picker_hit_row(viewport, 6, 4), Some(2));
        assert_eq!(picker_hit_row(viewport, 7, 4), None);
    }

    #[test] fn exact_identity_not_variant_or_typed_fallback() {
        let m = models();
        assert_eq!(selected_row(&m, "j-model-extra", 0), None);
        assert_eq!(m[0].qualified(), "openai/j-model");
    }

    /// The OAuth attempt status feed (`AttemptStatus`,
    /// `crates/server/src/provider_integration.rs:59-66`) maps to a terminal
    /// flag plus the setup-surface notice. `pending` (and any unknown status)
    /// keeps polling; `complete`/`failed`/`expired` stop it.
    #[test] fn oauth_status_maps_terminal_states_and_keeps_pending_live() {
        assert_eq!(oauth_status(r#"{"status":"pending"}"#), (false, None));
        assert_eq!(oauth_status("{}"), (false, None));
        assert_eq!(
            oauth_status(r#"{"status":"complete"}"#),
            (true, Some("OpenAI connected".into()))
        );
        assert_eq!(
            oauth_status(r#"{"status":"expired"}"#),
            (true, Some("OpenAI sign-in expired; try again".into()))
        );
        assert_eq!(
            oauth_status(r#"{"status":"failed","message":"denied"}"#),
            (true, Some("OpenAI sign-in failed: denied".into()))
        );
        // A failed status without a message still surfaces a terminal notice.
        assert_eq!(
            oauth_status(r#"{"status":"failed"}"#),
            (true, Some("OpenAI sign-in failed: unknown error".into()))
        );
    }
}
