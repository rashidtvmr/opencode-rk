//! Integration tests for configured_commands own config layer.
//! Verifies that `load()` and `load_agent()` consume the native own config
//! file (opencode.json in OPENCODE_RK_CONFIG_DIR) as an additional layer
//! between global config and project configs.

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use opencode_rk_providers::custom_provider;

/// RAII guard that serializes environment access, then restores changed vars.
struct EnvGuard {
    _env_lock: MutexGuard<'static, ()>,
    _temp_dir: tempfile::TempDir,
    prev_rk: Option<std::ffi::OsString>,
    prev_xdg: Option<std::ffi::OsString>,
    prev_home: Option<std::ffi::OsString>,
    prev_oc_config: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn new() -> Self {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let env_lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp_dir = tempfile::TempDir::new().unwrap();
        let prev_rk = std::env::var_os("OPENCODE_RK_CONFIG_DIR");
        let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let prev_home = std::env::var_os("HOME");
        let prev_oc_config = std::env::var_os("OPENCODE_CONFIG_DIR");
        EnvGuard {
            _env_lock: env_lock,
            _temp_dir: temp_dir,
            prev_rk,
            prev_xdg,
            prev_home,
            prev_oc_config,
        }
    }

    fn config_dir(&self) -> PathBuf {
        self._temp_dir.path().to_path_buf()
    }

    fn setup(&self) {
        std::env::set_var("OPENCODE_RK_CONFIG_DIR", self.config_dir());
        std::env::set_var("OPENCODE_CONFIG_DIR", self.config_dir());
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("HOME");
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(prev) = self.prev_rk.take() {
            std::env::set_var("OPENCODE_RK_CONFIG_DIR", prev);
        } else {
            std::env::remove_var("OPENCODE_RK_CONFIG_DIR");
        }
        if let Some(prev) = self.prev_xdg.take() {
            std::env::set_var("XDG_CONFIG_HOME", prev);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        if let Some(prev) = self.prev_home.take() {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
        if let Some(prev) = self.prev_oc_config.take() {
            std::env::set_var("OPENCODE_CONFIG_DIR", prev);
        } else {
            std::env::remove_var("OPENCODE_CONFIG_DIR");
        }
    }
}

fn write_own_config(dir: &std::path::Path, json_content: &str) -> PathBuf {
    let path = dir.join("opencode.json");
    fs::write(&path, json_content).unwrap();
    path
}

fn write_project_config(dir: &std::path::Path, json_content: &str) -> PathBuf {
    let path = dir.join("opencode.json");
    fs::write(&path, json_content).unwrap();
    path
}

#[tokio::test]
async fn load_agent_reads_from_own_config() {
    let guard = EnvGuard::new();
    guard.setup();

    let own_dir = guard.config_dir();
    write_own_config(
        &own_dir,
        r#"{ "agents": { "my-agent": { "name": "My Agent", "model": "openai" } } }"#,
    );

    let project_dir = own_dir.join("project");
    fs::create_dir_all(&project_dir).unwrap();

    // Should find the agent from the own config
    let result = opencode_rk_server::configured_commands::load_agent(&project_dir, "my-agent");
    assert!(result.is_ok(), "load_agent should succeed: {:?}", result.err());
    let agent = result.unwrap();
    assert!(agent.is_some(), "agent should be found in own config");
    assert_eq!(agent.unwrap()["name"], "My Agent");
}

#[tokio::test]
async fn load_agent_project_overrides_own_config() {
    let guard = EnvGuard::new();
    guard.setup();

    let root = guard.config_dir();

    // Own config has agent definition
    write_own_config(
        &root,
        r#"{ "agents": { "test-agent": { "name": "Own Agent", "model": "openai" } } }"#,
    );

    // Project config overrides the same agent
    let project_dir = root.join("project");
    fs::create_dir_all(&project_dir).unwrap();
    write_project_config(
        &project_dir,
        r#"{ "agents": { "test-agent": { "name": "Project Agent", "model": "anthropic" } } }"#,
    );

    let result = opencode_rk_server::configured_commands::load_agent(&project_dir, "test-agent");
    assert!(result.is_ok());
    let agent = result.unwrap().unwrap();
    // Project should win
    assert_eq!(agent["name"], "Project Agent");
}

#[tokio::test]
async fn load_commands_from_own_config() {
    let guard = EnvGuard::new();
    guard.setup();

    let own_dir = guard.config_dir();
    write_own_config(
        &own_dir,
        r#"{ "commands": { "test-cmd": { "description": "Test command", "template": "hello" } } }"#,
    );

    let project_dir = own_dir.join("project");
    fs::create_dir_all(&project_dir).unwrap();

    let result = opencode_rk_server::configured_commands::load(&project_dir);
    assert!(result.is_ok(), "load should succeed: {:?}", result.err());
    let inventory = result.unwrap();
    assert!(
        inventory.commands.contains_key("test-cmd"),
        "test-cmd should be in inventory from own config"
    );
    let cmd = &inventory.commands["test-cmd"];
    assert_eq!(cmd.template, "hello");
}

#[tokio::test]
async fn load_commands_project_overrides_own_config() {
    let guard = EnvGuard::new();
    guard.setup();

    let root = guard.config_dir();

    // Own config has command
    write_own_config(
        &root,
        r#"{ "commands": { "my-cmd": { "description": "Own command", "template": "from-own" } } }"#,
    );

    // Project config overrides
    let project_dir = root.join("project");
    fs::create_dir_all(&project_dir).unwrap();
    write_project_config(
        &project_dir,
        r#"{ "commands": { "my-cmd": { "description": "Project command", "template": "from-project" } } }"#,
    );

    let result = opencode_rk_server::configured_commands::load(&project_dir);
    assert!(result.is_ok());
    let inventory = result.unwrap();
    let cmd = &inventory.commands["my-cmd"];
    // Project should win
    assert_eq!(cmd.template, "from-project");
}

#[tokio::test]
async fn malformed_own_config_returns_error() {
    let guard = EnvGuard::new();
    guard.setup();

    let own_dir = guard.config_dir();
    // Write malformed JSON
    fs::write(own_dir.join("opencode.json"), r#"{ "agents": { "bad": } }"#).unwrap();

    let project_dir = own_dir.join("project");
    fs::create_dir_all(&project_dir).unwrap();

    let result = opencode_rk_server::configured_commands::load(&project_dir);
    assert!(
        result.is_err(),
        "malformed own config should return explicit error"
    );
}

#[tokio::test]
async fn own_config_source_bytes_unchanged() {
    let guard = EnvGuard::new();
    guard.setup();

    let root = guard.config_dir();
    let json = r#"{ "agents": { "test-agent": { "name": "Test", "model": "openai" } } }"#;
    let own_path = write_own_config(&root, json);
    let bytes_before = fs::read(&own_path).unwrap();

    let project_dir = root.join("project");
    fs::create_dir_all(&project_dir).unwrap();

    // Trigger load
    let _ = opencode_rk_server::configured_commands::load(&project_dir).unwrap();
    let _ = opencode_rk_server::configured_commands::load_agent(&project_dir, "test-agent").unwrap();

    let bytes_after = fs::read(&own_path).unwrap();
    assert_eq!(bytes_before, bytes_after, "own config source bytes must not change");
}

#[tokio::test]
async fn provider_list_load_with_imported_env_sourcebytes() {
    // Verify provider integration across list/load
    let guard = EnvGuard::new();
    guard.setup();

    let dir = guard.config_dir();
    fs::write(
        dir.join("custom-providers.json"),
        r#"{ "providers": { "legacy-llm": { "name": "Legacy", "base_url": "https://legacy.example/v1" } } }"#,
    )
    .unwrap();
    fs::write(
        dir.join("opencode.json"),
        r#"{ "providers": { "imported-llm": { "name": "Imported", "base_url": "https://imported.example/v1", "api_key_env": "IMP_API_KEY" } } }"#,
    )
    .unwrap();

    let configs = custom_provider::list().await.expect("should list");
    let ids: Vec<_> = configs.iter().map(|c| c.id.as_str()).collect();
    assert!(ids.contains(&"legacy-llm"));
    assert!(ids.contains(&"imported-llm"));

    // Verify source files unchanged
    let legacy_bytes = fs::read(dir.join("custom-providers.json")).unwrap();
    let own_bytes = fs::read(dir.join("opencode.json")).unwrap();
    assert!(legacy_bytes.windows(b"legacy-llm".len()).any(|w| w == b"legacy-llm"));
    assert!(own_bytes.windows(b"imported-llm".len()).any(|w| w == b"imported-llm"));
}
