//! P1: session compaction interior parity with pinned upstream
//! `packages/opencode/src/session/compaction.ts` and `overflow.ts` at
//! `95daf90670b7c039c436c85537da5fbfe2205b41`.
//!
//! Pure, caller-owned, bounded. No I/O, clock, threads or provider calls.

use opencode_rk_sessions::compaction::{
    is_overflow, plan_prune, preserve_recent_budget, truncate_tool_output, usable_context, MessageRef,
    PruneError, PrunePlan, Role, ToolPartRef, COMPACTION_BUFFER, MAX_PRESERVE_RECENT_TOKENS,
    MIN_PRESERVE_RECENT_TOKENS, PRUNE_MINIMUM, PRUNE_PROTECT, TOOL_OUTPUT_MAX_CHARS,
};

fn tool(tool: &str, completed: bool, compacted: bool, tokens: u64) -> ToolPartRef {
    ToolPartRef {
        tool: tool.to_owned(),
        completed,
        compacted,
        output_tokens: tokens,
    }
}

fn user() -> MessageRef {
    MessageRef {
        role: Role::User,
        is_summary: false,
        tool_parts: Vec::new(),
    }
}

fn assistant(parts: Vec<ToolPartRef>) -> MessageRef {
    MessageRef {
        role: Role::Assistant,
        is_summary: false,
        tool_parts: parts,
    }
}

fn summary(parts: Vec<ToolPartRef>) -> MessageRef {
    MessageRef {
        role: Role::Assistant,
        is_summary: true,
        tool_parts: parts,
    }
}

#[test]
fn p1_t01_upstream_constants_and_usable_overflow() {
    assert_eq!(COMPACTION_BUFFER, 20_000);
    assert_eq!(PRUNE_MINIMUM, 20_000);
    assert_eq!(PRUNE_PROTECT, 40_000);
    assert_eq!(TOOL_OUTPUT_MAX_CHARS, 2_000);
    assert_eq!(MIN_PRESERVE_RECENT_TOKENS, 2_000);
    assert_eq!(MAX_PRESERVE_RECENT_TOKENS, 15_000);

    assert_eq!(usable_context(0, None, None, 32_000), 0);
    assert_eq!(usable_context(200_000, Some(180_000), None, 32_000), 160_000);
    assert_eq!(usable_context(200_000, None, None, 32_000), 168_000);
    assert_eq!(
        usable_context(200_000, Some(180_000), Some(5_000), 32_000),
        175_000
    );

    assert!(!is_overflow(159_999, 160_000, true, 200_000));
    assert!(is_overflow(160_000, 160_000, true, 200_000));
    assert!(!is_overflow(999_999, 160_000, false, 200_000));
    assert!(!is_overflow(999_999, 0, true, 0));
}

#[test]
fn p1_t02_preserve_recent_budget_clamps_configured_and_derived() {
    assert_eq!(preserve_recent_budget(100_000, None), 15_000);
    assert_eq!(preserve_recent_budget(4_000, None), 2_000);
    assert_eq!(preserve_recent_budget(40_000, None), 10_000);
    assert_eq!(preserve_recent_budget(100_000, Some(7_777)), 7_777);
}

#[test]
fn p1_t03_prune_skips_recent_turn_and_emits_only_past_protect() {
    // Four turns. Scanning newest-first, the two most recent turns are skipped
    // by the `turns < 2` gate. The 50k tool output consumes the 40k protect
    // budget; the 30k call beyond it is pruned (> PRUNE_MINIMUM).
    let messages = vec![
        user(),
        assistant(vec![tool("read", true, false, 30_000)]),
        user(),
        assistant(vec![tool("grep", true, false, 50_000)]),
        user(),
        assistant(vec![tool("bash", true, false, 30_000)]),
        user(),
        assistant(vec![tool("read", true, false, 1_000)]),
    ];
    let plan = plan_prune(&messages, true).expect("valid input");
    assert!(plan.prune);
    assert_eq!(plan.pruned_tokens, 80_000);
    // Newest-first scan order: the 50k call beyond protect, then the old 30k.
    assert_eq!(plan.targets.len(), 2);
    assert_eq!(plan.targets[0].message_index, 3);
    assert_eq!(plan.targets[0].part_index, 0);
    assert_eq!(plan.targets[1].message_index, 1);
    assert!(!plan.truncated);
}

#[test]
fn p1_t04_prune_below_minimum_reports_no_prune() {
    // Protect consumed by 30k + 1k, then 19k spills over but is <= PRUNE_MINIMUM.
    let messages = vec![
        user(),
        assistant(vec![tool("read", true, false, 19_000)]),
        user(),
        assistant(vec![tool("grep", true, false, 1_000)]),
        user(),
        assistant(vec![tool("bash", true, false, 30_000)]),
        user(),
        assistant(vec![tool("read", true, false, 1_000)]),
    ];
    let plan = plan_prune(&messages, true).expect("valid input");
    assert!(!plan.prune);
    assert!(plan.targets.is_empty());
    assert_eq!(plan.pruned_tokens, 0);
}

#[test]
fn p1_t05_prune_gates_summary_compacted_and_protected_tools() {
    // Disabled gate never scans.
    let messages = vec![
        user(),
        assistant(vec![tool("read", true, false, 99_999)]),
        user(),
        assistant(vec![tool("grep", true, false, 99_999)]),
    ];
    assert!(!plan_prune(&messages, false).expect("valid input").prune);

    // A summary assistant stops the backward scan before older tool calls.
    let with_summary = vec![
        user(),
        assistant(vec![tool("bash", true, false, 90_000)]),
        summary(vec![tool("read", true, false, 99_000)]),
        user(),
        assistant(vec![tool("grep", true, false, 1_000)]),
        user(),
        assistant(vec![tool("read", true, false, 1_000)]),
    ];
    let plan = plan_prune(&with_summary, true).expect("valid input");
    assert!(!plan.prune);
    assert!(plan.targets.is_empty());

    // An already-compacted part stops the scan; the newer 50k is still pruned.
    let compacted = vec![
        user(),
        assistant(vec![tool("bash", true, true, 30_000)]),
        user(),
        assistant(vec![tool("grep", true, false, 50_000)]),
        user(),
        assistant(vec![tool("read", true, false, 30_000)]),
        user(),
        assistant(vec![tool("read", true, false, 1_000)]),
    ];
    let plan = plan_prune(&compacted, true).expect("valid input");
    assert!(plan.prune);
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.targets[0].message_index, 3);

    // The protected "skill" tool output is never pruned.
    let protected = vec![
        user(),
        assistant(vec![tool("skill", true, false, 99_000)]),
        user(),
        assistant(vec![tool("grep", true, false, 50_000)]),
        user(),
        assistant(vec![tool("read", true, false, 30_000)]),
        user(),
        assistant(vec![tool("read", true, false, 1_000)]),
    ];
    let plan = plan_prune(&protected, true).expect("valid input");
    assert!(plan.prune);
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.targets[0].message_index, 3);
}

#[test]
fn p1_t06_truncate_and_message_bound() {
    let short = "x".repeat(TOOL_OUTPUT_MAX_CHARS);
    assert_eq!(truncate_tool_output(&short), short);
    let long = "y".repeat(TOOL_OUTPUT_MAX_CHARS + 10);
    let out = truncate_tool_output(&long);
    assert!(out.ends_with("\n[truncated]"));
    assert_eq!(
        out.chars().filter(|c| *c == 'y').count(),
        TOOL_OUTPUT_MAX_CHARS
    );

    let huge = vec![user(); 100_001];
    assert_eq!(plan_prune(&huge, true), Err(PruneError::TooManyMessages));
    let _ = PrunePlan::default();
}