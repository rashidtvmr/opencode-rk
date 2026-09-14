use opencode_rk_providers::router::{
    route_chore, ChoreKind, ModelCandidate, ModelRef, RouteDecision,
};

// ROUTE-012 keeps catalog concerns outside the router contract: candidates carry
// a comparable routing cost directly (lower `cost_microunits` is cheaper), plus
// current availability and the chores they support. Equal-cost candidates are
// ordered by ModelRef (provider_id, then model_id) so selection does not depend
// on input iteration order.

fn model(provider_id: &str, model_id: &str) -> ModelRef {
    ModelRef {
        provider_id: provider_id.to_owned(),
        model_id: model_id.to_owned(),
    }
}

fn candidate(
    model: ModelRef,
    cost_microunits: u64,
    available: bool,
    supported_chores: Vec<ChoreKind>,
) -> ModelCandidate {
    ModelCandidate {
        model,
        cost_microunits,
        available,
        supported_chores,
    }
}

#[test]
fn route_012_t01_quota_uses_cheapest_capable_model_and_caps_output_at_one() {
    let main = model("anthropic", "sonnet");
    let cheap = model("anthropic", "haiku");
    let expensive = model("openai", "mini");
    let candidates = vec![
        candidate(
            expensive,
            5,
            true,
            vec![ChoreKind::QuotaProbe],
        ),
        candidate(cheap.clone(), 1, true, vec![ChoreKind::QuotaProbe]),
    ];

    let decision = route_chore(ChoreKind::QuotaProbe, &main, &candidates);

    assert_eq!(
        decision,
        RouteDecision {
            model: cheap,
            max_output_tokens: Some(1),
        }
    );
}

#[test]
fn route_012_t02_topic_title_and_summarize_use_cheapest_capable_model() {
    let main = model("anthropic", "sonnet");
    let cheap = model("anthropic", "haiku");
    let expensive = model("openai", "mini");
    let candidates = vec![
        candidate(
            expensive,
            5,
            true,
            vec![ChoreKind::Topic, ChoreKind::Title, ChoreKind::Summarize],
        ),
        candidate(
            cheap.clone(),
            1,
            true,
            vec![ChoreKind::Topic, ChoreKind::Title, ChoreKind::Summarize],
        ),
    ];

    for chore in [ChoreKind::Topic, ChoreKind::Title, ChoreKind::Summarize] {
        let decision = route_chore(chore, &main, &candidates);

        assert_eq!(decision.model, cheap);
        assert_eq!(decision.max_output_tokens, None);
    }
}

#[test]
fn route_012_t03_unavailable_or_unsupported_cheaper_models_are_skipped() {
    let main = model("anthropic", "sonnet");
    let eligible = model("provider-c", "eligible");
    let candidates = vec![
        candidate(
            model("provider-a", "unavailable"),
            1,
            false,
            vec![ChoreKind::Title],
        ),
        candidate(
            model("provider-b", "unsupported"),
            2,
            true,
            vec![ChoreKind::Topic],
        ),
        candidate(eligible.clone(), 3, true, vec![ChoreKind::Title]),
    ];

    let decision = route_chore(ChoreKind::Title, &main, &candidates);

    assert_eq!(decision.model, eligible);
    assert_eq!(decision.max_output_tokens, None);
}

#[test]
fn route_012_t04_no_capable_candidate_fails_open_to_callers_main_model() {
    let main = model("anthropic", "sonnet");
    let candidates = vec![
        candidate(
            model("provider-a", "unavailable"),
            1,
            false,
            vec![ChoreKind::Summarize],
        ),
        candidate(
            model("provider-b", "unsupported"),
            2,
            true,
            vec![ChoreKind::Title],
        ),
    ];

    let decision = route_chore(ChoreKind::Summarize, &main, &candidates);

    assert_eq!(
        decision,
        RouteDecision {
            model: main,
            max_output_tokens: None,
        }
    );
}

#[test]
fn route_012_t05_equal_cost_selection_is_deterministic() {
    let main = model("anthropic", "sonnet");
    let alpha = model("anthropic", "alpha");
    let zeta = model("anthropic", "zeta");

    let first_order = vec![
        candidate(zeta.clone(), 1, true, vec![ChoreKind::Topic]),
        candidate(alpha.clone(), 1, true, vec![ChoreKind::Topic]),
    ];
    let second_order = vec![
        candidate(alpha.clone(), 1, true, vec![ChoreKind::Topic]),
        candidate(zeta, 1, true, vec![ChoreKind::Topic]),
    ];

    let first = route_chore(ChoreKind::Topic, &main, &first_order);
    let second = route_chore(ChoreKind::Topic, &main, &second_order);

    assert_eq!(first.model, alpha);
    assert_eq!(second.model, alpha);
    assert_eq!(first, second);
}
