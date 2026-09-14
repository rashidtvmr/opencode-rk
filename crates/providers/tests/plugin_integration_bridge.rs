use opencode_rk_providers::integration::{
    IntegrationError, IntegrationInfo, IntegrationMethod, IntegrationRegistry,
    PluginIntegrationDeclaration, PluginIntegrationRegistration, MAX_INTEGRATIONS_PER_SCOPE,
    MAX_INTEGRATION_METHODS,
};

fn integration(id: &str, name: &str) -> IntegrationInfo {
    IntegrationInfo {
        id: id.to_owned(),
        name: name.to_owned(),
    }
}

fn registration(
    id: &str,
    name: &str,
    methods: Vec<IntegrationMethod>,
) -> PluginIntegrationRegistration {
    PluginIntegrationRegistration {
        info: integration(id, name),
        methods,
    }
}

fn declaration(integrations: Vec<PluginIntegrationRegistration>) -> PluginIntegrationDeclaration {
    PluginIntegrationDeclaration { integrations }
}

#[test]
fn ext_007_t01_plugin_scope_registers_integration_info_and_key_metadata() {
    let mut registry = IntegrationRegistry::new();

    registry
        .register_plugin_scope(
            41,
            declaration(vec![registration(
                "opencode",
                "OpenCode",
                vec![IntegrationMethod::Key {
                    label: "API key (service account)".to_owned(),
                }],
            )]),
        )
        .expect("bounded plugin integration metadata should register");

    assert_eq!(
        registry.get("opencode"),
        Some(integration("opencode", "OpenCode"))
    );
    assert_eq!(
        registry.methods("opencode"),
        vec![IntegrationMethod::Key {
            label: "API key (service account)".to_owned(),
        }]
    );
}

#[test]
fn ext_007_t02_oauth_metadata_keeps_order_and_same_id_replaces_in_place() {
    let mut registry = IntegrationRegistry::new();

    registry
        .register_plugin_scope(
            7,
            declaration(vec![registration(
                "openai",
                "OpenAI",
                vec![
                    IntegrationMethod::OAuth {
                        id: "chatgpt-browser".to_owned(),
                        label: "ChatGPT Pro/Plus (browser)".to_owned(),
                    },
                    IntegrationMethod::OAuth {
                        id: "chatgpt-headless".to_owned(),
                        label: "ChatGPT Pro/Plus (headless)".to_owned(),
                    },
                    IntegrationMethod::OAuth {
                        id: "chatgpt-browser".to_owned(),
                        label: "ChatGPT browser replacement".to_owned(),
                    },
                ],
            )]),
        )
        .expect("same-id OAuth metadata should replace within the declaration");

    assert_eq!(
        registry.methods("openai"),
        vec![
            IntegrationMethod::OAuth {
                id: "chatgpt-browser".to_owned(),
                label: "ChatGPT browser replacement".to_owned(),
            },
            IntegrationMethod::OAuth {
                id: "chatgpt-headless".to_owned(),
                label: "ChatGPT Pro/Plus (headless)".to_owned(),
            },
        ]
    );
}

#[test]
fn ext_007_t03_later_plugin_scope_overrides_visible_integration_and_method_metadata() {
    let mut registry = IntegrationRegistry::new();

    registry
        .register_plugin_scope(
            100,
            declaration(vec![registration(
                "opencode",
                "OpenCode base",
                vec![IntegrationMethod::Key {
                    label: "Base key".to_owned(),
                }],
            )]),
        )
        .expect("base plugin scope should register");
    registry
        .register_plugin_scope(
            200,
            declaration(vec![registration(
                "opencode",
                "OpenCode override",
                vec![IntegrationMethod::Key {
                    label: "Override key".to_owned(),
                }],
            )]),
        )
        .expect("later plugin scope should override visible metadata");

    assert_eq!(
        registry.get("opencode"),
        Some(integration("opencode", "OpenCode override"))
    );
    assert_eq!(
        registry.methods("opencode"),
        vec![IntegrationMethod::Key {
            label: "Override key".to_owned(),
        }]
    );
}

#[test]
fn ext_007_t04_closing_plugin_scopes_reveals_prior_and_removes_only_owned_contributions() {
    let mut registry = IntegrationRegistry::new();

    registry
        .register_plugin_scope(
            1,
            declaration(vec![
                registration("shared", "Shared base", Vec::new()),
                registration("base-only", "Base only", Vec::new()),
            ]),
        )
        .expect("base plugin scope should register");
    registry
        .register_plugin_scope(
            2,
            declaration(vec![
                registration("shared", "Shared override", Vec::new()),
                registration("later-only", "Later only", Vec::new()),
            ]),
        )
        .expect("later plugin scope should register");

    assert_eq!(
        registry.get("shared"),
        Some(integration("shared", "Shared override"))
    );
    assert!(registry.close_scope(2));
    assert_eq!(
        registry.get("shared"),
        Some(integration("shared", "Shared base"))
    );
    assert_eq!(
        registry.get("base-only"),
        Some(integration("base-only", "Base only"))
    );
    assert_eq!(registry.get("later-only"), None);

    assert!(registry.close_scope(1));
    assert_eq!(registry.get("shared"), None);
    assert_eq!(registry.get("base-only"), None);
}

#[test]
fn ext_007_t05_invalid_or_overflow_declaration_is_typed_and_atomically_non_mutating() {
    assert_eq!(MAX_INTEGRATION_METHODS, 8);
    assert_eq!(MAX_INTEGRATIONS_PER_SCOPE, 32);

    let mut registry = IntegrationRegistry::new();
    registry
        .register_plugin_scope(
            1,
            declaration(vec![registration(
                "stable",
                "Stable",
                vec![IntegrationMethod::Key {
                    label: "Stable key".to_owned(),
                }],
            )]),
        )
        .expect("baseline declaration should register");
    let before_list = registry.list();
    let before_methods = registry.methods("stable");

    let invalid = registry
        .register_plugin_scope(
            2,
            declaration(vec![
                registration("would-have-committed", "Would have committed", Vec::new()),
                registration(
                    "invalid-oauth",
                    "Invalid OAuth",
                    vec![IntegrationMethod::OAuth {
                        id: String::new(),
                        label: "Missing method id".to_owned(),
                    }],
                ),
            ]),
        )
        .expect_err("whole declaration must be prevalidated before mutation");
    assert_eq!(invalid, IntegrationError::InvalidMethodId);
    assert_eq!(registry.list(), before_list);
    assert_eq!(registry.methods("stable"), before_methods);
    assert_eq!(registry.get("would-have-committed"), None);

    let too_many_methods = (0..=MAX_INTEGRATION_METHODS)
        .map(|index| IntegrationMethod::OAuth {
            id: format!("oauth-{index}"),
            label: format!("OAuth {index}"),
        })
        .collect();
    let method_overflow = registry
        .register_plugin_scope(
            3,
            declaration(vec![registration(
                "method-overflow",
                "Method overflow",
                too_many_methods,
            )]),
        )
        .expect_err("one declared integration must reuse the method-count cap");
    assert_eq!(
        method_overflow,
        IntegrationError::TooManyMethods {
            max: MAX_INTEGRATION_METHODS,
            actual: MAX_INTEGRATION_METHODS + 1,
        }
    );
    assert_eq!(registry.list(), before_list);
    assert_eq!(registry.methods("stable"), before_methods);
    assert_eq!(registry.get("method-overflow"), None);

    let overflow = (0..=MAX_INTEGRATIONS_PER_SCOPE)
        .map(|index| registration(&format!("plugin-{index}"), "Plugin", Vec::new()))
        .collect();
    let overflow = registry
        .register_plugin_scope(4, declaration(overflow))
        .expect_err("one plugin scope must reuse the integration-count cap");
    assert_eq!(
        overflow,
        IntegrationError::TooManyIntegrations {
            max: MAX_INTEGRATIONS_PER_SCOPE,
            actual: MAX_INTEGRATIONS_PER_SCOPE + 1,
        }
    );
    assert_eq!(registry.list(), before_list);
    assert_eq!(registry.methods("stable"), before_methods);
    assert_eq!(registry.get("plugin-0"), None);
}
