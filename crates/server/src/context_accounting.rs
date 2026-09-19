//! LANE-CONTEXT-ACCOUNT: bounded context accounting service (roadmap 3.3).
//!
//! Given the typed pieces of what a provider round carries (system prompt
//! bytes, rules segment, history message bytes, tool schemas, current turn),
//! produces a per-segment token estimate (documented chars/4 estimator, no
//! tokenizer dep), total vs a model context limit, compaction headroom flag
//! with threshold, and a bounded snapshot struct (the /CONTEXT render source).
//!
//! Deterministic, cap on history pieces counted (documented), saturating math
//! (no overflow). Standalone std-only module.

#![forbid(unsafe_code)]

/// Maximum history message pieces counted. When more exist, later pieces
/// are dropped and [`ContextAccountReport::truncated`] is set.
pub const MAX_HISTORY_PIECES: usize = 256;

/// Token estimator: one token ≈ 4 bytes of UTF-8 text.
///
/// This is a well-known heuristic for English text; exact counts depend on
/// the tokenizer but this keeps the module self-contained and deterministic.
#[must_use]
pub fn estimate_tokens(byte_len: usize) -> u64 {
    (byte_len / 4) as u64
}

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// One history message piece for token estimation.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct HistoryPiece {
    pub bytes: usize,
}

/// Minimal turn-state input for [`build_account_report`].
///
/// Every field is optional (absent → zero). The caller supplies raw byte
/// lengths; tokens are estimated internally.
#[derive(Clone, Debug, Default)]
pub struct AccountInput {
    pub system_prompt_bytes: usize,
    pub rules_bytes: usize,
    pub history_messages: Vec<HistoryPiece>,
    pub tool_schemas_bytes: Vec<usize>,
    pub current_turn_bytes: usize,
    pub model_limit: u64,
    pub compaction_threshold: f64,
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// The kind of context segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SegmentKind {
    System,
    Rules,
    History,
    ToolSchemas,
    CurrentTurn,
}

/// A single token-counted segment of the context window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub kind: SegmentKind,
    pub tokens: u64,
    pub bytes: usize,
}

/// Failures for [`build_account_report`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountError {
    /// Model limit is zero (cannot compute headroom).
    ZeroModelLimit,
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroModelLimit => write!(f, "model_limit must be greater than zero"),
        }
    }
}

impl std::error::Error for AccountError {}

/// Bounded snapshot of context-window token usage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextAccountReport {
    pub total_tokens: u64,
    pub model_limit: u64,
    pub segments: Vec<Segment>,
    /// True when the source had more history pieces than [`MAX_HISTORY_PIECES`].
    pub truncated: bool,
    /// Tokens remaining below the model limit after all included segments.
    pub headroom: u64,
    /// True when token usage exceeds [`AccountInput::compaction_threshold`]
    /// of the model limit, signalling the UI should offer compaction.
    pub needs_compaction: bool,
}

/// Serializable snapshot for /CONTEXT render.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContextAccountSnapshot {
    pub total_tokens: u64,
    pub model_limit: u64,
    pub segments: Vec<SnapshotSegment>,
    pub truncated: bool,
    pub headroom: u64,
    pub needs_compaction: bool,
}

/// Snapshot segment (string kind for JSON).
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotSegment {
    pub kind: String,
    pub tokens: u64,
    pub bytes: usize,
}

impl ContextAccountReport {
    /// Produce a serializable snapshot.
    pub fn snapshot(&self) -> String {
        let snap = ContextAccountSnapshot {
            total_tokens: self.total_tokens,
            model_limit: self.model_limit,
            segments: self
                .segments
                .iter()
                .map(|s| SnapshotSegment {
                    kind: format!("{:?}", s.kind),
                    tokens: s.tokens,
                    bytes: s.bytes,
                })
                .collect(),
            truncated: self.truncated,
            headroom: self.headroom,
            needs_compaction: self.needs_compaction,
        };
        serde_json::to_string(&snap).expect("snapshot serialization")
    }
}

impl ContextAccountSnapshot {
    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Result<Self, AccountError> {
        serde_json::from_str(json).map_err(|_| AccountError::ZeroModelLimit)
    }
}

/// Build a bounded [`ContextAccountReport`] from an [`AccountInput`].
///
/// Segments are emitted in stable order: System, Rules, History (one segment
/// per piece, capped at [`MAX_HISTORY_PIECES`]), ToolSchemas (one per entry),
/// CurrentTurn. When more than [`MAX_HISTORY_PIECES`] exist, later pieces are
/// dropped and `truncated` is set.
///
/// Token estimation uses the documented `bytes / 4` heuristic.
///
/// # Errors
///
/// Returns [`AccountError::ZeroModelLimit`] if `model_limit` is zero.
pub fn build_account_report(input: &AccountInput) -> Result<ContextAccountReport, AccountError> {
    if input.model_limit == 0 {
        return Err(AccountError::ZeroModelLimit);
    }

    let model_limit = input.model_limit;
    let mut segments: Vec<Segment> = Vec::new();

    // 1. System
    if input.system_prompt_bytes > 0 {
        segments.push(Segment {
            kind: SegmentKind::System,
            tokens: estimate_tokens(input.system_prompt_bytes),
            bytes: input.system_prompt_bytes,
        });
    }

    // 2. Rules
    if input.rules_bytes > 0 {
        segments.push(Segment {
            kind: SegmentKind::Rules,
            tokens: estimate_tokens(input.rules_bytes),
            bytes: input.rules_bytes,
        });
    }

    // 3. History (capped at MAX_HISTORY_PIECES)
    let history_count = input.history_messages.len().min(MAX_HISTORY_PIECES);
    let truncated = input.history_messages.len() > MAX_HISTORY_PIECES;
    for piece in &input.history_messages[..history_count] {
        segments.push(Segment {
            kind: SegmentKind::History,
            tokens: estimate_tokens(piece.bytes),
            bytes: piece.bytes,
        });
    }

    // 4. Tool schemas (one per entry, zero-byte entries skipped)
    for &bytes in &input.tool_schemas_bytes {
        if bytes > 0 {
            segments.push(Segment {
                kind: SegmentKind::ToolSchemas,
                tokens: estimate_tokens(bytes),
                bytes,
            });
        }
    }

    // 5. CurrentTurn
    if input.current_turn_bytes > 0 {
        segments.push(Segment {
            kind: SegmentKind::CurrentTurn,
            tokens: estimate_tokens(input.current_turn_bytes),
            bytes: input.current_turn_bytes,
        });
    }

    let total_tokens: u64 = segments
        .iter()
        .fold(0u64, |acc, s| acc.saturating_add(s.tokens));
    let headroom = model_limit.saturating_sub(total_tokens);

    // Compaction threshold: needs_compaction when total/limit > threshold
    let usage_ratio = if model_limit > 0 {
        total_tokens as f64 / model_limit as f64
    } else {
        1.0
    };
    let needs_compaction = usage_ratio > input.compaction_threshold;

    Ok(ContextAccountReport {
        total_tokens,
        model_limit,
        segments,
        truncated,
        headroom,
        needs_compaction,
    })
}
