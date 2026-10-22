//! Black-box acceptance tests for configuration import discovery and
//! normalization.  These tests deliberately use only the public import API;
//! source fixtures are disposable and no process environment is changed.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use opencode_rk_foundation::config_import::{
    discover_claude, discover_codex, discover_opencode, merge_plans, normalize, read_sources,
    ClaudeRoots, CodexRoots, ConfigKind, OpenCodeRoots, SourceEntry, MAX_AGENT_MD_BYTES,
    MAX_DOC_BYTES, MAX_PLAN_KEYS, MAX_PLAN_SOURCES,
};
use serde_json::Value;

struct TempTree {
    root: PathBuf,
}

impl TempTree {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "opencode-rk-config-import-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create disposable fixture directory");
        Self { root }
    }

    fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.root.join(relative)
    }

    fn write(&self, relative: impl AsRef<Path>, text: &str) -> PathBuf {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture parent");
        }
        fs::write(&path, text).expect("write fixture");
        path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn string_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string field {key} in {value:?}"))
}

#[test]
fn opencode_jsonc_plural_and_legacy_sources_normalize_provider_model_and_secret_reference() {
    let text = r#"{
      // JSONC and legacy singular keys are both accepted.
      "providers": {
        "acme": {
          "name": "Acme",
          "options": {"baseURL": "https://llm.example/v1", "apiKey": "{env:ACME_API_KEY}"},
          "models": {"reasoner": {"variants": {"fast": {}, "accurate": {}}}}
        }
      },
      "provider": {"legacy": {"options": {"base_url": "https://legacy.example"}}},
      "model": "acme/reasoner"
    }"#;
    let source = normalize(ConfigKind::OpenCodeJson, text, "fixture/opencode.jsonc").unwrap();

    assert_eq!(source.base.get("model").and_then(Value::as_str), Some("acme/reasoner"));
    let acme = source.providers.get("acme").expect("plural provider imported");
    assert_eq!(string_field(acme, "base_url"), "https://llm.example/v1");
    assert_eq!(string_field(acme, "api_key_env"), "ACME_API_KEY");
    assert_eq!(
        acme.get("models").and_then(Value::as_array),
        Some(&vec![Value::String("reasoner".into())])
    );
    assert!(source.providers.contains_key("legacy"), "legacy provider key remains supported");
    // Variants are not silently fabricated or flattened into an unrelated model.
    // If the public normalized schema gains variant support, this assertion should
    // be extended to require its exact documented representation.
    assert!(!format!("{source:?}").contains("fast"));
    assert!(
        source.unsupported.iter().any(|note| note.field.contains("variants")),
        "discarded model variants need an explicit unsupported-field warning"
    );
}

#[test]
fn codex_custom_model_is_qualified_with_its_provider_and_raw_credentials_never_escape() {
    let text = r#"
model = "gpt-5"
model_provider = "custom"
[model_providers.custom]
name = "Custom Gateway"
base_url = "https://custom.example/v1"
env_key = "CUSTOM_API_KEY"
"#;
    let source = normalize(ConfigKind::CodexToml, text, "fixture/config.toml").unwrap();

    assert_eq!(source.base.get("model").and_then(Value::as_str), Some("custom/gpt-5"));
    let provider = source.providers.get("custom").expect("custom provider imported");
    assert_eq!(string_field(provider, "api_key_env"), "CUSTOM_API_KEY");

    let raw = r#"model_provider = "custom"
[model_providers.custom]
env_key = "sk-this-is-not-a-reference-and-must-not-be-output-123456789"
"#;
    let redacted = normalize(ConfigKind::CodexToml, raw, "fixture/secret.toml").unwrap();
    let debug = format!("{redacted:?}");
    assert!(!debug.contains("sk-this-is-not-a-reference"));
    assert!(redacted.unsupported.iter().any(|note| note.field.contains("env_key")));
}

#[test]
fn claude_discovery_is_user_then_shared_project_then_local_project() {
    let tree = TempTree::new("claude-order");
    let roots = ClaudeRoots {
        home: Some(tree.path("home")),
        project_dir: Some(tree.path("project")),
    };
    let entries = discover_claude(&roots);
    let paths: Vec<_> = entries.iter().map(|entry| entry.path.clone()).collect();
    assert_eq!(paths, vec![
        tree.path("home/.claude/settings.json"),
        tree.path("project/.claude/settings.json"),
        tree.path("project/.claude/settings.local.json"),
    ]);
}

#[test]
fn markdown_agent_imports_prompt_model_and_tool_list_but_reports_permission_policy() {
    let text = "---\nname: reviewer\ndescription: Reviews code\nmodel: acme/reasoner\ntools: read, grep\npermissionMode: plan\n---\nRead the diff and explain the risks.\n";
    let source = normalize(ConfigKind::ClaudeAgentMd, text, "fixture/agents/reviewer.md").unwrap();
    let agent = source.agents.get("reviewer").expect("agent name from frontmatter");
    assert_eq!(string_field(agent, "model"), "acme/reasoner");
    assert_eq!(string_field(agent, "prompt"), "Read the diff and explain the risks.");
    assert_eq!(agent.get("tools").and_then(Value::as_array).unwrap().len(), 2);
    assert!(source.unsupported.iter().any(|note| note.field.ends_with("permissionMode")));
}

#[test]
fn unsupported_hooks_and_permissions_are_explicit_and_not_executed() {
    let text = r#"{
      "model": "acme/reasoner",
      "hooks": {"PreToolUse": [{"command": "touch /tmp/must-not-run"}]},
      "permissions": {"allow": ["Bash(*)"]}
    }"#;
    let source = normalize(ConfigKind::ClaudeJson, text, "fixture/settings.json").unwrap();
    assert!(source.unsupported.iter().any(|note| note.field == "hooks"));
    assert!(source.unsupported.iter().any(|note| note.field == "permissions"));
    assert!(!Path::new("/tmp/must-not-run").exists());
}

#[test]
fn discovery_reads_bytes_unchanged_and_inline_content_does_not_fake_a_file() {
    let tree = TempTree::new("read-sources");
    let original = "{\n  \"model\": \"acme/reasoner\"\n}\n";
    let path = tree.write("opencode.json", original);
    let mut inline = SourceEntry::new(ConfigKind::OpenCodeJson, tree.path("never-created.json"));
    inline.origin = "OPENCODE_CONFIG_CONTENT".into();
    inline.text = Some(original.into());
    let docs = read_sources(&[
        SourceEntry::new(ConfigKind::OpenCodeJson, &path),
        inline,
    ]);
    assert_eq!(docs[0].text, original);
    assert_eq!(docs[1].text, original);
    assert!(docs[1].read_error.is_none());
    assert!(!tree.path("never-created.json").exists());
    assert_eq!(fs::read_to_string(path).unwrap(), original);

    let inline_entry = SourceEntry::new(ConfigKind::OpenCodeJson, tree.path("unused"));
    let mut inline_entry = inline_entry;
    inline_entry.text = Some("inline-secret-value".into());
    assert!(!format!("{inline_entry:?}").contains("inline-secret-value"));
    assert!(!format!("{:?}", docs[1]).contains("acme/reasoner"));
}

#[test]
fn normalize_rejects_oversized_json_and_markdown_documents() {
    let json = "x".repeat(MAX_DOC_BYTES + 1);
    let markdown = "x".repeat(MAX_AGENT_MD_BYTES + 1);
    assert!(normalize(ConfigKind::OpenCodeJson, &json, "huge.json").is_err());
    assert!(normalize(ConfigKind::ClaudeAgentMd, &markdown, "huge.md").is_err());
}

#[test]
fn codex_and_opencode_discovery_are_explicit_root_walks_without_environment_access() {
    let tree = TempTree::new("discovery");
    let codex = discover_codex(&CodexRoots {
        codex_home: Some(tree.path("home")),
        project_dir: Some(tree.path("project")),
    });
    assert_eq!(codex.len(), 2);
    assert_eq!(codex[0].path, tree.path("home/config.toml"));
    assert_eq!(codex[1].path, tree.path("project/.codex/config.toml"));

    let opencode = discover_opencode(&OpenCodeRoots {
        cwd: tree.path("project/work"),
        global_dir: None,
        explicit_config: None,
        explicit_dir: None,
        explicit_content: Some("{}".into()),
        disable_project: true,
    });
    assert_eq!(opencode.last().unwrap().origin, "OPENCODE_CONFIG_CONTENT");
    assert_eq!(opencode.last().unwrap().text.as_deref(), Some("{}"));
}

#[test]
fn merge_overrides_provider_endpoint_by_leaf_and_preserves_global_credentials_and_models() {
    let global = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"providers":{"acme":{"options":{"baseURL":"https://global.example","apiKey":"{env:ACME_KEY}"},"models":{"gpt-5.6/vendor/name":{}}}}}"#,
        "global/opencode.json",
    )
    .unwrap();
    let project = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"providers":{"acme":{"options":{"baseURL":"https://project.example"}}}}"#,
        "project/opencode.json",
    )
    .unwrap();
    let plan = merge_plans(&[global, project]).unwrap();
    let provider = plan
        .value
        .get("providers")
        .and_then(|providers| providers.get("acme"))
        .expect("merged provider");
    assert_eq!(string_field(provider, "base_url"), "https://project.example");
    assert_eq!(string_field(provider, "api_key_env"), "ACME_KEY");
    assert_eq!(
        provider.get("models").and_then(Value::as_array),
        Some(&vec![Value::String("gpt-5.6/vendor/name".into())])
    );
    assert_eq!(
        plan.provenance.get("providers.acme").map(String::as_str),
        Some("project/opencode.json")
    );
}

#[test]
fn merge_records_unequal_origins_but_identical_values_are_not_conflicts() {
    let first = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"model":"gpt-5.6/vendor/name"}"#,
        "first.json",
    )
    .unwrap();
    let second = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"model":"other/model"}"#,
        "second.json",
    )
    .unwrap();
    let conflict_plan = merge_plans(&[first.clone(), second]).unwrap();
    let conflict = conflict_plan
        .conflicts
        .iter()
        .find(|conflict| {
            conflict.origins
                == vec!["first.json".to_string(), "second.json".to_string()]
        })
        .expect("unequal values record both contributing origins");
    assert_eq!(conflict.field, "model");
    assert!(!conflict.credential);

    let identical = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"model":"gpt-5.6/vendor/name"}"#,
        "second-identical.json",
    )
    .unwrap();
    let identical_plan = merge_plans(&[first, identical]).unwrap();
    assert!(identical_plan.conflicts.is_empty());
    assert_eq!(
        identical_plan.value.get("model").and_then(Value::as_str),
        Some("gpt-5.6/vendor/name")
    );
}

#[test]
fn merge_agents_overrides_prompt_and_tools_without_dropping_sibling_fields() {
    let global = normalize(
        ConfigKind::OpenCodeAgentMd,
        "---\nname: reviewer\ndescription: global description\nmodel: gpt-5.6/vendor/name\ntools: read, grep\n---\nGlobal prompt\n",
        "global/reviewer.md",
    )
    .unwrap();
    let project = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"agents":{"reviewer":{"prompt":"Project prompt","tools":["read","edit"]}}}"#,
        "project/opencode.json",
    )
    .unwrap();
    let plan = merge_plans(&[global, project]).unwrap();
    let agent = plan
        .value
        .get("agents")
        .and_then(|agents| agents.get("reviewer"))
        .expect("merged agent");
    assert_eq!(string_field(agent, "prompt"), "Project prompt");
    assert_eq!(string_field(agent, "description"), "global description");
    assert_eq!(string_field(agent, "model"), "gpt-5.6/vendor/name");
    assert_eq!(agent.get("tools").and_then(Value::as_array).unwrap().len(), 2);
}

#[test]
fn merge_does_not_mutate_normalized_source_inputs_and_enforces_plan_bounds() {
    let source = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"providers":{"acme":{"options":{"apiKey":"{env:ACME_KEY}"}}}}"#,
        "unchanged.json",
    )
    .unwrap();
    let before = format!("{source:?}");
    let sources = vec![source.clone()];
    let _ = merge_plans(&sources).unwrap();
    assert_eq!(format!("{:?}", sources[0]), before);

    let too_many: Vec<_> = (0..=MAX_PLAN_SOURCES)
        .map(|index| normalize(ConfigKind::OpenCodeJson, "{}", &format!("source-{index}" )).unwrap())
        .collect();
    assert!(merge_plans(&too_many).is_err());

    let mut many_keys = normalize(ConfigKind::OpenCodeJson, "{}", "many-keys").unwrap();
    for index in 0..=MAX_PLAN_KEYS {
        many_keys.models.insert(format!("gpt-{index}"), Value::Null);
    }
    assert!(merge_plans(&[many_keys]).is_err());
}

#[test]
fn merge_uses_nested_runtime_maps_and_recursively_merges_three_sources_by_leaf() {
    let global = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"providers":{"acme":{"name":"Global","options":{"baseURL":"https://global.example","apiKey":"{env:GLOBAL_KEY}"}}},"agents":{"reviewer":{"description":"Global reviewer","model":"gpt-5.6/vendor/name","prompt":"global prompt","tools":["read"]}}}"#,
        "global.json",
    )
    .unwrap();
    let project = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"providers":{"acme":{"name":"Project","options":{"baseURL":"https://project.example"}}},"agents":{"reviewer":{"prompt":"project prompt","tools":["read","grep"]}},"commands":{"review":{"template":"$EDITOR"}}}"#,
        "project.json",
    )
    .unwrap();
    let local = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"providers":{"acme":{"options":{"baseURL":"https://local.example"}}},"agents":{"reviewer":{"tools":["read","edit"]}}}"#,
        "local.json",
    )
    .unwrap();
    let plan = merge_plans(&[global, project, local]).unwrap();

    let provider = plan
        .value
        .get("providers")
        .and_then(|providers| providers.get("acme"))
        .expect("runtime provider map is nested");
    assert_eq!(string_field(provider, "base_url"), "https://local.example");
    assert_eq!(string_field(provider, "api_key_env"), "GLOBAL_KEY");
    assert_eq!(string_field(provider, "name"), "Project");
    assert!(plan.value.get("providers.acme").is_none());

    let agent = plan
        .value
        .get("agents")
        .and_then(|agents| agents.get("reviewer"))
        .expect("runtime agent map is nested");
    assert_eq!(string_field(agent, "description"), "Global reviewer");
    assert_eq!(string_field(agent, "prompt"), "project prompt");
    assert_eq!(
        agent.get("tools").and_then(Value::as_array),
        Some(&vec![Value::String("read".into()), Value::String("edit".into())])
    );
    assert!(plan.value.get("agents.reviewer").is_none());
    assert_eq!(
        plan.value
            .get("commands")
            .and_then(|commands| commands.get("review"))
            .and_then(|command| command.get("template"))
            .and_then(Value::as_str),
        Some("$EDITOR")
    );

    let endpoint_conflict = plan
        .conflicts
        .iter()
        .find(|conflict| conflict.field == "providers.acme.base_url")
        .expect("endpoint conflict is reported at its nested leaf");
    assert_eq!(
        endpoint_conflict.origins,
        vec!["global.json", "project.json", "local.json"]
    );
    assert_eq!(
        plan.provenance.get("providers.acme.base_url").map(String::as_str),
        Some("local.json")
    );
}

#[test]
fn merge_preserves_model_map_as_nested_map_with_exact_dotted_slash_key() {
    let mut source = normalize(ConfigKind::OpenCodeJson, "{}", "models.json").unwrap();
    source.models.insert(
        "gpt-5.6/vendor/name".into(),
        serde_json::json!({"provider":"vendor"}),
    );
    let plan = merge_plans(&[source]).unwrap();
    assert_eq!(
        plan.value
            .get("models")
            .and_then(|models| models.get("gpt-5.6/vendor/name"))
            .and_then(|model| model.get("provider"))
            .and_then(Value::as_str),
        Some("vendor")
    );
    assert!(plan.value.get("models.gpt-5.6/vendor/name").is_none());
}

#[test]
fn merge_keeps_literal_model_ids_separate_when_one_id_is_a_prefix_of_another() {
    // These are two literal catalogue identifiers, not a parent object and a
    // child path.  The public model map is keyed by exact IDs, so changing
    // `a` must not clear provenance for the independent ID `a.b`.
    let mut first = normalize(ConfigKind::OpenCodeJson, "{}", "first-models.json").unwrap();
    first.models.insert("a".into(), Value::String("first-a".into()));
    first
        .models
        .insert("a.b".into(), Value::String("first-a-dot-b".into()));

    let mut second = normalize(ConfigKind::OpenCodeJson, "{}", "second-models.json").unwrap();
    second.models.insert("a".into(), Value::String("second-a".into()));

    let mut third = normalize(ConfigKind::OpenCodeJson, "{}", "third-models.json").unwrap();
    third
        .models
        .insert("a.b".into(), Value::String("third-a-dot-b".into()));

    let plan = merge_plans(&[first, second, third]).unwrap();
    let models = plan
        .value
        .get("models")
        .and_then(Value::as_object)
        .expect("runtime models map is nested");
    assert_eq!(models.get("a").and_then(Value::as_str), Some("second-a"));
    assert_eq!(
        models.get("a.b").and_then(Value::as_str),
        Some("third-a-dot-b")
    );

    // The conflict for the literal ID `a.b` must retain the first and third
    // origins.  The middle source changed only the separate literal ID `a`.
    let conflict = plan
        .conflicts
        .iter()
        .find(|conflict| conflict.field == "models.a%2Eb")
        .expect("literal a.b change must be a conflict");
    assert_eq!(
        conflict.origins,
        vec!["first-models.json", "third-models.json"]
    );
    assert!(!conflict.origins.iter().any(|origin| origin == "second-models.json"));
    assert_eq!(
        plan.provenance.get("models.a%2Eb").map(String::as_str),
        Some("third-models.json")
    );
    assert_eq!(
        plan.provenance.get("models.a").map(String::as_str),
        Some("second-models.json")
    );
}

#[test]
fn merge_metadata_escapes_percent_and_dot_for_distinct_literal_model_ids() {
    let ids = ["a.b", "a%2Eb", "a%252Eb"];
    // Establish all three literal IDs together first, then override each ID
    // independently.  Distinct sources make each expected conflict real while
    // preserving the collision-injection coverage.
    let mut initial = normalize(ConfigKind::OpenCodeJson, "{}", "initial.json").unwrap();
    for (index, id) in ids.iter().enumerate() {
        initial
            .models
            .insert((*id).into(), Value::String(format!("initial-{index}")));
    }
    let mut dot_override = normalize(ConfigKind::OpenCodeJson, "{}", "dot-override.json").unwrap();
    dot_override.models.insert("a.b".into(), Value::String("dot-updated".into()));
    let mut encoded_override = normalize(ConfigKind::OpenCodeJson, "{}", "encoded-override.json").unwrap();
    encoded_override.models.insert("a%2Eb".into(), Value::String("encoded-updated".into()));
    let mut double_override = normalize(ConfigKind::OpenCodeJson, "{}", "double-override.json").unwrap();
    double_override.models.insert("a%252Eb".into(), Value::String("double-updated".into()));

    let plan = merge_plans(&[initial, dot_override, encoded_override, double_override]).unwrap();
    let models = plan.value.get("models").and_then(Value::as_object).unwrap();
    assert_eq!(models.get("a.b").and_then(Value::as_str), Some("dot-updated"));
    assert_eq!(models.get("a%2Eb").and_then(Value::as_str), Some("encoded-updated"));
    assert_eq!(models.get("a%252Eb").and_then(Value::as_str), Some("double-updated"));

    assert_eq!(plan.provenance.get("models.a%2Eb").map(String::as_str), Some("dot-override.json"));
    assert_eq!(plan.provenance.get("models.a%252Eb").map(String::as_str), Some("encoded-override.json"));
    assert_eq!(plan.provenance.get("models.a%25252Eb").map(String::as_str), Some("double-override.json"));

    assert_eq!(plan.conflicts.len(), 3);
    let dot_conflict = plan.conflicts.iter().find(|conflict| conflict.field == "models.a%2Eb").unwrap();
    assert_eq!(dot_conflict.origins, vec!["initial.json", "dot-override.json"]);
    let encoded_conflict = plan.conflicts.iter().find(|conflict| conflict.field == "models.a%252Eb").unwrap();
    assert_eq!(encoded_conflict.origins, vec!["initial.json", "encoded-override.json"]);
    let double_conflict = plan.conflicts.iter().find(|conflict| conflict.field == "models.a%25252Eb").unwrap();
    assert_eq!(double_conflict.origins, vec!["initial.json", "double-override.json"]);
}

#[test]
fn merge_distinguishes_nested_model_field_from_literal_dotted_model_id() {
    let mut first = normalize(ConfigKind::OpenCodeJson, "{}", "nested-first.json").unwrap();
    first.models.insert("a".into(), serde_json::json!({"b": "child-first"}));
    first.models.insert("a.b".into(), Value::String("literal-first".into()));

    let mut second = normalize(ConfigKind::OpenCodeJson, "{}", "nested-second.json").unwrap();
    second.models.insert("a".into(), serde_json::json!({"b": "child-second"}));

    let mut third = normalize(ConfigKind::OpenCodeJson, "{}", "nested-third.json").unwrap();
    third.models.insert("a.b".into(), Value::String("literal-third".into()));

    let plan = merge_plans(&[first, second, third]).unwrap();
    let nested = plan.value.get("models").and_then(|models| models.get("a")).unwrap();
    assert_eq!(nested.get("b").and_then(Value::as_str), Some("child-second"));
    assert_eq!(
        plan.value.get("models").and_then(|models| models.get("a.b")).and_then(Value::as_str),
        Some("literal-third")
    );
    let child_conflict = plan.conflicts.iter().find(|conflict| conflict.field == "models.a.b").unwrap();
    assert_eq!(child_conflict.origins, vec!["nested-first.json", "nested-second.json"]);
    let literal_conflict = plan.conflicts.iter().find(|conflict| conflict.field == "models.a%2Eb").unwrap();
    assert_eq!(literal_conflict.origins, vec!["nested-first.json", "nested-third.json"]);
    assert_eq!(plan.provenance.get("models.a.b").map(String::as_str), Some("nested-second.json"));
    assert_eq!(plan.provenance.get("models.a%2Eb").map(String::as_str), Some("nested-third.json"));
}

#[test]
fn merge_credential_conflict_flag_is_field_sensitive_not_model_id_sensitive() {
    let mut model_first = normalize(ConfigKind::OpenCodeJson, "{}", "model-first.json").unwrap();
    model_first.models.insert("apikey".into(), Value::String("one".into()));
    let mut model_second = normalize(ConfigKind::OpenCodeJson, "{}", "model-second.json").unwrap();
    model_second.models.insert("apikey".into(), Value::String("two".into()));
    let model_plan = merge_plans(&[model_first, model_second]).unwrap();
    let model_conflict = model_plan.conflicts.iter().find(|conflict| conflict.field == "models.apikey").unwrap();
    assert!(!model_conflict.credential);

    let provider_first = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"providers":{"acme":{"options":{"baseURL":"https://first.example","apiKey":"{env:FIRST_KEY}"}}}}"#,
        "provider-first.json",
    )
    .unwrap();
    let provider_second = normalize(
        ConfigKind::OpenCodeJson,
        r#"{"providers":{"acme":{"options":{"baseURL":"https://second.example","apiKey":"{env:SECOND_KEY}"}}}}"#,
        "provider-second.json",
    )
    .unwrap();
    let provider_plan = merge_plans(&[provider_first, provider_second]).unwrap();
    let provider = provider_plan
        .value
        .get("providers")
        .and_then(|providers| providers.get("acme"))
        .expect("canonical provider remains in runtime map");
    assert_eq!(string_field(provider, "api_key_env"), "SECOND_KEY");
    assert!(provider.get("api_key").is_none());
    let plan_source_provider = provider_plan.sources[1]
        .providers
        .get("acme")
        .expect("canonical provider is retained in source preview");
    assert_eq!(string_field(plan_source_provider, "api_key_env"), "SECOND_KEY");
    assert!(plan_source_provider.get("api_key").is_none());
    let provider_conflict = provider_plan
        .conflicts
        .iter()
        .find(|conflict| conflict.field == "providers.acme.api_key_env")
        .unwrap();
    assert!(provider_conflict.credential);
    assert_eq!(provider_conflict.origins, vec!["provider-first.json", "provider-second.json"]);
}

#[test]
fn merge_canonicalizes_caller_constructed_provider_credentials_at_public_boundary() {
    let mut first = normalize(ConfigKind::OpenCodeJson, "{}", "constructed-first.json").unwrap();
    first.providers.insert(
        "acme".into(),
        serde_json::json!({
            "id": "acme",
            "base_url": "https://first.example",
            "api_key": "{env:FIRST_KEY}"
        }),
    );
    let mut second = normalize(ConfigKind::OpenCodeJson, "{}", "constructed-second.json").unwrap();
    second.providers.insert(
        "acme".into(),
        serde_json::json!({
            "id": "acme",
            "base_url": "https://second.example",
            "api_key": "{env:SECOND_KEY}"
        }),
    );

    let plan = merge_plans(&[first, second]).unwrap();
    let provider = plan
        .value
        .get("providers")
        .and_then(|providers| providers.get("acme"))
        .expect("constructed provider is nested in runtime output");
    assert_eq!(string_field(provider, "api_key_env"), "SECOND_KEY");
    assert!(provider.get("api_key").is_none());
    let preview = plan.sources[1].providers.get("acme").unwrap();
    assert_eq!(string_field(preview, "api_key_env"), "SECOND_KEY");
    assert!(preview.get("api_key").is_none());

    let conflict = plan
        .conflicts
        .iter()
        .find(|conflict| conflict.field == "providers.acme.api_key_env")
        .expect("canonical credential field conflict");
    assert!(conflict.credential);
    assert_eq!(
        conflict.origins,
        vec!["constructed-first.json", "constructed-second.json"]
    );
}

#[test]
fn merge_does_not_expose_caller_constructed_raw_provider_credentials() {
    let raw_secret = "sk-raw-credential-must-not-escape-123456789";
    let mut source = normalize(ConfigKind::OpenCodeJson, "{}", "constructed-raw.json").unwrap();
    source.providers.insert(
        "acme".into(),
        serde_json::json!({
            "id": "acme",
            "base_url": "https://raw.example",
            "api_key": raw_secret
        }),
    );

    let plan = merge_plans(&[source]).unwrap();
    let serialized = serde_json::to_string(&plan).unwrap();
    let debug = format!("{plan:?}");
    assert!(!serialized.contains(raw_secret));
    assert!(!debug.contains(raw_secret));
    let provider = plan
        .value
        .get("providers")
        .and_then(|providers| providers.get("acme"))
        .expect("raw provider remains represented without its credential");
    for field in ["api_key", "apiKey", "api_key_env"] {
        assert!(provider.get(field).is_none(), "runtime must not expose {field}");
    }
    let preview = plan.sources[0].providers.get("acme").unwrap();
    for field in ["api_key", "apiKey", "api_key_env"] {
        assert!(preview.get(field).is_none(), "provider preview must not expose {field}");
    }
    assert!(!serialized.contains("[redacted]"));
    assert!(!debug.contains("[redacted]"));
    let diagnostic = plan
        .unsupported
        .iter()
        .find(|note| note.origin == "constructed-raw.json")
        .expect("raw credential must produce an unsupported diagnostic");
    assert!(diagnostic.field.starts_with("providers.acme"));
    assert!(
        diagnostic.reason.to_ascii_lowercase().contains("credential")
            || diagnostic.reason.to_ascii_lowercase().contains("secret")
            || diagnostic.reason.to_ascii_lowercase().contains("redact")
    );
}

#[test]
fn normalize_provider_ids_match_the_native_import_consumer_contract() {
    let origin = "provider-consumer-ids.json";
    let source = normalize(
        ConfigKind::OpenCodeJson,
        r#"{
          "providers": {
            "acme": {"options": {"baseURL": "https://acme.example"}},
            "Acme": {"options": {"baseURL": "https://uppercase.example"}},
            "-leading": {"options": {"baseURL": "https://leading.example"}}
          }
        }"#,
        origin,
    )
    .unwrap();

    // `acme` is accepted by providers::valid_imported_id.  The other two
    // entries must not reach the runtime provider catalogue: preserving them
    // would make native import reject an otherwise usable mixed source.
    assert_eq!(source.providers.len(), 1);
    assert!(source.providers.contains_key("acme"));
    assert!(!source.providers.contains_key("Acme"));
    assert!(!source.providers.contains_key("-leading"));

    let expected_reason = "provider identifier is not valid for native imported provider consumers";
    assert!(source.unsupported.iter().any(|note| {
        note.origin == origin
            && note.field == "providers.Acme"
            && note.reason == expected_reason
    }));
    assert!(source.unsupported.iter().any(|note| {
        note.origin == origin
            && note.field == "providers.-leading"
            && note.reason == expected_reason
    }));
}

#[test]
fn opencode_provider_root_fields_are_reported_without_leaking_raw_credentials() {
    let origin = "root-provider-fields.json";
    let raw_credential = "sk-root-secret-sentinel-that-must-not-escape-123456789";
    let source = normalize(
        ConfigKind::OpenCodeJson,
        &format!(
            r#"{{
              "providers": {{
                "acme": {{
                  "apiKey": "{raw_credential}",
                  "baseURL": "https://root.example/v1",
                  "unknownField": "not imported",
                  "options": {{
                    "baseURL": "https://options.example/v1",
                    "apiKey": "{{env:ACME_API_KEY}}"
                  }}
                }}
              }}
            }}"#
        ),
        origin,
    )
    .unwrap();

    for field in [
        "providers.acme.apiKey",
        "providers.acme.baseURL",
        "providers.acme.unknownField",
    ] {
        let diagnostic = source
            .unsupported
            .iter()
            .find(|note| note.origin == origin && note.field == field)
            .unwrap_or_else(|| panic!("missing field-specific unsupported diagnostic for {field}"));
        assert!(!diagnostic.reason.is_empty());
        assert!(!diagnostic.reason.contains(raw_credential));
    }
    assert!(!source.unsupported.iter().any(|note| {
        note.origin == origin && note.field == "providers.acme.options.apiKey"
    }));

    let provider = source.providers.get("acme").expect("provider imported");
    assert_eq!(string_field(provider, "base_url"), "https://options.example/v1");
    assert_eq!(string_field(provider, "api_key_env"), "ACME_API_KEY");
    assert!(provider.get("api_key").is_none());

    let plan = merge_plans(&[source.clone()]).unwrap();
    let serialized_value = serde_json::to_string(&plan.value).unwrap();
    let serialized_sources = serde_json::to_string(&plan.sources).unwrap();
    let serialized_diagnostics = serde_json::to_string(&plan.unsupported).unwrap();
    assert!(!serialized_value.contains(raw_credential));
    assert!(!serialized_sources.contains(raw_credential));
    assert!(!serialized_diagnostics.contains(raw_credential));
    assert!(!format!("{source:?}").contains(raw_credential));
    assert!(!serde_json::to_string(&plan).unwrap().contains(raw_credential));
}
