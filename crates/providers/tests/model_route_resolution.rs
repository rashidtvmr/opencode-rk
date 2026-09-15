use std::collections::BTreeMap;

use opencode_rk_providers::{
    model_route::{resolve_model_route, ModelRoute, ModelRouteError, MAX_COMBO_MODELS},
    router::ModelRef,
};

fn model(provider_id: &str, model_id: &str) -> ModelRef {
    ModelRef {
        provider_id: provider_id.to_owned(),
        model_id: model_id.to_owned(),
    }
}

#[test]
fn route_005_t01_explicit_provider_model_resolves_directly() {
    let aliases = BTreeMap::from([(
        "openai/gpt-5".to_owned(),
        model("anthropic", "claude-sonnet"),
    )]);
    let combos = BTreeMap::from([(
        "openai/gpt-5".to_owned(),
        vec![model("anthropic", "claude-haiku")],
    )]);

    let resolved = resolve_model_route("openai/gpt-5", &aliases, &combos)
        .expect("explicit provider/model target should resolve");

    assert_eq!(resolved, ModelRoute::Direct(model("openai", "gpt-5")));
}

#[test]
fn route_005_t02_known_combo_resolves_to_named_combo_and_member_list() {
    let aliases = BTreeMap::new();
    let members = vec![
        model("anthropic", "claude-sonnet"),
        model("openai", "gpt-5"),
    ];
    let combos = BTreeMap::from([("balanced".to_owned(), members.clone())]);

    let resolved = resolve_model_route("balanced", &aliases, &combos)
        .expect("known combo name should resolve");

    assert_eq!(
        resolved,
        ModelRoute::Combo {
            name: "balanced".to_owned(),
            models: members,
        }
    );
}

#[test]
fn route_005_t03_combo_name_wins_over_same_name_alias() {
    let aliases = BTreeMap::from([("fast".to_owned(), model("anthropic", "claude-haiku"))]);
    let combo_members = vec![
        model("openai", "gpt-5-mini"),
        model("google", "gemini-flash"),
    ];
    let combos = BTreeMap::from([("fast".to_owned(), combo_members.clone())]);

    let resolved = resolve_model_route("fast", &aliases, &combos)
        .expect("combo should take precedence over alias with the same name");

    assert_eq!(
        resolved,
        ModelRoute::Combo {
            name: "fast".to_owned(),
            models: combo_members,
        }
    );
}

#[test]
fn route_005_t04_known_alias_resolves_to_explicit_provider_model() {
    let aliases = BTreeMap::from([("coding".to_owned(), model("anthropic", "claude-sonnet"))]);
    let combos = BTreeMap::new();

    let resolved = resolve_model_route("coding", &aliases, &combos)
        .expect("known alias should resolve to one provider/model target");

    assert_eq!(
        resolved,
        ModelRoute::Direct(model("anthropic", "claude-sonnet"))
    );
}

#[test]
fn route_005_t05_invalid_unknown_and_combo_size_limits_return_typed_results() {
    assert_eq!(
        MAX_COMBO_MODELS, 8,
        "combo member bound is part of the public contract"
    );

    let aliases = BTreeMap::new();
    let empty_combos = BTreeMap::new();

    assert_eq!(
        resolve_model_route("", &aliases, &empty_combos),
        Err(ModelRouteError::InvalidInput)
    );
    assert_eq!(
        resolve_model_route("provider/", &aliases, &empty_combos),
        Err(ModelRouteError::InvalidInput)
    );
    assert_eq!(
        resolve_model_route("/model", &aliases, &empty_combos),
        Err(ModelRouteError::InvalidInput)
    );
    assert_eq!(
        resolve_model_route("unknown-name", &aliases, &empty_combos),
        Err(ModelRouteError::UnknownModel("unknown-name".to_owned()))
    );

    let boundary_members = (0..MAX_COMBO_MODELS)
        .map(|index| model("provider", &format!("model-{index}")))
        .collect::<Vec<_>>();
    let boundary_combos = BTreeMap::from([("boundary".to_owned(), boundary_members.clone())]);

    assert_eq!(
        resolve_model_route("boundary", &aliases, &boundary_combos),
        Ok(ModelRoute::Combo {
            name: "boundary".to_owned(),
            models: boundary_members,
        })
    );

    let overflow_members = (0..=MAX_COMBO_MODELS)
        .map(|index| model("provider", &format!("overflow-{index}")))
        .collect::<Vec<_>>();
    let overflow_combos = BTreeMap::from([("overflow".to_owned(), overflow_members)]);

    assert_eq!(
        resolve_model_route("overflow", &aliases, &overflow_combos),
        Err(ModelRouteError::TooManyComboModels {
            name: "overflow".to_owned(),
            max: MAX_COMBO_MODELS,
            actual: MAX_COMBO_MODELS + 1,
        })
    );
}
