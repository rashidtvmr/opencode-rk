//! Bounded, local MCP catalog parsing and search.
//!
//! This module only works on caller-supplied bytes and entries. It does not
//! read files, contact a network, start an installer, or retain catalog state.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum serialized catalog size accepted by [`load_index`].
pub const MAX_INDEX_BYTES: usize = 256 * 1024;
/// Maximum number of entries accepted by [`load_index`] and [`search`].
pub const MAX_INDEX_ENTRIES: usize = 512;
/// Maximum query text length, measured in Unicode scalar values.
pub const MAX_QUERY_CHARS: usize = 256;
/// Hard upper bound for returned hits.
pub const MAX_RESULTS: usize = 50;
/// Default hit count for [`CatalogQuery`].
pub const DEFAULT_MAX_RESULTS: usize = 20;
/// Maximum description bytes retained in a result snippet.
pub const MAX_SNIPPET_BYTES: usize = 256;

/// One display-only MCP catalog entry.
///
/// Deliberately excludes commands, environment values, credentials, and
/// executable installation payloads. Installation, if any, belongs to a
/// separate permission-brokered operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub provider: String,
    pub source: String,
}

/// Search criteria for the local MCP catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogQuery {
    pub text: String,
    pub tag: Option<String>,
    pub provider: Option<String>,
    pub max_results: usize,
}

impl Default for CatalogQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            tag: None,
            provider: None,
            max_results: DEFAULT_MAX_RESULTS,
        }
    }
}

/// Display metadata returned for one catalog match.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CatalogHit {
    pub name: String,
    pub provider: String,
    pub source: String,
    pub snippet: String,
}

/// Search hits in deterministic rank order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SearchResult {
    pub hits: Vec<CatalogHit>,
    pub truncated: bool,
}

/// Catalog validation and query failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum CatalogError {
    #[error("bad MCP catalog index")]
    BadIndex,
    #[error("MCP catalog query is empty")]
    EmptyQuery,
    #[error("MCP catalog exceeds a resource cap")]
    OverCap,
}

/// Parse a versioned catalog from caller-owned JSON bytes.
///
/// The byte cap is checked before deserialization. Invalid, incomplete, or
/// unsupported-version indexes fail closed; no partial entries are returned.
/// The returned vector is the only retained catalog representation.
pub fn load_index(bytes: &[u8]) -> Result<Vec<CatalogEntry>, CatalogError> {
    if bytes.len() > MAX_INDEX_BYTES {
        return Err(CatalogError::OverCap);
    }

    let raw: RawCatalog = serde_json::from_slice(bytes).map_err(|_| CatalogError::BadIndex)?;
    if raw.version != "1" || !valid_date(&raw.updated) {
        return Err(CatalogError::BadIndex);
    }
    if raw.entries.len() > MAX_INDEX_ENTRIES {
        return Err(CatalogError::OverCap);
    }
    if raw.entries.iter().any(|entry| !valid_entry(entry)) {
        return Err(CatalogError::BadIndex);
    }

    Ok(raw.entries)
}

/// Search a caller-supplied catalog without I/O or external effects.
///
/// Text terms are case-insensitive substring matches and are ANDed. Tag and
/// provider filters are case-insensitive exact filters. Ranking favors name
/// matches, then provider, tags, and description; ties use name, provider,
/// source, and original position for stable deterministic output.
pub fn search(index: &[CatalogEntry], query: &CatalogQuery) -> Result<SearchResult, CatalogError> {
    let text = query.text.trim();
    let tag = active_filter(query.tag.as_deref());
    let provider = active_filter(query.provider.as_deref());

    if query.text.chars().count() > MAX_QUERY_CHARS {
        return Err(CatalogError::OverCap);
    }
    if text.is_empty() && tag.is_none() && provider.is_none() {
        return Err(CatalogError::EmptyQuery);
    }
    if index.len() > MAX_INDEX_ENTRIES {
        return Err(CatalogError::OverCap);
    }
    if index.iter().any(|entry| !valid_entry(entry)) {
        return Err(CatalogError::BadIndex);
    }

    if !(1..=MAX_RESULTS).contains(&query.max_results) {
        return Err(CatalogError::OverCap);
    }
    let limit = query.max_results;
    let terms: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    let tag_lower = tag.map(str::to_lowercase);
    let provider_lower = provider.map(str::to_lowercase);

    let mut ranked = Vec::with_capacity(limit);
    let mut matched = 0usize;
    for (position, entry) in index.iter().enumerate() {
        if let Some(filter) = tag_lower.as_deref() {
            if !entry
                .tags
                .iter()
                .any(|candidate| candidate.to_lowercase() == filter)
            {
                continue;
            }
        }
        if let Some(filter) = provider_lower.as_deref() {
            if entry.provider.to_lowercase() != filter {
                continue;
            }
        }

        let name = entry.name.to_lowercase();
        let description = entry.description.to_lowercase();
        let provider_text = entry.provider.to_lowercase();
        let tags = entry
            .tags
            .iter()
            .map(|value| value.to_lowercase())
            .collect::<Vec<_>>();

        let score = if terms.is_empty() {
            0
        } else {
            let mut score = 0usize;
            let mut all_match = true;
            for term in &terms {
                let name_match = name.contains(term);
                let provider_match = provider_text.contains(term);
                let tag_match = tags.iter().any(|value| value.contains(term));
                let description_match = description.contains(term);
                if !name_match && !provider_match && !tag_match && !description_match {
                    all_match = false;
                    break;
                }
                score = score.saturating_add(field_score(
                    &name,
                    &description,
                    &provider_text,
                    &tags,
                    term,
                ));
            }
            if !all_match {
                continue;
            }
            score
        };

        matched = matched.saturating_add(1);
        let candidate = RankedHit {
            score,
            position,
            hit: CatalogHit {
                name: entry.name.clone(),
                provider: entry.provider.clone(),
                source: entry.source.clone(),
                snippet: snippet(&entry.description),
            },
        };
        ranked.push(candidate);
        ranked.sort_by(compare_ranked);
        if ranked.len() > limit {
            ranked.pop();
        }
    }

    let hits = ranked.into_iter().map(|item| item.hit).collect();
    Ok(SearchResult {
        hits,
        truncated: matched > limit,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    version: String,
    updated: String,
    entries: Vec<CatalogEntry>,
}

#[derive(Debug)]
struct RankedHit {
    score: usize,
    position: usize,
    hit: CatalogHit,
}

fn active_filter(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..].iter().all(u8::is_ascii_digit)
}

fn valid_entry(entry: &CatalogEntry) -> bool {
    !entry.name.trim().is_empty()
        && !entry.description.trim().is_empty()
        && !entry.provider.trim().is_empty()
        && !entry.source.trim().is_empty()
        && entry.tags.iter().all(|tag| !tag.trim().is_empty())
}

fn field_score(
    name: &str,
    description: &str,
    provider: &str,
    tags: &[String],
    term: &str,
) -> usize {
    let mut score = 0usize;
    if name == term {
        score = score.saturating_add(1_000);
    } else if name.starts_with(term) {
        score = score.saturating_add(800);
    } else if name.contains(term) {
        score = score.saturating_add(600);
    }
    if provider.contains(term) {
        score = score.saturating_add(400);
    }
    if tags.iter().any(|value| value.contains(term)) {
        score = score.saturating_add(300);
    }
    if description.contains(term) {
        score = score.saturating_add(200);
    }
    score.saturating_add(name.matches(term).count())
}

fn compare_ranked(left: &RankedHit, right: &RankedHit) -> std::cmp::Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| left.hit.name.cmp(&right.hit.name))
        .then_with(|| left.hit.provider.cmp(&right.hit.provider))
        .then_with(|| left.hit.source.cmp(&right.hit.source))
        .then_with(|| left.position.cmp(&right.position))
}

fn snippet(description: &str) -> String {
    if description.len() <= MAX_SNIPPET_BYTES {
        return description.to_owned();
    }
    let mut end = MAX_SNIPPET_BYTES;
    while end > 0 && !description.is_char_boundary(end) {
        end -= 1;
    }
    description[..end].to_owned()
}
