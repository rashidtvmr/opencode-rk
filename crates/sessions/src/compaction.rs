//! Session compaction interior parity with pinned upstream
//! `packages/opencode/src/session/compaction.ts` and `overflow.ts` at
//! `95daf90670b7c039c436c85537da5fbfe2205b41`.
//!
//! Pure, caller-owned, bounded decision helpers. No I/O, clock, threads,
//! provider calls or global state. The caller owns the transcript and applies
//! the returned prune targets; this module never mutates input.
#![forbid(unsafe_code)]

use std::fmt;

/// Reserved context for the compaction buffer (upstream `overflow.ts`).
pub const COMPACTION_BUFFER: u64 = 20_000;
/// Minimum pruned tokens before pruning is committed (upstream `PRUNE_MINIMUM`).
pub const PRUNE_MINIMUM: u64 = 20_000;
/// Tool-output tokens protected from pruning (upstream `PRUNE_PROTECT`).
pub const PRUNE_PROTECT: u64 = 40_000;
/// Max characters kept from one tool output before truncation.
pub const TOOL_OUTPUT_MAX_CHARS: usize = 2_000;
/// Lower bound of the derived recent-turn preservation budget.
pub const MIN_PRESERVE_RECENT_TOKENS: u64 = 2_000;
/// Upper bound of the derived recent-turn preservation budget.
pub const MAX_PRESERVE_RECENT_TOKENS: u64 = 15_000;
/// Tool names whose output is never pruned.
pub const PRUNE_PROTECTED_TOOLS: [&str; 1] = ["skill"];
/// Hard bound on transcript messages admitted to a prune scan.
pub const MAX_PRUNE_MESSAGES: usize = 100_000;

/// Message role as far as pruning cares.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    User,
    Assistant,
}

/// One completed-or-not tool part visible to the prune scan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolPartRef {
    pub tool: String,
    pub completed: bool,
    pub compacted: bool,
    pub output_tokens: u64,
}

/// One transcript message and its tool parts, in stored order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageRef {
    pub role: Role,
    pub is_summary: bool,
    pub tool_parts: Vec<ToolPartRef>,
}

/// A single tool part selected for pruning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PruneTarget {
    pub message_index: usize,
    pub part_index: usize,
    pub tokens: u64,
}

/// Bounded prune decision. `targets` is empty unless `prune` is true.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PrunePlan {
    pub prune: bool,
    pub targets: Vec<PruneTarget>,
    pub pruned_tokens: u64,
    pub truncated: bool,
}

/// Input failures; never carry transcript content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PruneError {
    /// Transcript exceeds [`MAX_PRUNE_MESSAGES`].
    TooManyMessages,
}

impl fmt::Display for PruneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyMessages => f.write_str("transcript exceeds prune scan bound"),
        }
    }
}

impl std::error::Error for PruneError {}

/// Usable context window for one model.
///
/// Mirrors upstream `usable`: `0` when the model declares no context window;
/// otherwise `input - reserved` when an input window exists, else
/// `context - max_output`. `reserved` defaults to
/// `min(COMPACTION_BUFFER, max_output)`.
#[must_use]
pub fn usable_context(
    context: u64,
    input_window: Option<u64>,
    reserved: Option<u64>,
    output_token_max: u64,
) -> u64 {
    if context == 0 {
        return 0;
    }
    let reserved = reserved.unwrap_or_else(|| COMPACTION_BUFFER.min(output_token_max));
    match input_window {
        Some(input) => input.saturating_sub(reserved),
        None => context.saturating_sub(output_token_max),
    }
}

/// Whether accumulated tokens overflow the usable window.
///
/// `auto_enabled` is the explicit auto-compaction switch; an empty context
/// window never overflows.
#[must_use]
pub fn is_overflow(count: u64, usable: u64, auto_enabled: bool, context: u64) -> bool {
    if !auto_enabled || context == 0 {
        return false;
    }
    count >= usable
}

/// Derived recent-turn preservation budget.
///
/// Uses the configured value when supplied, else clamps `floor(usable / 4)`
/// into `[MIN, MAX]`.
#[must_use]
pub fn preserve_recent_budget(usable: u64, configured: Option<u64>) -> u64 {
    configured.unwrap_or_else(|| {
        let derived = usable / 4;
        derived.clamp(MIN_PRESERVE_RECENT_TOKENS, MAX_PRESERVE_RECENT_TOKENS)
    })
}

/// Truncate one tool output to [`TOOL_OUTPUT_MAX_CHARS`], appending the
/// upstream marker only when truncation happens. Character-based, so it never
/// splits a UTF-8 code point.
#[must_use]
pub fn truncate_tool_output(value: &str) -> String {
    if value.chars().count() <= TOOL_OUTPUT_MAX_CHARS {
        return value.to_owned();
    }
    let head: String = value.chars().take(TOOL_OUTPUT_MAX_CHARS).collect();
    format!("{head}\n[truncated]")
}

/// Plan a backward prune scan without mutating the transcript.
///
/// Mirrors upstream `prune`: the two most recent user turns are skipped, the
/// scan stops at an assistant summary or an already-compacted part, protected
/// tool outputs are ignored, and `PRUNE_PROTECT` tokens are retained before
/// pruning begins. `gate` is the config prune switch; when false no scan runs.
pub fn plan_prune(messages: &[MessageRef], gate: bool) -> Result<PrunePlan, PruneError> {
    if messages.len() > MAX_PRUNE_MESSAGES {
        return Err(PruneError::TooManyMessages);
    }
    let mut plan = PrunePlan::default();
    if !gate {
        return Ok(plan);
    }

    let mut total: u64 = 0;
    let mut pruned: u64 = 0;
    let mut turns: u32 = 0;
    let mut targets: Vec<PruneTarget> = Vec::new();

    'scan: for msg_index in (0..messages.len()).rev() {
        let msg = &messages[msg_index];
        if msg.role == Role::User {
            turns = turns.saturating_add(1);
        }
        if turns < 2 {
            continue;
        }
        if msg.role == Role::Assistant && msg.is_summary {
            break 'scan;
        }
        for part_index in (0..msg.tool_parts.len()).rev() {
            let part = &msg.tool_parts[part_index];
            if !part.completed {
                continue;
            }
            if PRUNE_PROTECTED_TOOLS.contains(&part.tool.as_str()) {
                continue;
            }
            if part.compacted {
                break 'scan;
            }
            let estimate = part.output_tokens;
            total = total.saturating_add(estimate);
            if total <= PRUNE_PROTECT {
                continue;
            }
            pruned = pruned.saturating_add(estimate);
            targets.push(PruneTarget {
                message_index: msg_index,
                part_index,
                tokens: estimate,
            });
        }
    }

    if pruned > PRUNE_MINIMUM {
        plan.prune = true;
        plan.pruned_tokens = pruned;
        plan.targets = targets;
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_never_splits_utf8() {
        let value: String = "\u{00e9}".repeat(TOOL_OUTPUT_MAX_CHARS + 5);
        let out = truncate_tool_output(&value);
        assert!(out.ends_with("\n[truncated]"));
        assert_eq!(out.chars().filter(|c| *c == '\u{00e9}').count(), TOOL_OUTPUT_MAX_CHARS);
    }

    #[test]
    fn preserve_recent_budget_boundaries() {
        assert_eq!(preserve_recent_budget(0, None), MIN_PRESERVE_RECENT_TOKENS);
        assert_eq!(preserve_recent_budget(1_000_000, None), MAX_PRESERVE_RECENT_TOKENS);
    }
}