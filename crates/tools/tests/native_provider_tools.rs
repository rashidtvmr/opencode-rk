//! RED contract for the installed native provider-tool surface.
//!
//! This target is intentionally written before the implementation revision:
//! schemas, aliases, cwd propagation, search tools, and broker side effects
//! are all observable product behavior.

use opencode_rk_security::{PermissionBroker, PermissionSet, SecurityPolicy};
use opencode_rk_tools::executor::{ToolCall, ToolExecutor};
use opencode_rk_tools::registry::ToolRegistry;
use serde_json::json;
use tempfile::tempdir;

#[test]
fn every_default_provider_tool_has_a_nonempty_executable_schema() {
    let registry = ToolRegistry::new();
    for name in ["bash", "shell", "read", "write", "edit", "grep", "glob"] {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert!(tool.enabled, "{name} is disabled");
        assert_eq!(tool.input_schema["type"], "object", "{name} schema");
        assert!(
            !tool.input_schema["properties"]
                .as_object()
                .unwrap()
                .is_empty(),
            "{name} has fake schema"
        );
    }
}

#[test]
fn canonical_shell_aliases_have_the_same_real_contract() {
    let registry = ToolRegistry::new();
    assert_eq!(
        registry.get("bash").unwrap().input_schema,
        registry.get("shell").unwrap().input_schema
    );
}

#[tokio::test]
async fn read_write_edit_and_search_are_real_operations() {
    let workspace = tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    std::fs::write(workspace.path().join("src/note.txt"), "before\nneedle\n").unwrap();
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(workspace.path()))
        .with_permissions(PermissionSet::star());
    let executor = ToolExecutor::new()
        .with_cwd(workspace.path())
        .with_broker(broker);

    let read = executor
        .execute(ToolCall::new(
            "r",
            "read",
            json!({"path":workspace.path().join("src/note.txt")}),
        ))
        .await;
    assert!(read.success, "read: {:?}", read.error);
    assert!(read.output.contains("needle"));

    let edit = executor.execute(ToolCall::new("e", "edit", json!({
        "path":workspace.path().join("src/note.txt"), "oldString":"before", "newString":"after"
    }))).await;
    assert!(edit.success, "edit: {:?}", edit.error);
    assert!(
        std::fs::read_to_string(workspace.path().join("src/note.txt"))
            .unwrap()
            .starts_with("after")
    );

    let grep = executor.execute(ToolCall::new("g", "grep", json!({
        "pattern":"needle", "path":workspace.path().join("src"), "literal":true, "caseSensitive":true, "limit":10
    }))).await;
    assert!(grep.success, "grep: {:?}", grep.error);
    assert!(grep.output.contains("note.txt"));

    let glob = executor.execute(ToolCall::new("gl", "glob", json!({
        "pattern":"**/*.txt", "path":workspace.path().join("src"), "hidden":false, "limit":10
    }))).await;
    assert!(glob.success, "glob: {:?}", glob.error);
    assert!(glob.output.contains("note.txt"));
}

#[tokio::test]
async fn shell_uses_request_workdir_and_is_not_daemon_cwd() {
    let daemon_cwd = tempdir().unwrap();
    let request_cwd = tempdir().unwrap();
    let result = ToolExecutor::new()
        .with_cwd(daemon_cwd.path())
        .execute(ToolCall::new(
            "s",
            "shell",
            json!({"command":"pwd", "workdir":request_cwd.path()}),
        ))
        .await;
    assert!(result.success, "shell: {:?}", result.error);
    assert_eq!(
        std::fs::canonicalize(result.output.trim()).unwrap(),
        std::fs::canonicalize(request_cwd.path()).unwrap()
    );
}

#[tokio::test]
async fn denied_file_effect_has_no_side_effect() {
    let result = ToolExecutor::new()
        .execute(ToolCall::new(
            "w",
            "write",
            json!({"path":"/tmp/native-tools-blocked.txt", "content":"no"}),
        ))
        .await;
    assert!(!result.success, "write must be brokered and scoped");
}

#[tokio::test]
async fn read_and_edit_reject_symlink_targets_without_touching_outside() {
    let workspace = tempdir().unwrap();
    let outside = tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        workspace.path().join("link.txt"),
    )
    .unwrap();
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(workspace.path()))
        .with_permissions(PermissionSet::star());
    let executor = ToolExecutor::new()
        .with_cwd(workspace.path())
        .with_broker(broker);
    let read = executor
        .execute(ToolCall::new("r", "read", json!({"path":"link.txt"})))
        .await;
    assert!(!read.success);
    let edit = executor
        .execute(ToolCall::new(
            "e",
            "edit",
            json!({"path":"link.txt","oldString":"outside-secret","newString":"changed"}),
        ))
        .await;
    assert!(!edit.success);
    assert_eq!(
        std::fs::read_to_string(outside.path().join("secret.txt")).unwrap(),
        "outside-secret"
    );
}

#[tokio::test]
async fn search_reports_supported_globs_and_rejects_regex_and_excessive_patterns() {
    let workspace = tempdir().unwrap();
    std::fs::write(workspace.path().join("a.txt"), "needle").unwrap();
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(workspace.path()))
        .with_permissions(PermissionSet::star());
    let executor = ToolExecutor::new()
        .with_cwd(workspace.path())
        .with_broker(broker);
    let regex = executor
        .execute(ToolCall::new(
            "g",
            "grep",
            json!({"pattern":"nee.*e","path":".","literal":false}),
        ))
        .await;
    assert!(!regex.success);
    std::fs::create_dir_all(workspace.path().join("src/deep")).unwrap();
    std::fs::write(workspace.path().join("src/deep/a1.txt"), "needle").unwrap();
    let glob = executor
        .execute(ToolCall::new(
            "gl",
            "glob",
            json!({"pattern":"src/**/a?.txt","path":"."}),
        ))
        .await;
    assert!(glob.success, "supported glob failed: {:?}", glob.error);
    assert!(glob.output.contains("src/deep/a1.txt"));
    let excessive = executor
        .execute(ToolCall::new(
            "long",
            "glob",
            json!({"pattern":"*".repeat(257),"path":"."}),
        ))
        .await;
    assert!(
        !excessive.success,
        "unbounded patterns must not reach glob matching"
    );
}
