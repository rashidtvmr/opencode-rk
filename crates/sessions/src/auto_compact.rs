//! Auto-compaction policy contract.

pub const MAX_OUTPUT_TOKENS_FOR_SUMMARY: u32 = 20_000;
pub const AUTOCOMPACT_BUFFER: u32 = 13_000;
pub const WARNING_BUFFER: u32 = 20_000;
pub const MANUAL_COMPACT_BUFFER: u32 = 3_000;
pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactAction {
    None,
    AutoCompact,
    ManualCompact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenWarningState {
    Normal,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactPolicyInput {
    pub token_count: u32,
    pub effective_context_window: u32,
    pub manual_requested: bool,
    pub disable_compact: bool,
    pub disable_auto_compact: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompactPolicyState {
    pub consecutive_failures: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactDecision {
    pub action: CompactAction,
    pub warning: TokenWarningState,
    pub breaker_open: bool,
}

#[must_use]
pub fn evaluate_compact_policy(
    input: &CompactPolicyInput,
    state: &CompactPolicyState,
) -> CompactDecision {
    let breaker_open = state.consecutive_failures >= MAX_CONSECUTIVE_FAILURES;
    let warning = warning_state(input.token_count, input.effective_context_window);

    let action = if input.disable_compact {
        CompactAction::None
    } else if input.manual_requested {
        CompactAction::ManualCompact
    } else if input.disable_auto_compact || breaker_open {
        CompactAction::None
    } else if input.effective_context_window > 0
        && input.token_count
            >= input
                .effective_context_window
                .saturating_sub(AUTOCOMPACT_BUFFER)
    {
        CompactAction::AutoCompact
    } else {
        CompactAction::None
    };

    CompactDecision {
        action,
        warning,
        breaker_open,
    }
}

pub fn record_compaction_result(state: &mut CompactPolicyState, success: bool) {
    if success {
        state.consecutive_failures = 0;
    } else {
        state.consecutive_failures = state
            .consecutive_failures
            .saturating_add(1)
            .min(MAX_CONSECUTIVE_FAILURES);
    }
}

fn warning_state(token_count: u32, effective_context_window: u32) -> TokenWarningState {
    if effective_context_window == 0 || token_count >= effective_context_window {
        TokenWarningState::Error
    } else if effective_context_window.saturating_sub(token_count) <= WARNING_BUFFER {
        TokenWarningState::Warning
    } else {
        TokenWarningState::Normal
    }
}
