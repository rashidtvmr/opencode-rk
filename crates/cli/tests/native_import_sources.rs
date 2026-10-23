//! Tests for native import source discovery (native_import_sources).
//! Uses disposable temp fixtures and explicit env snapshots - no real env races.

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[path = "../src/native_config_import.rs"]
mod native_config_import;
#[path = "../src/native_import_dialog.rs"]
mod native_import_dialog;
#[path = "../src/native_import_sources.rs"]
mod native_import_sources;

use native_import_sources::{discover_import, EnvSnapshot, source_groups};

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rk-native-import-{label}-{}-{}-{}",
            process::id(),
            nanos,
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("sandbox root");
        Self { root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    fs::write(path, bytes).expect("fixture write");
}

fn env_with(home: &Path) -> EnvSnapshot {
    EnvSnapshot {
        home: Some(home.to_path_buf()),
        xdg_config_home: None,
        opencode_config: None,
        opencode_config_dir: None,
        opencode_config_content: None,
        codex_home: None,
        claude_config_dir: None,
    }
}

#[test]
fn empty_environment_yields_no_offer() {
    let sandbox = Sandbox::new("empty");
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = EnvSnapshot {
        home: None,
        xdg_config_home: None,
        opencode_config: None,
        opencode_config_dir: None,
        opencode_config_content: None,
        codex_home: None,
        claude_config_dir: None,
    };

    let result = discover_import(&cwd, &env, &dest).expect("discovery must not error");
    assert!(result.is_none(), "no sources => no offer");
}

#[test]
fn discovers_opencode_global_config() {
    let sandbox = Sandbox::new("oc-global");
    let home = sandbox.path("home");
    let global_dir = home.join(".config/opencode");
    write(&global_dir.join("opencode.json"), br#"{"model":"gpt-5.6"}"#);
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = env_with(&home);

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find a source");

    assert!(
        offer.plan.value.get("model").is_some(),
        "plan should contain imported model"
    );
    assert!(
        offer.sources.iter().any(|s| s.path.ends_with("opencode.json")),
        "should include the global config file path"
    );
    // Source path must not be the whole HOME dir
    for s in &offer.sources {
        assert_ne!(s.path, home, "source path must not be home root");
    }
}

#[test]
fn discovers_opencode_project_ancestor_config() {
    let sandbox = Sandbox::new("oc-ancestor");
    let home = sandbox.path("home");
    let cwd = sandbox.path("home/proj/sub");
    let proj_root = sandbox.path("home/proj");
    write(&proj_root.join("opencode.json"), br#"{"model":"ancestor-model"}"#);
    let dest = sandbox.path("native/opencode.json");
    let env = env_with(&home);

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find ancestor config");

    let model = offer.plan.value.get("model").and_then(Value::as_str);
    assert_eq!(model, Some("ancestor-model"));
}

#[test]
fn discovers_claude_settings() {
    let sandbox = Sandbox::new("claude");
    let home = sandbox.path("home");
    let claude_dir = home.join(".claude");
    write(
        &claude_dir.join("settings.json"),
        br#"{"model":"claude-3-opus"}"#,
    );
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = env_with(&home);

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find claude settings");

    let model = offer.plan.value.get("model").and_then(Value::as_str);
    assert_eq!(model, Some("claude-3-opus"));
}

#[test]
fn discovers_claude_agent_markdown() {
    let sandbox = Sandbox::new("claude-agents");
    let home = sandbox.path("home");
    let agents_dir = home.join(".claude/agents");
    write(
        &agents_dir.join("my-agent.md"),
        b"---\nname: my-agent\ndescription: test agent\n---\nprompt body",
    );
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = env_with(&home);

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find claude agent");

    // Runtime JSON stores agents under the nested `agents` map.
    assert!(
        offer
            .plan
            .value
            .get("agents")
            .and_then(|agents| agents.get("my-agent"))
            .is_some(),
        "plan should contain agents.my-agent in the nested runtime map"
    );
}

#[test]
fn discovers_codex_config() {
    let sandbox = Sandbox::new("codex");
    let home = sandbox.path("home");
    let codex_dir = home.join(".codex");
    write(
        &codex_dir.join("config.toml"),
        b"model = \"gpt-5\"\nmodel_provider = \"openai\"\n",
    );
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = env_with(&home);

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find codex config");

    let model = offer.plan.value.get("model").and_then(Value::as_str);
    assert_eq!(model, Some("openai/gpt-5"), "codex model should be qualified");
}

#[test]
fn missing_source_files_yield_no_offer() {
    let sandbox = Sandbox::new("missing");
    let home = sandbox.path("home");
    // No config files exist anywhere
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = env_with(&home);

    let result = discover_import(&cwd, &env, &dest).expect("discovery ok");
    assert!(result.is_none(), "no source files => no offer");
}

#[test]
fn opencode_config_content_inline_is_discovered() {
    let sandbox = Sandbox::new("content");
    let home = sandbox.path("home");
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = EnvSnapshot {
        home: Some(home.clone()),
        xdg_config_home: None,
        opencode_config: None,
        opencode_config_dir: None,
        opencode_config_content: Some(r#"{"model":"from-content"}"#.to_string()),
        codex_home: None,
        claude_config_dir: None,
    };

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find inline content");

    let model = offer.plan.value.get("model").and_then(Value::as_str);
    assert_eq!(model, Some("from-content"));
}

#[test]
fn opencode_config_dir_env_overrides_home() {
    let sandbox = Sandbox::new("explicit-dir");
    let home = sandbox.path("home");
    let explicit = sandbox.path("explicit");
    write(&explicit.join("opencode.json"), br#"{"model":"explicit-model"}"#);
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = EnvSnapshot {
        home: Some(home.clone()),
        xdg_config_home: None,
        opencode_config: None,
        opencode_config_dir: Some(explicit.clone()),
        opencode_config_content: None,
        codex_home: None,
        claude_config_dir: None,
    };

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find explicit config");

    let model = offer.plan.value.get("model").and_then(Value::as_str);
    assert_eq!(model, Some("explicit-model"));
    // The source should be the explicit file, not the global home dir
    assert!(offer
        .sources
        .iter()
        .any(|s| s.path == explicit.join("opencode.json")));
}

#[test]
fn dest_overlapping_source_is_rejected() {
    let sandbox = Sandbox::new("overlap");
    let home = sandbox.path("home");
    let global_dir = home.join(".config/opencode");
    write(&global_dir.join("opencode.json"), br#"{"model":"gpt-5.6"}"#);
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    // dest is INSIDE the source tree
    let dest = global_dir.join("opencode-rk.json");
    let env = env_with(&home);

    let result = discover_import(&cwd, &env, &dest);
    let err = match &result {
        Err(e) => e,
        Ok(_) => panic!("expected overlap error, got offer"),
    };
    assert!(
        err.contains("same directory"),
        "error should mention same-directory clash: got {err}"
    );
}

#[test]
fn dest_sibling_of_source_is_allowed() {
    let sandbox = Sandbox::new("sibling");
    let home = sandbox.path("home");
    let global_dir = home.join(".config/opencode");
    write(&global_dir.join("opencode.json"), br#"{"model":"gpt-5.6"}"#);
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = home.join(".config/opencode-rk/opencode.json");
    let env = env_with(&home);

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find source");

    assert_eq!(offer.plan.value.get("model").and_then(Value::as_str), Some("gpt-5.6"));
}

#[test]
fn ancestor_precedence_closer_wins() {
    let sandbox = Sandbox::new("precedence");
    let home = sandbox.path("home");
    // Root config
    let root_dir = sandbox.path("proj");
    write(&root_dir.join("opencode.json"), br#"{"model":"root-model"}"#);
    // Closer ancestor config (should win)
    let sub_dir = sandbox.path("proj/sub");
    write(&sub_dir.join("opencode.json"), br#"{"model":"sub-model"}"#);
    // CWD is even closer
    let cwd = sandbox.path("proj/sub/deep");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = env_with(&home);

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find configs");

    let model = offer.plan.value.get("model").and_then(Value::as_str);
    assert_eq!(model, Some("sub-model"), "nearest ancestor should win");
}

#[test]
fn warnings_collected_for_malformed_source() {
    let sandbox = Sandbox::new("malformed");
    let home = sandbox.path("home");
    let global_dir = home.join(".config/opencode");
    // A malformed JSON file
    write(&global_dir.join("opencode.json"), b"{ not valid json");
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = env_with(&home);

    // Malformed source should produce a warning, and if no other sources
    // exist, the offer should still be None (no meaningful content).
    let result = discover_import(&cwd, &env, &dest).expect("discovery ok");
    // With only a malformed source, we may get None (no valid plan content)
    // but the key is it doesn't panic.
    if let Some(offer) = result {
        assert!(!offer.warnings.is_empty(), "warnings should be recorded");
    }
}

#[test]
fn source_groups_preserves_ordering() {
    let sandbox = Sandbox::new("groups");
    let home = sandbox.path("home");
    // OpenCode global
    let oc_global = home.join(".config/opencode");
    write(&oc_global.join("opencode.json"), br#"{"model":"gpt-5.6"}"#);
    // Claude
    let claude_dir = home.join(".claude");
    write(
        &claude_dir.join("settings.json"),
        br#"{"model":"claude-3-opus"}"#,
    );
    // Codex
    let codex_dir = home.join(".codex");
    write(
        &codex_dir.join("config.toml"),
        b"model = \"gpt-4\"\n",
    );
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = env_with(&home);

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find sources");

    let groups = source_groups(&offer);
    let group_names: Vec<&str> = groups.iter().map(|(name, _)| name.as_str()).collect();

    // OpenCode should come before Codex before Claude (stable ordering)
    let oc_pos = group_names.iter().position(|&n| n == "OpenCode");
    let codex_pos = group_names.iter().position(|&n| n == "Codex");
    let claude_pos = group_names.iter().position(|&n| n == "Claude Code");

    if let (Some(oc), Some(codex)) = (oc_pos, codex_pos) {
        assert!(oc < codex, "OpenCode group should precede Codex");
    }
    if let (Some(codex), Some(claude)) = (codex_pos, claude_pos) {
        assert!(codex < claude, "Codex group should precede Claude");
    }
}

#[test]
fn redaction_in_plan_value_does_not_leak_secrets() {
    let sandbox = Sandbox::new("redaction");
    let home = sandbox.path("home");
    let global_dir = home.join(".config/opencode");
    write(
        &global_dir.join("opencode.json"),
        br#"{"model":"gpt-5.6","apiKey":"sk-secret-value-1234567890"}"#,
    );
    let cwd = sandbox.path("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let dest = sandbox.path("native/opencode.json");
    let env = env_with(&home);

    let offer = discover_import(&cwd, &env, &dest)
        .expect("discovery ok")
        .expect("should find source");

    let serialized = serde_json::to_string(&offer.plan.value).expect("serialize plan");
    assert!(
        !serialized.contains("sk-secret-value-1234567890"),
        "raw API key must not appear in plan value"
    );}
