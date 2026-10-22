//! Regression tests for native own config integration.
//!
//! These tests exercise the actual runtime behavior of:
//! - `own_config_dir()` / `own_config_path()` path resolution
//! - `load_imported_providers()` reading the own config file
//! - `custom_provider::list()` / `custom_provider::load()` consuming the
//!   imported providers layer
//! - Source bytes unchanged (no mutation of existing files)
//! - Malformed own config returns explicit error

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use opencode_rk_providers::imported_config::{
    load_imported_providers, own_config_dir, own_config_path,
};
use opencode_rk_providers::custom_provider::{list, load};
/// RAII guard that serializes environment access, then restores changed vars.
struct EnvGuard {
    _env_lock: MutexGuard<'static, ()>,
    temp_dir: tempfile::TempDir,
    prev_rk: Option<std::ffi::OsString>,
    prev_xdg: Option<std::ffi::OsString>,
    prev_home: Option<std::ffi::OsString>,
    prev_oc_config: Option<std::ffi::OsString>,
    prev_imported_shared_api_key: Option<std::ffi::OsString>,
    prev_custom_env_var: Option<std::ffi::OsString>,
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
        let prev_imported_shared_api_key = std::env::var_os("IMPORTED_SHARED_API_KEY");
        let prev_custom_env_var = std::env::var_os("CUSTOM_ENV_VAR");
        EnvGuard {
            _env_lock: env_lock,
            temp_dir,
            prev_rk,
            prev_xdg,
            prev_home,
            prev_oc_config,
            prev_imported_shared_api_key,
            prev_custom_env_var,
        }
    }

    fn config_dir(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }

    fn set_as_rk_config_dir(&self) {
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
        if let Some(prev) = self.prev_imported_shared_api_key.take() {
            std::env::set_var("IMPORTED_SHARED_API_KEY", prev);
        } else {
            std::env::remove_var("IMPORTED_SHARED_API_KEY");
        }
        if let Some(prev) = self.prev_custom_env_var.take() {
            std::env::set_var("CUSTOM_ENV_VAR", prev);
        } else {
            std::env::remove_var("CUSTOM_ENV_VAR");
        }
    }
}

/// Write the own config file `opencode.json` in the given dir.
fn write_own_config(dir: &std::path::Path, json_content: &str) -> PathBuf {
    let path = dir.join("opencode.json");
    fs::write(&path, json_content).unwrap();
    path
}

/// Write a legacy `custom-providers.json` in the given dir.
fn write_custom_providers(dir: &std::path::Path, json_content: &str) -> PathBuf {
    let path = dir.join("custom-providers.json");
    fs::write(&path, json_content).unwrap();
    path
}

#[tokio::test]
async fn own_config_dir_resolves_from_rk_env() {
    let guard = EnvGuard::new();
    std::env::set_var("OPENCODE_RK_CONFIG_DIR", guard.config_dir());
    let dir = own_config_dir().unwrap();
    assert_eq!(dir, guard.config_dir());
    let path = own_config_path().unwrap();
    assert_eq!(path.file_name().unwrap(), "opencode.json");
    assert_eq!(path.parent().unwrap(), dir);
}

#[tokio::test]
async fn own_config_dir_resolves_from_xdg() {
    let _guard = EnvGuard::new();
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::env::remove_var("OPENCODE_RK_CONFIG_DIR");
    std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
    let dir = own_config_dir().unwrap();
    let expected = temp_dir.path().join("opencode-rk");
    assert_eq!(dir, expected);
}

#[tokio::test]
async fn own_config_dir_resolves_from_home() {
    let _guard = EnvGuard::new();
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::env::remove_var("OPENCODE_RK_CONFIG_DIR");
    std::env::remove_var("XDG_CONFIG_HOME");
    std::env::set_var("HOME", temp_dir.path());
    let dir = own_config_dir().unwrap();
    let expected = temp_dir.path().join(".config/opencode-rk");
    assert_eq!(dir, expected);
}

#[tokio::test]
async fn own_config_absent_returns_empty_providers() {
    let _guard = EnvGuard::new();
    std::env::set_var("OPENCODE_RK_CONFIG_DIR", "/nonexistent/path/that/should/not/exist");
    let result = load_imported_providers();
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn imported_provider_listed_with_base_url() {
    let guard = EnvGuard::new();
    guard.set_as_rk_config_dir();
    write_own_config(
        &guard.config_dir(),
        r#"{ "providers": { "my-llm": { "name": "My LLM", "base_url": "https://api.my-llm.example/v1", "api_key_env": "MY_LLM_API_KEY" } } }"#,
    );

    let providers = load_imported_providers().expect("should load");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].id, "my-llm");
    assert_eq!(providers[0].name, "My LLM");
    assert_eq!(providers[0].base_url, "https://api.my-llm.example/v1");
    assert_eq!(providers[0].api_key_env, Some("MY_LLM_API_KEY".to_string()));
}

#[tokio::test]
async fn custom_provider_list_includes_imported() {
    let guard = EnvGuard::new();
    guard.set_as_rk_config_dir();
    // Legacy custom-providers.json
    write_custom_providers(
        &guard.config_dir(),
        r#"{ "providers": { "legacy-llm": { "name": "Legacy", "base_url": "https://legacy.example/v1" } } }"#,
    );
    // Own config with imported provider
    write_own_config(
        &guard.config_dir(),
        r#"{ "providers": { "imported-llm": { "name": "Imported", "base_url": "https://imported.example/v1" } } }"#,
    );

    let configs = list().await.expect("should list");
    let ids: Vec<_> = configs.iter().map(|c| c.id.as_str()).collect();
    assert!(ids.contains(&"legacy-llm"), "legacy provider present");
    assert!(ids.contains(&"imported-llm"), "imported provider present in list");
    assert_eq!(
        configs.iter().find(|c| c.id == "imported-llm").unwrap().base_url,
        "https://imported.example/v1"
    );
}

#[tokio::test]
async fn custom_provider_load_resolves_imported_base_url() {
    let guard = EnvGuard::new();
    guard.set_as_rk_config_dir();
    write_own_config(
        &guard.config_dir(),
        r#"{ "providers": { "imported-llm": { "name": "Imported", "base_url": "https://imported.example/v1", "api_key_env": "IMPORTED_API_KEY" } } }"#,
    );

    let config = load("imported-llm").await.expect("should load").unwrap();
    assert_eq!(config.id, "imported-llm");
    assert_eq!(config.base_url, "https://imported.example/v1");
}

#[tokio::test]
async fn legacy_id_collision_endpoint_and_credential_consistent() {
    let guard = EnvGuard::new();
    guard.set_as_rk_config_dir();
    // Legacy has same ID - should win on endpoint
    write_custom_providers(
        &guard.config_dir(),
        r#"{ "providers": { "shared-llm": { "name": "Legacy Shared", "base_url": "https://legacy.example/v1" } } }"#,
    );
    // Own config also has same ID with different base_url and api_key_env
    write_own_config(
        &guard.config_dir(),
        r#"{ "providers": { "shared-llm": { "name": "Imported Shared", "base_url": "https://imported.example/v1", "api_key_env": "IMPORTED_SHARED_API_KEY" } } }"#,
    );

    // Legacy entry should win in list (list only adds non-existing IDs)
    let configs = list().await.expect("should list");
    let shared = configs.iter().find(|c| c.id == "shared-llm").unwrap();
    assert_eq!(shared.base_url, "https://legacy.example/v1");

    // load should use legacy endpoint and NOT import api_key_env
    let config = load("shared-llm").await.expect("should load").unwrap();
    assert_eq!(config.base_url, "https://legacy.example/v1");
    // Set only the imported env var - should not be picked up due to shadowing
    std::env::set_var("IMPORTED_SHARED_API_KEY", "imported-secret-key");
    let config = load("shared-llm").await.expect("should load").unwrap();
    // api_key should NOT be the imported secret (legacy convention uses SHARED_LLM_API_KEY)
    assert!(
        config.api_key.as_ref().map(|k| k.as_str()) != Some("imported-secret-key"),
        "legacy shadowing should not use imported api_key_env"
    );
    std::env::remove_var("IMPORTED_SHARED_API_KEY");
}

#[tokio::test]
async fn imported_provider_uses_custom_api_key_env() {
    let guard = EnvGuard::new();
    guard.set_as_rk_config_dir();
    write_own_config(
        &guard.config_dir(),
        r#"{ "providers": { "env-llm": { "name": "Env LLM", "base_url": "https://env.example/v1", "api_key_env": "CUSTOM_ENV_VAR" } } }"#,
    );
    std::env::set_var("CUSTOM_ENV_VAR", "env-secret-key");

    let config = load("env-llm").await.expect("should load").unwrap();
    assert_eq!(config.api_key, Some("env-secret-key".to_string()));

    std::env::remove_var("CUSTOM_ENV_VAR");
}

#[tokio::test]
async fn malformed_own_config_returns_explicit_error() {
    let guard = EnvGuard::new();
    guard.set_as_rk_config_dir();
    write_own_config(&guard.config_dir(), r#"{ "providers": { "bad": { "base_url": } } }"#);

    let result = load_imported_providers();
    assert!(result.is_err(), "malformed own config should return explicit error");
}

#[tokio::test]
async fn malformed_own_config_providers_wrong_type_returns_error() {
    let guard = EnvGuard::new();
    guard.set_as_rk_config_dir();
    write_own_config(&guard.config_dir(), r#"{ "providers": "not-an-object" }"#);

    let result = load_imported_providers();
    assert!(result.is_err(), "providers as non-object should return explicit error");
}

#[tokio::test]
async fn openai_provider_skipped_not_consumed() {
    let guard = EnvGuard::new();
    guard.set_as_rk_config_dir();
    write_own_config(
        &guard.config_dir(),
        r#"{ "providers": { "openai": { "name": "OpenAI", "base_url": "https://api.openai.com/v1" } } }"#,
    );

    let providers = load_imported_providers().expect("should load");
    assert!(providers.is_empty(), "openai builtin provider should be skipped");
}

#[tokio::test]
async fn source_bytes_unchanged() {
    let guard = EnvGuard::new();
    std::env::set_var("OPENCODE_RK_CONFIG_DIR", guard.config_dir());

    // Also set OPENCODE_CONFIG_DIR to same dir for legacy custom-providers.json
    let legacy_dir = guard.config_dir();
    std::env::set_var("OPENCODE_CONFIG_DIR", &legacy_dir);

    let json = r#"{ "providers": { "test-llm": { "name": "Test", "base_url": "https://test.example/v1" } } }"#;
    let own_path = write_own_config(&guard.config_dir(), json);
    let own_bytes_before = fs::read(&own_path).unwrap();

    let custom_path = write_custom_providers(
        &legacy_dir,
        r#"{ "providers": { "legacy-llm": { "name": "Legacy", "base_url": "https://legacy.example/v1" } } }"#,
    );
    let custom_bytes_before = fs::read(&custom_path).unwrap();

    // Trigger reads
    let _ = list().await.expect("should list");
    let _ = load("test-llm").await.expect("should load");

    let own_bytes_after = fs::read(&own_path).unwrap();
    let custom_bytes_after = fs::read(&custom_path).unwrap();

    assert_eq!(own_bytes_before, own_bytes_after, "own config source bytes must not change");
    assert_eq!(custom_bytes_before, custom_bytes_after, "legacy source bytes must not change");
}
