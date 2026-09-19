//! LANE-CONTEXT-ACCOUNT: TDD tests for bounded context accounting service.
//!
//! Standalone test harness — imports context_accounting module directly.
//! Tests frozen before implementation; assertion edits forbidden.

#[path = "../src/context_accounting.rs"]
mod context_accounting;

use context_accounting::*;

// T01: per-segment token split sums to total_tokens
#[test]
fn t01_segment_split_sums_to_total() {
    let input = AccountInput {
        system_prompt_bytes: 400,  // 100 tokens
        rules_bytes: 200,          // 50 tokens
        history_messages: vec![
            HistoryPiece { bytes: 80 },   // 20 tokens
            HistoryPiece { bytes: 120 },  // 30 tokens
        ],
        tool_schemas_bytes: vec![160, 80], // 40 + 20 = 60
        current_turn_bytes: 40,            // 10 tokens
        model_limit: 1000,
        compaction_threshold: 0.8,
    };
    let report = build_account_report(&input).unwrap();
    let sum: u64 = report.segments.iter().map(|s| s.tokens).sum();
    assert_eq!(sum, report.total_tokens);
    assert_eq!(report.total_tokens, 270); // 100+50+20+30+40+20+10
}

// T02: estimator documented constant (bytes/4) applied consistently
#[test]
fn t02_estimator_bytes_per_4_consistent() {
    assert_eq!(estimate_tokens(0), 0);
    assert_eq!(estimate_tokens(1), 0);
    assert_eq!(estimate_tokens(3), 0);
    assert_eq!(estimate_tokens(4), 1);
    assert_eq!(estimate_tokens(7), 1);
    assert_eq!(estimate_tokens(8), 2);
    assert_eq!(estimate_tokens(100), 25);
    assert_eq!(estimate_tokens(101), 25);

    let input = AccountInput {
        system_prompt_bytes: 12,   // 3 tokens
        rules_bytes: 16,           // 4 tokens
        history_messages: vec![HistoryPiece { bytes: 4 }],
        tool_schemas_bytes: vec![8],
        current_turn_bytes: 20,    // 5 tokens
        model_limit: 100,
        compaction_threshold: 0.8,
    };
    let report = build_account_report(&input).unwrap();
    assert_eq!(report.segments[0].tokens, 3); // System: 12/4
    assert_eq!(report.segments[1].tokens, 4); // Rules: 16/4
    assert_eq!(report.segments[2].tokens, 1); // History: 4/4
    assert_eq!(report.segments[3].tokens, 2); // ToolSchemas: 8/4
    assert_eq!(report.segments[4].tokens, 5); // CurrentTurn: 20/4
}

// T03: headroom threshold boundary — compaction flag flips at threshold
#[test]
fn t03_headroom_threshold_boundary() {
    // total=100, limit=125, threshold=0.8 → usage_ratio=0.8 → NOT > 0.8 → no compact
    let input_at = AccountInput {
        system_prompt_bytes: 400, // 100 tokens
        model_limit: 125,
        compaction_threshold: 0.8,
        ..Default::default()
    };
    let report_at = build_account_report(&input_at).unwrap();
    assert!(!report_at.needs_compaction);

    // total=101 (system=404 bytes→101 tokens), limit=125, threshold=0.8 → ratio=0.808 > 0.8 → compact
    let input_above = AccountInput {
        system_prompt_bytes: 404, // 101 tokens
        model_limit: 125,
        compaction_threshold: 0.8,
        ..Default::default()
    };
    let report_above = build_account_report(&input_above).unwrap();
    assert!(report_above.needs_compaction);

    // total=10, limit=125 → ratio=0.08 < 0.8 → no compact
    let input_safe = AccountInput {
        system_prompt_bytes: 40, // 10 tokens
        model_limit: 125,
        compaction_threshold: 0.8,
        ..Default::default()
    };
    let report_safe = build_account_report(&input_safe).unwrap();
    assert!(!report_safe.needs_compaction);
}

// T04: saturation on huge input — no overflow on usize::MAX
#[test]
fn t04_saturation_on_huge_input() {
    let input = AccountInput {
        system_prompt_bytes: usize::MAX,
        rules_bytes: usize::MAX,
        history_messages: vec![HistoryPiece { bytes: usize::MAX }],
        tool_schemas_bytes: vec![usize::MAX],
        current_turn_bytes: usize::MAX,
        model_limit: u64::MAX,
        compaction_threshold: 0.8,
    };
    // Must not panic — saturating arithmetic throughout
    let report = build_account_report(&input).unwrap();
    assert_eq!(report.total_tokens, u64::MAX);
    assert_eq!(report.headroom, 0);
    assert!(report.needs_compaction);
}

// T05: cap behavior — history pieces beyond MAX_HISTORY_PIECES are dropped
#[test]
fn t05_cap_behavior_history_limit() {
    let mut input = AccountInput {
        model_limit: 100_000,
        compaction_threshold: 0.8,
        ..Default::default()
    };
    // MAX_HISTORY_PIECES + 10 extra pieces
    for _ in 0..(MAX_HISTORY_PIECES + 10) {
        input.history_messages.push(HistoryPiece { bytes: 40 }); // 10 tokens each
    }
    let report = build_account_report(&input).unwrap();
    // Only MAX_HISTORY_PIECES history segments
    let history_count = report
        .segments
        .iter()
        .filter(|s| s.kind == SegmentKind::History)
        .count();
    assert_eq!(history_count, MAX_HISTORY_PIECES);
    assert!(report.truncated);
}

// T06: deterministic — same input produces identical report
#[test]
fn t06_deterministic_output() {
    let input = AccountInput {
        system_prompt_bytes: 400,
        rules_bytes: 200,
        history_messages: vec![HistoryPiece { bytes: 80 }, HistoryPiece { bytes: 120 }],
        tool_schemas_bytes: vec![160],
        current_turn_bytes: 40,
        model_limit: 5000,
        compaction_threshold: 0.8,
    };
    let r1 = build_account_report(&input).unwrap();
    let r2 = build_account_report(&input).unwrap();
    assert_eq!(r1, r2);
}

// T07: stable segment order
#[test]
fn t07_stable_segment_order() {
    let input = AccountInput {
        system_prompt_bytes: 400,
        rules_bytes: 200,
        history_messages: vec![HistoryPiece { bytes: 80 }, HistoryPiece { bytes: 120 }],
        tool_schemas_bytes: vec![160],
        current_turn_bytes: 40,
        model_limit: 5000,
        compaction_threshold: 0.8,
    };
    let report = build_account_report(&input).unwrap();
    assert_eq!(report.segments[0].kind, SegmentKind::System);
    assert_eq!(report.segments[1].kind, SegmentKind::Rules);
    assert_eq!(report.segments[2].kind, SegmentKind::History);
    assert_eq!(report.segments[3].kind, SegmentKind::History);
    assert_eq!(report.segments[4].kind, SegmentKind::ToolSchemas);
    assert_eq!(report.segments[5].kind, SegmentKind::CurrentTurn);
}

// T08: snapshot struct serializes round-trips
#[test]
fn t08_snapshot_roundtrip() {
    let input = AccountInput {
        system_prompt_bytes: 400,
        rules_bytes: 200,
        history_messages: vec![HistoryPiece { bytes: 80 }],
        tool_schemas_bytes: vec![160],
        current_turn_bytes: 40,
        model_limit: 5000,
        compaction_threshold: 0.8,
    };
    let report = build_account_report(&input).unwrap();
    let snap = report.snapshot();
    let restored = ContextAccountSnapshot::from_json(&snap).unwrap();
    assert_eq!(restored.total_tokens, report.total_tokens);
    assert_eq!(restored.model_limit, report.model_limit);
    assert_eq!(restored.needs_compaction, report.needs_compaction);
    assert_eq!(restored.segments.len(), report.segments.len());
    for (a, b) in restored.segments.iter().zip(report.segments.iter()) {
        assert_eq!(a.kind, format!("{:?}", b.kind));
        assert_eq!(a.tokens, b.tokens);
    }
}

// T09: empty input — zero tokens, full headroom, no compaction
#[test]
fn t09_empty_input() {
    let input = AccountInput {
        model_limit: 1000,
        compaction_threshold: 0.8,
        ..Default::default()
    };
    let report = build_account_report(&input).unwrap();
    assert_eq!(report.total_tokens, 0);
    assert!(report.segments.is_empty());
    assert!(!report.truncated);
    assert_eq!(report.headroom, 1000);
    assert!(!report.needs_compaction);
}

// T10: zero model limit is an error
#[test]
fn t10_zero_model_limit_error() {
    let input = AccountInput {
        model_limit: 0,
        compaction_threshold: 0.8,
        ..Default::default()
    };
    assert_eq!(
        build_account_report(&input),
        Err(AccountError::ZeroModelLimit)
    );
}

// T11: headroom saturates when total exceeds limit
#[test]
fn t11_headroom_saturates_over_limit() {
    let input = AccountInput {
        system_prompt_bytes: 1000, // 250 tokens
        model_limit: 200,
        compaction_threshold: 0.8,
        ..Default::default()
    };
    let report = build_account_report(&input).unwrap();
    assert_eq!(report.total_tokens, 250);
    assert_eq!(report.headroom, 0); // saturating_sub
    assert!(report.needs_compaction);
}

// T12: tool_schemas with zero bytes are skipped (no empty segments)
#[test]
fn t12_zero_tool_schemas_skipped() {
    let input = AccountInput {
        tool_schemas_bytes: vec![0, 4, 0],
        model_limit: 1000,
        compaction_threshold: 0.8,
        ..Default::default()
    };
    let report = build_account_report(&input).unwrap();
    let schema_count = report
        .segments
        .iter()
        .filter(|s| s.kind == SegmentKind::ToolSchemas)
        .count();
    assert_eq!(schema_count, 1);
    assert_eq!(report.segments[0].bytes, 4);
}
