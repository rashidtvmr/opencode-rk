use opencode_rk_sessions::reference::{
    PluginReferenceDeclaration, PluginReferenceRegistration, ReferenceError, ReferenceRegistry,
    ReferenceSource, MAX_REFERENCES_PER_SCOPE, MAX_REFERENCE_ALIAS_BYTES,
    MAX_REFERENCE_METADATA_BYTES, MAX_REFERENCE_SCOPES,
};

fn local(
    alias: &str,
    path: &str,
    description: Option<&str>,
    hidden: Option<bool>,
) -> PluginReferenceRegistration {
    PluginReferenceRegistration {
        alias: alias.to_owned(),
        source: ReferenceSource::Local {
            path: path.to_owned(),
            description: description.map(str::to_owned),
            hidden,
        },
    }
}

fn git(
    alias: &str,
    repository: &str,
    branch: Option<&str>,
    description: Option<&str>,
    hidden: Option<bool>,
) -> PluginReferenceRegistration {
    PluginReferenceRegistration {
        alias: alias.to_owned(),
        source: ReferenceSource::Git {
            repository: repository.to_owned(),
            branch: branch.map(str::to_owned),
            description: description.map(str::to_owned),
            hidden,
        },
    }
}

fn declaration(references: Vec<PluginReferenceRegistration>) -> PluginReferenceDeclaration {
    PluginReferenceDeclaration { references }
}

#[test]
fn ext_003_t01_local_metadata_registration_is_bounded_and_deterministic() {
    let mut registry = ReferenceRegistry::new();
    let alpha = local(
        "alpha",
        "/workspace/docs",
        Some("Use for API documentation"),
        Some(true),
    );
    let zeta = local("zeta", "/workspace/zeta", None, Some(false));

    registry
        .register_plugin_scope(10, declaration(vec![zeta.clone(), alpha.clone()]))
        .unwrap();

    assert_eq!(registry.list(), vec![alpha, zeta]);
}

#[test]
fn ext_003_t02_git_metadata_is_retained_as_inert_source_data() {
    let mut registry = ReferenceRegistry::new();
    let sdk = git(
        "sdk",
        "github.com/example/sdk",
        Some("release/v2"),
        Some("Use for SDK implementation details"),
        Some(true),
    );

    registry
        .register_plugin_scope(20, declaration(vec![sdk.clone()]))
        .unwrap();

    assert_eq!(registry.list(), vec![sdk]);
}

#[test]
fn ext_003_t03_later_scope_overrides_alias_and_close_reveals_prior_metadata() {
    let mut registry = ReferenceRegistry::new();
    let original = local("docs", "/workspace/docs-v1", Some("Original docs"), None);
    let override_value = git(
        "docs",
        "github.com/example/docs",
        Some("main"),
        Some("Replacement docs"),
        Some(false),
    );

    registry
        .register_plugin_scope(1, declaration(vec![original.clone()]))
        .unwrap();
    registry
        .register_plugin_scope(2, declaration(vec![override_value.clone()]))
        .unwrap();

    assert_eq!(registry.list(), vec![override_value]);
    assert!(registry.close_scope(2));
    assert_eq!(registry.list(), vec![original]);
    assert!(!registry.close_scope(2));
    assert!(registry.close_scope(1));
    assert!(registry.list().is_empty());
}

#[test]
fn ext_003_t04_add_remove_list_and_close_mutate_only_the_owning_scope() {
    let mut registry = ReferenceRegistry::new();
    let base = local("base", "/workspace/base", Some("Base reference"), None);
    let shared = local("shared", "/workspace/shared", Some("Shared base"), None);
    let scoped = local("scoped", "/workspace/scoped", Some("Scoped only"), None);
    let shared_override = git(
        "shared",
        "github.com/example/shared",
        None,
        Some("Scoped override"),
        None,
    );

    registry
        .register_plugin_scope(1, declaration(vec![shared.clone(), base.clone()]))
        .unwrap();
    registry.add(2, scoped.clone()).unwrap();
    registry.add(2, shared_override).unwrap();

    assert_eq!(
        registry.list(),
        vec![
            base.clone(),
            scoped.clone(),
            git(
                "shared",
                "github.com/example/shared",
                None,
                Some("Scoped override"),
                None,
            )
        ]
    );

    assert!(!registry.remove(2, "base").unwrap());
    assert!(registry.remove(2, "shared").unwrap());
    assert_eq!(registry.list(), vec![base.clone(), scoped.clone(), shared]);

    assert!(registry.remove(2, "scoped").unwrap());
    assert_eq!(
        registry.list(),
        vec![
            base.clone(),
            local("shared", "/workspace/shared", Some("Shared base"), None,)
        ]
    );

    assert!(registry.close_scope(2));
    assert_eq!(
        registry.list(),
        vec![
            base,
            local("shared", "/workspace/shared", Some("Shared base"), None,)
        ]
    );
    assert!(registry.close_scope(1));
    assert!(registry.list().is_empty());
}

#[test]
fn ext_003_t05_invalid_or_overflowing_declarations_are_typed_and_atomic() {
    assert_eq!(MAX_REFERENCE_SCOPES, 8);
    assert_eq!(MAX_REFERENCES_PER_SCOPE, 32);
    assert_eq!(MAX_REFERENCE_ALIAS_BYTES, 128);
    assert_eq!(MAX_REFERENCE_METADATA_BYTES, 4 * 1024);

    let mut registry = ReferenceRegistry::new();
    let baseline = local("base", "/workspace/base", Some("Baseline"), None);
    registry
        .register_plugin_scope(1, declaration(vec![baseline.clone()]))
        .unwrap();

    let invalid_alias = "bad alias".to_owned();
    assert_eq!(
        registry.register_plugin_scope(
            2,
            declaration(vec![
                local("valid", "/workspace/valid", None, None),
                local(&invalid_alias, "/workspace/bad", None, None),
            ]),
        ),
        Err(ReferenceError::InvalidAlias {
            alias: invalid_alias.clone(),
        })
    );
    assert_eq!(registry.list(), vec![baseline.clone()]);

    assert_eq!(
        registry.register_plugin_scope(
            2,
            declaration(vec![
                local("duplicate", "/workspace/one", None, None),
                local("duplicate", "/workspace/two", None, None),
            ]),
        ),
        Err(ReferenceError::DuplicateAlias {
            alias: "duplicate".to_owned(),
        })
    );
    assert_eq!(registry.list(), vec![baseline.clone()]);

    let oversized_alias = "a".repeat(MAX_REFERENCE_ALIAS_BYTES + 1);
    assert_eq!(
        registry.register_plugin_scope(
            2,
            declaration(vec![local(
                &oversized_alias,
                "/workspace/oversized-alias",
                None,
                None,
            )]),
        ),
        Err(ReferenceError::TextTooLong {
            field: "alias",
            max: MAX_REFERENCE_ALIAS_BYTES,
            actual: MAX_REFERENCE_ALIAS_BYTES + 1,
        })
    );
    assert_eq!(registry.list(), vec![baseline.clone()]);

    let oversized_path = "p".repeat(MAX_REFERENCE_METADATA_BYTES + 1);
    assert_eq!(
        registry.register_plugin_scope(
            2,
            declaration(vec![local("oversized", &oversized_path, None, None)]),
        ),
        Err(ReferenceError::TextTooLong {
            field: "path",
            max: MAX_REFERENCE_METADATA_BYTES,
            actual: MAX_REFERENCE_METADATA_BYTES + 1,
        })
    );
    assert_eq!(registry.list(), vec![baseline]);

    let mut scope_limited = ReferenceRegistry::new();
    for scope_id in 0..MAX_REFERENCE_SCOPES {
        scope_limited
            .register_plugin_scope(
                scope_id as u64,
                declaration(vec![local(
                    &format!("scope-{scope_id}"),
                    "/workspace/ref",
                    None,
                    None,
                )]),
            )
            .unwrap();
    }
    let before_scope_overflow = scope_limited.list();
    assert_eq!(
        scope_limited.register_plugin_scope(
            MAX_REFERENCE_SCOPES as u64,
            declaration(vec![local("overflow", "/workspace/ref", None, None)]),
        ),
        Err(ReferenceError::TooManyScopes {
            max: MAX_REFERENCE_SCOPES,
            actual: MAX_REFERENCE_SCOPES + 1,
        })
    );
    assert_eq!(scope_limited.list(), before_scope_overflow);

    let mut reference_limited = ReferenceRegistry::new();
    let too_many_references = (0..=MAX_REFERENCES_PER_SCOPE)
        .map(|index| local(&format!("ref-{index}"), "/workspace/ref", None, None))
        .collect();
    assert_eq!(
        reference_limited.register_plugin_scope(1, declaration(too_many_references)),
        Err(ReferenceError::TooManyReferences {
            max: MAX_REFERENCES_PER_SCOPE,
            actual: MAX_REFERENCES_PER_SCOPE + 1,
        })
    );
    assert!(reference_limited.list().is_empty());
}
