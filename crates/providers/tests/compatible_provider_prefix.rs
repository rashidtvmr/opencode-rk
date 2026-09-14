use std::collections::BTreeMap;

use opencode_rk_providers::{
    model_route::{
        CompatibleProviderNode, CompatibleProviderPrefixError, MAX_COMPATIBLE_PROVIDER_NODES,
        resolve_compatible_provider_prefix,
    },
    router::ModelRef,
};

fn reserved(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(alias, provider)| ((*alias).to_owned(), (*provider).to_owned()))
        .collect()
}

fn node(id: impl Into<String>, prefix: impl Into<String>) -> CompatibleProviderNode {
    CompatibleProviderNode {
        id: id.into(),
        prefix: prefix.into(),
    }
}

#[test]
fn route_007_t01_reserved_alias_wins_over_colliding_compatible_prefix() {
    let aliases = reserved(&[("cf", "cloudflare-ai")]);
    let nodes = [node("openai-compatible-chat-test", "cf")];

    let resolved = resolve_compatible_provider_prefix(
        "cf/@cf/black-forest-labs/flux-2-klein-9b",
        &aliases,
        &nodes,
    )
    .expect("reserved provider aliases must win before compatible-node prefixes");

    assert_eq!(
        resolved,
        ModelRef {
            provider_id: "cloudflare-ai".to_owned(),
            model_id: "@cf/black-forest-labs/flux-2-klein-9b".to_owned(),
        }
    );
}

#[test]
fn route_007_t02_non_reserved_prefix_resolves_to_compatible_node() {
    let aliases = reserved(&[("openai", "openai")]);
    let nodes = [node("openai-compatible-chat-test", "oct")];

    let resolved = resolve_compatible_provider_prefix("oct/gpt-image-1", &aliases, &nodes)
        .expect("non-reserved compatible prefix should resolve to its node id");

    assert_eq!(
        resolved,
        ModelRef {
            provider_id: "openai-compatible-chat-test".to_owned(),
            model_id: "gpt-image-1".to_owned(),
        }
    );
}

#[test]
fn route_007_t03_explicit_canonical_provider_model_remains_canonical() {
    let aliases = reserved(&[("openai", "openai")]);
    let nodes = [node("compatible-collision", "openai")];

    let resolved = resolve_compatible_provider_prefix("openai/gpt-5", &aliases, &nodes)
        .expect("canonical provider/model must not be shadowed by a compatible node");

    assert_eq!(
        resolved,
        ModelRef {
            provider_id: "openai".to_owned(),
            model_id: "gpt-5".to_owned(),
        }
    );
}

#[test]
fn route_007_t04_malformed_and_unknown_prefixes_return_typed_errors() {
    let aliases = reserved(&[("openai", "openai")]);
    let nodes = [node("openai-compatible-chat-test", "oct")];

    assert_eq!(
        resolve_compatible_provider_prefix("/gpt-5", &aliases, &nodes),
        Err(CompatibleProviderPrefixError::InvalidInput)
    );
    assert_eq!(
        resolve_compatible_provider_prefix("oct/", &aliases, &nodes),
        Err(CompatibleProviderPrefixError::InvalidInput)
    );
    assert_eq!(
        resolve_compatible_provider_prefix("missing/gpt-5", &aliases, &nodes),
        Err(CompatibleProviderPrefixError::UnknownPrefix(
            "missing".to_owned()
        ))
    );
}

#[test]
fn route_007_t05_compatible_node_input_is_bounded_at_public_limit() {
    let aliases = BTreeMap::new();
    let at_limit: Vec<_> = (0..MAX_COMPATIBLE_PROVIDER_NODES)
        .map(|index| node(format!("node-{index}"), format!("p{index}")))
        .collect();
    let boundary_input = format!("p{}/model", MAX_COMPATIBLE_PROVIDER_NODES - 1);

    let resolved = resolve_compatible_provider_prefix(&boundary_input, &aliases, &at_limit)
        .expect("a compatible-node slice at the public limit must remain valid");
    assert_eq!(
        resolved,
        ModelRef {
            provider_id: format!("node-{}", MAX_COMPATIBLE_PROVIDER_NODES - 1),
            model_id: "model".to_owned(),
        }
    );

    let mut overflow = at_limit;
    overflow.push(node("overflow-node", "overflow"));
    assert_eq!(
        resolve_compatible_provider_prefix("overflow/model", &aliases, &overflow),
        Err(CompatibleProviderPrefixError::TooManyCompatibleProviderNodes {
            max: MAX_COMPATIBLE_PROVIDER_NODES,
            actual: MAX_COMPATIBLE_PROVIDER_NODES + 1,
        })
    );
}
