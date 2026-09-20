//! Agentic turn-loop controller (bounded provider/tool rounds per turn).
//!
//! The live turn stream (`create_turn_stream`) previously announced tool
//! calls without executing them and had no iteration bound. This module is
//! the pure-state half of the repair: it owns the step budget (cap 64, the
//! same bound as `agents::agent_executor::MAX_STEPS`), per-round tool-call
//! accounting, tool-result truncation, the Responses `function_call_output`
//! payload format, and terminal mapping for stop reasons including the
//! step limit. The async orchestration (provider stream, tool executor,
//! persistence) stays in the caller; this type cannot hang, block, or IO.
//!
//! Cited gaps this closes (audited 2026-09-18):
//! - crates/server/src/lib.rs:163 "the current web Responses turn adapter
//!   does not execute native tools"
//! - no stop_reason/finish_reason handling anywhere in crates/ before
//!   3b9f69c/be4a16a; step-limit termination did not exist at all.
#![forbid(unsafe_code)]

use std::fmt;

/// Hard cap on provider/tool rounds per user turn. One round = one provider
/// stream consumption plus at most one tool dispatch batch from it.
pub const MAX_TURN_STEPS: u32 = 64;

/// Hard cap on tool calls accepted from a single provider round.
pub const MAX_CALLS_PER_ROUND: usize = 16;

/// Maximum retained characters of a tool result fed back to the provider;
/// larger outputs are truncated with a visible marker (bounded context).
pub const MAX_TOOL_OUTPUT_CHARS: usize = 16_384;

/// Why a turn reached its terminal state. Mirrors standard agent-loop
/// stop reasons and adds the local iteration-cap cause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TurnStop {
    /// Model finished normally.
    Completed,
    /// Model stopped early (e.g. max_output_tokens, content filter).
    Incomplete { reason: Option<String> },
    /// Local iteration cap reached while the model still requested tools.
    MaxSteps { steps: u32 },
    /// Policy denied a requested tool; the turn ends without executing it.
    PolicyDenied { tool: String },
}

impl TurnStop {
    /// Stable wire string for NDJSON consumers (TUI, web).
    pub fn as_str(&self) -> String {
        match self {
            TurnStop::Completed => "completed".to_owned(),
            TurnStop::Incomplete { reason: None } => "incomplete".to_owned(),
            TurnStop::Incomplete { reason: Some(r) } => format!("incomplete:{r}"),
            TurnStop::MaxSteps { steps } => format!("max_steps:{steps}"),
            TurnStop::PolicyDenied { tool } => format!("policy_denied:{tool}"),
        }
    }
}

impl fmt::Display for TurnStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

/// One tool invocation requested by the provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestedCall {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
}

/// A tool result formatted for provider feedback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallOutput {
    pub call_id: String,
    pub output: String,
}

/// Provider-requested action after one streamed round.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoundEnd {
    /// No tools requested; the turn terminates with this stop reason.
    Terminal(TurnStop),
    /// Tools requested; the caller must dispatch them and start a new round.
    ToolRound(Vec<RequestedCall>),
}

/// Step-budget controller for one user turn. Enforces the iteration cap the
/// live turn path never had: once `MAX_TURN_STEPS` rounds are consumed, any
/// further tool request is refused and the turn ends with `MaxSteps`.
#[derive(Debug)]
pub struct LoopController {
    steps_used: u32,
    max_steps: u32,
}

impl LoopController {
    pub fn new() -> Self {
        Self {
            steps_used: 0,
            max_steps: MAX_TURN_STEPS,
        }
    }

    pub fn with_cap(max_steps: u32) -> Self {
        Self {
            steps_used: 0,
            max_steps: max_steps.max(1),
        }
    }

    pub fn steps_used(&self) -> u32 {
        self.steps_used
    }

    /// Configured round cap (exposed for stop-reason reporting).
    pub fn max_steps(&self) -> u32 {
        self.max_steps
    }

    pub fn steps_remaining(&self) -> u32 {
        self.max_steps - self.steps_used
    }

    pub fn exhausted(&self) -> bool {
        self.steps_remaining() == 0
    }

    /// True when one more provider round may start. The turn's first provider
    /// round is implicit (it began before this controller existed), so a turn
    /// with cap N runs at most N provider rounds: with cap 1 the model gets
    /// exactly one round and any tool request ends the turn as [`TurnStop::MaxSteps`].
    pub fn can_start_next_round(&self) -> bool {
        self.steps_used < self.max_steps.saturating_sub(1)
    }

    /// Consume one round before streaming it. Refused when the budget is gone.
    pub fn begin_round(&mut self) -> Result<(), TurnStop> {
        if self.exhausted() {
            return Err(TurnStop::MaxSteps {
                steps: self.steps_used,
            });
        }
        self.steps_used += 1;
        Ok(())
    }

    /// Classify the end of a round. A tool round exceeding the per-round call
    /// cap is truncated to [`MAX_CALLS_PER_ROUND`] in place (overflow call
    /// notices stay in [`Self::truncate_calls`], which the dispatch stage
    /// uses); pending batches are never unbounded past this point.
    pub fn end_round(&self, mut calls: Vec<RequestedCall>) -> RoundEnd {
        if calls.is_empty() {
            return RoundEnd::Terminal(TurnStop::Completed);
        }
        calls.truncate(MAX_CALLS_PER_ROUND);
        RoundEnd::ToolRound(calls)
    }

    pub fn truncate_calls(&self, calls: &[RequestedCall]) -> (Vec<RequestedCall>, Vec<CallOutput>) {
        if calls.len() <= MAX_CALLS_PER_ROUND {
            return (calls.to_vec(), Vec::new());
        }
        let kept = calls[..MAX_CALLS_PER_ROUND].to_vec();
        let overflow = calls[MAX_CALLS_PER_ROUND..]
            .iter()
            .map(|call| CallOutput {
                call_id: call.call_id.clone(),
                output: format!(
                    "error: step budget per round exceeded (max {MAX_CALLS_PER_ROUND} calls); call not executed"
                ),
            })
            .collect();
        (kept, overflow)
    }
}

impl Default for LoopController {
    fn default() -> Self {
        Self::new()
    }
}

/// Truncate a tool result to [`MAX_TOOL_OUTPUT_CHARS`] with a visible marker
/// so the model (and transcript) can see the cut instead of silent loss.
pub fn truncate_tool_output(output: &str) -> String {
    if output.len() <= MAX_TOOL_OUTPUT_CHARS {
        return output.to_owned();
    }
    let mut cut = MAX_TOOL_OUTPUT_CHARS;
    while !output.is_char_boundary(cut) {
        cut -= 1;
    }
    format!(
        "{}\n[truncated {} of {} bytes]",
        &output[..cut],
        output.len() - cut,
        output.len()
    )
}

/// Build the Responses `function_call_output` input item for one executed call.
pub fn function_call_output(item: &CallOutput) -> serde_json::Value {
    serde_json::json!({
        "type": "function_call_output",
        "call_id": item.call_id,
        "output": item.output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // T01: cap refuses round 65 and reports the limit as the stop cause.
    #[test]
    fn t01_iteration_cap_terminates_the_turn() {
        let mut lc = LoopController::new();
        for _ in 0..MAX_TURN_STEPS {
            lc.begin_round().expect("within cap");
        }
        assert_eq!(lc.steps_used(), MAX_TURN_STEPS);
        assert!(lc.exhausted());
        assert_eq!(
            lc.begin_round(),
            Err(TurnStop::MaxSteps {
                steps: MAX_TURN_STEPS
            })
        );
    }

    // T02: a tool round does not end the turn; an empty round does.
    #[test]
    fn t02_tool_round_continues_and_empty_round_completes() {
        let lc = LoopController::with_cap(4);
        let calls = vec![RequestedCall {
            call_id: "call_1".into(),
            name: "bash".into(),
            arguments: "{}".into(),
        }];
        match lc.end_round(calls) {
            RoundEnd::ToolRound(batch) => assert_eq!(batch.len(), 1),
            RoundEnd::Terminal(_) => panic!("tool round must not be terminal"),
        }
        assert_eq!(
            lc.end_round(Vec::new()),
            RoundEnd::Terminal(TurnStop::Completed)
        );
    }

    // T03: per-round call cap truncates with explicit error outputs.
    #[test]
    fn t03_per_round_call_cap_truncates_with_error_outputs() {
        let lc = LoopController::new();
        let calls: Vec<RequestedCall> = (0..MAX_CALLS_PER_ROUND + 5)
            .map(|i| RequestedCall {
                call_id: format!("call_{i}"),
                name: "bash".into(),
                arguments: "{}".into(),
            })
            .collect();
        let (kept, overflow) = lc.truncate_calls(&calls);
        assert_eq!(kept.len(), MAX_CALLS_PER_ROUND);
        assert_eq!(overflow.len(), 5);
        assert!(overflow[0].output.contains("not executed"));
    }

    // T04: tool outputs are bounded with a visible truncation marker.
    #[test]
    fn t04_tool_output_truncation_is_visible_not_silent() {
        let small = "ok";
        assert_eq!(truncate_tool_output(small), "ok");
        let big = "x".repeat(MAX_TOOL_OUTPUT_CHARS + 100);
        let cut = truncate_tool_output(&big);
        assert!(cut.len() < big.len());
        assert!(cut.contains("[truncated 100 of"));
    }

    // T05: stop reasons serialize to stable wire strings.
    #[test]
    fn t05_stop_reasons_serialize_to_stable_strings() {
        assert_eq!(TurnStop::Completed.as_str(), "completed");
        assert_eq!(
            TurnStop::Incomplete {
                reason: Some("max_output_tokens".into())
            }
            .as_str(),
            "incomplete:max_output_tokens"
        );
        assert_eq!(
            TurnStop::MaxSteps { steps: 64 }.as_str(),
            "max_steps:64"
        );
        assert_eq!(
            TurnStop::PolicyDenied {
                tool: "bash".into()
            }
            .as_str(),
            "policy_denied:bash"
        );
    }

    // T06: function_call_output payloads match the Responses wire format.
    #[test]
    fn t06_function_call_output_matches_responses_wire_format() {
        let item = CallOutput {
            call_id: "call_9".into(),
            output: "file listing".into(),
        };
        let value = function_call_output(&item);
        assert_eq!(value["type"], "function_call_output");
        assert_eq!(value["call_id"], "call_9");
        assert_eq!(value["output"], "file listing");
    }
}
