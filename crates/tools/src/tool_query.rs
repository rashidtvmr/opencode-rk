//! Native codebase query (TOOL-017): keyword search over a caller index.
//!
//! Pure library: no I/O, no network, no threads, no retained state, no
//! logging of index text, snippets, or the query string. Hits flow only
//! through the return value.

use thiserror::Error;

/// Marker template appended to the tail snippet when over budget.
/// `{dropped}` renders as the count of dropped hits (or cut bytes when a
/// single oversize snippet was cut but no hit was dropped).
pub const TRUNC_MARKER: &str = "\n... [qry:truncated {dropped} hits/bytes]";
/// Stable tail of every rendered marker.
pub const MARKER_SUFFIX: &str = "hits/bytes]";
/// Upper bound on any rendered marker's byte length.
pub const MARKER_MAX: usize = 128;

/// Path-hit bonus per token in the deterministic score.
const PATH_BONUS: usize = 10;

/// Budget + opt-out switch for [`query`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryConfig {
    pub enabled: bool,
    pub max_hits: usize,
    pub max_bytes: usize,
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_hits: 50,
            max_bytes: 65536,
        }
    }
}

impl QueryConfig {
    /// Opt-out config: [`query`] returns empty, untruncated, without traversal.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_hits: 50,
            max_bytes: 65536,
        }
    }
}

/// One caller-indexed line of source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedDoc {
    pub path: String,
    pub line: u32,
    pub text: String,
}

/// One ranked hit; `path` + `line` form the `file:line` citation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub path: String,
    pub line: u32,
    pub snippet: String,
}

/// Ranked result plus budget flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Result_ {
    pub hits: Vec<Hit>,
    pub truncated: bool,
}

/// Query failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueryError {
    #[error("query is empty")]
    EmptyQuery,
}

/// Keyword search over a caller-supplied local index.
///
/// Tokenizes `q` on whitespace (lowercased, empties dropped) and AND-matches
/// every token as a case-insensitive substring of `text` or `path`.
/// Ranks deterministically by `(score desc, path asc, line asc)` where
/// `score = sum(tokens of PATH_BONUS if token in path + occurrences in text)`.
/// Enforces `max_hits` then `max_bytes` (snippet byte sum); over budget sets
/// `truncated` and appends a rendered [`TRUNC_MARKER`] to the tail snippet.
pub fn query(
    index: &[IndexedDoc],
    q: &str,
    cfg: &QueryConfig,
) -> Result<Result_, QueryError> {
    if !cfg.enabled {
        return Ok(Result_ {
            hits: Vec::new(),
            truncated: false,
        });
    }
    let tokens: Vec<String> = q
        .split_whitespace()
        .map(|t| t.to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return Err(QueryError::EmptyQuery);
    }

    // Score + rank every AND-matching doc. Index order breaks full ties so
    // the same index + q + cfg is always byte-identical.
    let mut ranked: Vec<(usize, usize, Hit)> = Vec::new();
    for (pos, doc) in index.iter().enumerate() {
        let path_lc = doc.path.to_lowercase();
        let text_lc = doc.text.to_lowercase();
        let mut score = 0usize;
        let mut matched = true;
        for tok in &tokens {
            let in_path = path_lc.contains(tok.as_str());
            let count = text_lc.matches(tok.as_str()).count();
            if !in_path && count == 0 {
                matched = false;
                break;
            }
            if in_path {
                score += PATH_BONUS;
            }
            score += count;
        }
        if matched {
            ranked.push((
                score,
                pos,
                Hit {
                    path: doc.path.clone(),
                    line: doc.line,
                    snippet: doc.text.clone(),
                },
            ));
        }
    }
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.2.path.cmp(&b.2.path))
            .then_with(|| a.2.line.cmp(&b.2.line))
            .then_with(|| a.1.cmp(&b.1))
    });

    let matched_total = ranked.len();
    if matched_total > cfg.max_hits {
        ranked.truncate(cfg.max_hits);
    }

    // Byte budget over snippet bytes; prefix that fits wins.
    let mut hits: Vec<Hit> = Vec::new();
    let mut bytes = 0usize;
    for (_, _, hit) in &ranked {
        if bytes + hit.snippet.len() <= cfg.max_bytes {
            bytes += hit.snippet.len();
            hits.push(hit.clone());
        } else {
            break;
        }
    }
    let mut cut_bytes = 0usize;
    if hits.is_empty() && !ranked.is_empty() {
        // Oversize single snippet: cut at a char boundary, still cite it.
        let first = &ranked[0].2;
        let mut end = cfg.max_bytes.min(first.snippet.len());
        while end > 0 && !first.snippet.is_char_boundary(end) {
            end -= 1;
        }
        cut_bytes = first.snippet.len() - end;
        hits.push(Hit {
            path: first.path.clone(),
            line: first.line,
            snippet: first.snippet[..end].to_string(),
        });
    }

    // Dropped relative to every matcher (max_hits cut + byte-budget cut).
    let dropped_hits = matched_total.saturating_sub(hits.len());
    let truncated = matched_total > hits.len() || cut_bytes > 0;
    if truncated {
        if let Some(tail) = hits.last_mut() {
            let n = if dropped_hits > 0 {
                dropped_hits
            } else {
                cut_bytes
            };
            let marker = render_marker(n);
            tail.snippet.push_str(&marker);
        }
    }

    Ok(Result_ { hits, truncated })
}

/// Render the truncation marker for `dropped` units; never exceeds MARKER_MAX.
fn render_marker(dropped: usize) -> String {
    let mut m = TRUNC_MARKER.replace("{dropped}", &dropped.to_string());
    if m.len() > MARKER_MAX {
        let mut end = MARKER_MAX;
        while end > 0 && !m.is_char_boundary(end) {
            end -= 1;
        }
        m.truncate(end);
    }
    debug_assert!(m.len() <= MARKER_MAX);
    m
}
