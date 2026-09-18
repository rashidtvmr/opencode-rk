//! /CONTEXT and /MEMORY snapshot service (roadmap 3.3+3.4).
//!
//! Builds bounded token-usage snapshots ([`ContextReport`]) from turn state
//! and memory-file classification reports ([`MemoryReport`]) from a
//! [`RulesSnapshot`]-shaped input. Pure-state, no I/O, no async, std-only.
//!
//! Token estimation uses the documented `bytes / 4` heuristic applied
//! consistently across all segment kinds.
//!
//! # In-file tests
//!
//! T01 – segment split sums to total_tokens
//! T02 – bounded segments (at most MAX_SEGMENTS)
//! T03 – over-limit flag + headroom preserved
//! T04 – memory loaded/skipped classification incl. reasons
//! T05 – deterministic order
//! T06 – estimator documented (bytes/4) applied consistently

#![forbid(unsafe_code)]

use std::path::PathBuf;

/// Maximum segments in a [`ContextReport`]. When more segments exist than
/// this limit, the report is truncated and `truncated` is set to `true`.
pub const MAX_SEGMENTS: usize = 16;

/// Token estimator: one token ≈ 4 bytes of UTF-8 text.
///
/// This is a well-known heuristic for English text; exact counts depend on
/// the tokenizer but this keeps the module self-contained and deterministic.
#[must_use]
pub fn estimate_tokens(byte_len: usize) -> u64 {
    (byte_len / 4) as u64
}

// ---------------------------------------------------------------------------
// ContextReport
// ---------------------------------------------------------------------------

/// The kind of context segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SegmentKind {
    System,
    Rules,
    History,
    ToolResults,
    CurrentTurn,
}

/// A single token-counted segment of the context window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub kind: SegmentKind,
    pub tokens: u64,
    pub bytes: usize,
}

/// Bounded snapshot of context-window token usage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextReport {
    pub total_tokens: u64,
    pub model_limit: u64,
    pub segments: Vec<Segment>,
    /// True when the source had more segments than [`MAX_SEGMENTS`].
    pub truncated: bool,
    /// Tokens remaining below the model limit after all included segments.
    pub headroom: u64,
}

/// Minimal turn-state input for [`build_context_report`].
///
/// Every field is optional (absent → zero). The caller supplies raw byte
/// lengths; tokens are estimated internally.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TurnStateInput {
    pub system_prompt_bytes: usize,
    pub rules_bytes: usize,
    pub history_entries: Vec<HistoryEntry>,
    pub tool_result_bytes: Vec<usize>,
    pub current_turn_bytes: usize,
    pub model_limit: u64,
}

/// One history entry for token estimation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryEntry {
    pub bytes: usize,
}

/// Failures for [`build_context_report`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextError {
    /// Model limit is zero (cannot compute headroom).
    ZeroModelLimit,
}

impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroModelLimit => write!(f, "model_limit must be greater than zero"),
        }
    }
}

impl std::error::Error for ContextError {}

/// Build a bounded [`ContextReport`] from a [`TurnStateInput`].
///
/// Segments are emitted in a stable order: System, Rules, History (one
/// segment per entry), ToolResults (one per entry), CurrentTurn. When more
/// than [`MAX_SEGMENTS`] would be emitted, later segments are dropped and
/// `truncated` is set to `true`.
///
/// # Errors
///
/// Returns [`ContextError::ZeroModelLimit`] if `model_limit` is zero.
pub fn build_context_report(input: &TurnStateInput) -> Result<ContextReport, ContextError> {
    if input.model_limit == 0 {
        return Err(ContextError::ZeroModelLimit);
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

    // 3. History (one segment per entry)
    for entry in &input.history_entries {
        segments.push(Segment {
            kind: SegmentKind::History,
            tokens: estimate_tokens(entry.bytes),
            bytes: entry.bytes,
        });
    }

    // 4. Tool results (one per entry)
    for &bytes in &input.tool_result_bytes {
        if bytes > 0 {
            segments.push(Segment {
                kind: SegmentKind::ToolResults,
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

    let total_segments = segments.len();
    let truncated = total_segments > MAX_SEGMENTS;
    if truncated {
        segments.truncate(MAX_SEGMENTS);
    }

    let total_tokens: u64 = segments.iter().map(|s| s.tokens).sum();
    let headroom = model_limit.saturating_sub(total_tokens);

    Ok(ContextReport {
        total_tokens,
        model_limit,
        segments,
        truncated,
        headroom,
    })
}

// ---------------------------------------------------------------------------
// MemoryReport
// ---------------------------------------------------------------------------

/// A successfully loaded memory file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryFile {
    pub path: PathBuf,
    pub glob: Option<String>,
    pub bytes: usize,
}

/// Reason a memory file was skipped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkipReason {
    /// File exceeds the per-file byte budget.
    OverBudget { limit: usize, actual: usize },
    /// Symlink or path-traversal escape detected.
    PathEscape,
    /// IO error reading the file.
    Io(String),
}

/// A skipped memory file with reason.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkippedFile {
    pub path: PathBuf,
    pub reason: SkipReason,
}

/// Report of loaded and skipped memory files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryReport {
    pub loaded: Vec<MemoryFile>,
    pub skipped: Vec<SkippedFile>,
}

/// Minimal rule-entry input for [`build_memory_report`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRuleEntry {
    pub path: PathBuf,
    pub glob: Option<String>,
    pub bytes: Vec<u8>,
}

/// Failures for [`build_memory_report`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryError {
    /// Per-file byte budget is zero (nothing could ever load).
    ZeroBudget,
    /// Total loaded entries exceed the budget (all remaining skipped).
    BudgetExceeded { loaded: usize, budget: usize },
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroBudget => write!(f, "per_file_budget must be greater than zero"),
            Self::BudgetExceeded { loaded, budget } => {
                write!(f, "total loaded bytes {loaded} exceeds budget {budget}")
            }
        }
    }
}

impl std::error::Error for MemoryError {}

/// Build a [`MemoryReport`] from a slice of [`MemoryRuleEntry`].
///
/// Files whose byte content exceeds `per_file_budget` are classified as
/// skipped with [`SkipReason::OverBudget`]. Loaded files are emitted in
/// input order (deterministic). The total loaded bytes must not exceed
/// `total_budget`.
///
/// # Errors
///
/// Returns [`MemoryError::ZeroBudget`] if `per_file_budget` is zero.
/// Returns [`MemoryError::BudgetExceeded`] if loading would exceed
/// `total_budget`.
pub fn build_memory_report(
    entries: &[MemoryRuleEntry],
    per_file_budget: usize,
    total_budget: usize,
) -> Result<MemoryReport, MemoryError> {
    if per_file_budget == 0 {
        return Err(MemoryError::ZeroBudget);
    }

    let mut loaded: Vec<MemoryFile> = Vec::new();
    let mut skipped: Vec<SkippedFile> = Vec::new();
    let mut total_loaded_bytes: usize = 0;

    for entry in entries {
        if entry.bytes.len() > per_file_budget {
            skipped.push(SkippedFile {
                path: entry.path.clone(),
                reason: SkipReason::OverBudget {
                    limit: per_file_budget,
                    actual: entry.bytes.len(),
                },
            });
            continue;
        }

        if total_loaded_bytes + entry.bytes.len() > total_budget {
            skipped.push(SkippedFile {
                path: entry.path.clone(),
                reason: SkipReason::OverBudget {
                    limit: total_budget - total_loaded_bytes,
                    actual: entry.bytes.len(),
                },
            });
            return Err(MemoryError::BudgetExceeded {
                loaded: total_loaded_bytes + entry.bytes.len(),
                budget: total_budget,
            });
        }

        total_loaded_bytes += entry.bytes.len();
        loaded.push(MemoryFile {
            path: entry.path.clone(),
            glob: entry.glob.clone(),
            bytes: entry.bytes.len(),
        });
    }

    Ok(MemoryReport { loaded, skipped })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // T01: segment split sums to total_tokens
    #[test]
    fn t01_segment_split_sums_to_total() {
        let input = TurnStateInput {
            system_prompt_bytes: 400,  // 100 tokens
            rules_bytes: 200,          // 50 tokens
            history_entries: vec![
                HistoryEntry { bytes: 80 },  // 20
                HistoryEntry { bytes: 120 }, // 30
            ],
            tool_result_bytes: vec![160, 80], // 40 + 20
            current_turn_bytes: 40,            // 10
            model_limit: 1000,
        };
        let report = build_context_report(&input).unwrap();
        let sum: u64 = report.segments.iter().map(|s| s.tokens).sum();
        assert_eq!(sum, report.total_tokens);
        assert_eq!(report.total_tokens, 270); // 100+50+20+30+40+20+10
    }

    // T02: bounded segments (at most MAX_SEGMENTS)
    #[test]
    fn t02_bounded_segments() {
        let mut input = TurnStateInput {
            model_limit: 10_000,
            ..Default::default()
        };
        // 20 history entries → 20 History segments + 0 others = 20 > MAX_SEGMENTS
        for _ in 0..20 {
            input.history_entries.push(HistoryEntry { bytes: 40 });
        }
        let report = build_context_report(&input).unwrap();
        assert!(report.segments.len() <= MAX_SEGMENTS);
        assert!(report.truncated);
    }

    // T03: over-limit flag + headroom
    #[test]
    fn t03_over_limit_flag_and_headroom() {
        // Total tokens = 250, model_limit = 200 → headroom = 0 (saturating)
        let input = TurnStateInput {
            system_prompt_bytes: 1000, // 250 tokens
            model_limit: 200,
            ..Default::default()
        };
        let report = build_context_report(&input).unwrap();
        assert_eq!(report.total_tokens, 250);
        assert_eq!(report.headroom, 0); // saturating_sub
        assert!(!report.truncated);

        // Normal case: headroom positive
        let input2 = TurnStateInput {
            system_prompt_bytes: 400, // 100 tokens
            model_limit: 1000,
            ..Default::default()
        };
        let report2 = build_context_report(&input2).unwrap();
        assert_eq!(report2.headroom, 900);
    }

    // T04: memory loaded/skipped classification incl. reasons
    #[test]
    fn t04_memory_loaded_skipped_classification() {
        let entries = vec![
            MemoryRuleEntry {
                path: PathBuf::from("AGENTS.md"),
                glob: None,
                bytes: b"hello".to_vec(),
            },
            MemoryRuleEntry {
                path: PathBuf::from("rules/big.md"),
                glob: Some("src/**".to_string()),
                bytes: vec![0u8; 2000], // over per_file_budget
            },
        ];
        let report = build_memory_report(&entries, 512, 4096).unwrap();
        assert_eq!(report.loaded.len(), 1);
        assert_eq!(report.loaded[0].path, PathBuf::from("AGENTS.md"));
        assert_eq!(report.loaded[0].glob, None);
        assert_eq!(report.loaded[0].bytes, 5);

        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].path, PathBuf::from("rules/big.md"));
        assert!(matches!(
            report.skipped[0].reason,
            SkipReason::OverBudget { .. }
        ));
    }

    // T05: deterministic order
    #[test]
    fn t05_deterministic_order() {
        let input = TurnStateInput {
            system_prompt_bytes: 400,
            rules_bytes: 200,
            history_entries: vec![
                HistoryEntry { bytes: 80 },
                HistoryEntry { bytes: 120 },
            ],
            tool_result_bytes: vec![160],
            current_turn_bytes: 40,
            model_limit: 5000,
        };
        let r1 = build_context_report(&input).unwrap();
        let r2 = build_context_report(&input).unwrap();
        assert_eq!(r1.segments.len(), r2.segments.len());
        for (a, b) in r1.segments.iter().zip(r2.segments.iter()) {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.tokens, b.tokens);
            assert_eq!(a.bytes, b.bytes);
        }
        // Verify stable order: System, Rules, History×2, ToolResults, CurrentTurn
        assert_eq!(r1.segments[0].kind, SegmentKind::System);
        assert_eq!(r1.segments[1].kind, SegmentKind::Rules);
        assert_eq!(r1.segments[2].kind, SegmentKind::History);
        assert_eq!(r1.segments[3].kind, SegmentKind::History);
        assert_eq!(r1.segments[4].kind, SegmentKind::ToolResults);
        assert_eq!(r1.segments[5].kind, SegmentKind::CurrentTurn);
    }

    // T06: estimator documented (bytes/4) applied consistently
    #[test]
    fn t06_estimator_bytes_per_4_consistent() {
        // Direct check: estimate_tokens matches bytes/4
        assert_eq!(estimate_tokens(0), 0);
        assert_eq!(estimate_tokens(1), 0);
        assert_eq!(estimate_tokens(3), 0);
        assert_eq!(estimate_tokens(4), 1);
        assert_eq!(estimate_tokens(7), 1);
        assert_eq!(estimate_tokens(8), 2);
        assert_eq!(estimate_tokens(100), 25);
        assert_eq!(estimate_tokens(101), 25);

        // Consistent across segment kinds
        let input = TurnStateInput {
            system_prompt_bytes: 12,   // 3 tokens
            rules_bytes: 16,           // 4 tokens
            history_entries: vec![HistoryEntry { bytes: 4 }],
            tool_result_bytes: vec![8],
            current_turn_bytes: 20,     // 5 tokens
            model_limit: 100,
        };
        let report = build_context_report(&input).unwrap();
        assert_eq!(report.segments[0].tokens, 3); // System: 12/4
        assert_eq!(report.segments[1].tokens, 4); // Rules: 16/4
        assert_eq!(report.segments[2].tokens, 1); // History: 4/4
        assert_eq!(report.segments[3].tokens, 2); // ToolResults: 8/4
        assert_eq!(report.segments[4].tokens, 5); // CurrentTurn: 20/4
    }

    // Edge: empty input
    #[test]
    fn edge_empty_input() {
        let input = TurnStateInput {
            model_limit: 1000,
            ..Default::default()
        };
        let report = build_context_report(&input).unwrap();
        assert_eq!(report.total_tokens, 0);
        assert_eq!(report.segments.len(), 0);
        assert!(!report.truncated);
        assert_eq!(report.headroom, 1000);
    }

    // Edge: zero model limit
    #[test]
    fn edge_zero_model_limit() {
        let input = TurnStateInput {
            model_limit: 0,
            ..Default::default()
        };
        assert_eq!(
            build_context_report(&input),
            Err(ContextError::ZeroModelLimit)
        );
    }

    // Edge: memory zero budget
    #[test]
    fn edge_memory_zero_budget() {
        assert_eq!(
            build_memory_report(&[], 0, 1024),
            Err(MemoryError::ZeroBudget)
        );
    }

    // Edge: memory total budget exceeded
    #[test]
    fn edge_memory_budget_exceeded() {
        let entries = vec![
            MemoryRuleEntry {
                path: PathBuf::from("a.md"),
                glob: None,
                bytes: vec![0u8; 600],
            },
            MemoryRuleEntry {
                path: PathBuf::from("b.md"),
                glob: None,
                bytes: vec![0u8; 600],
            },
        ];
        assert!(matches!(
            build_memory_report(&entries, 1024, 1000),
            Err(MemoryError::BudgetExceeded { .. })
        ));
    }

    // Edge: tool results with zero bytes are skipped
    #[test]
    fn edge_zero_tool_results_skipped() {
        let input = TurnStateInput {
            tool_result_bytes: vec![0, 4, 0],
            model_limit: 1000,
            ..Default::default()
        };
        let report = build_context_report(&input).unwrap();
        // Only the non-zero tool result should appear
        assert_eq!(report.segments.len(), 1);
        assert_eq!(report.segments[0].kind, SegmentKind::ToolResults);
        assert_eq!(report.segments[0].bytes, 4);
    }
}
