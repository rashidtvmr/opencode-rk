use opencode_rk_security::{PermissionBroker, SecurityPolicy};
use opencode_rk_tools::executor::{ToolCall, ToolExecutor};
use serde_json::json;

fn executor(root: &std::path::Path) -> ToolExecutor {
    ToolExecutor::new()
        .with_cwd(root)
        .with_broker(PermissionBroker::new(SecurityPolicy::lean_default(
            root.to_path_buf(),
        )))
}
#[tokio::test]
async fn oversize_write_is_rejected_instead_of_silent_truncation() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source");
    std::fs::write(&file, "keep").unwrap();
    let result = executor(root.path())
        .execute(ToolCall::new(
            "write",
            "write",
            json!({"path":file,"content":"x".repeat(1_048_577)}),
        ))
        .await;
    assert!(
        !result.success,
        "oversize content must not be silently shortened"
    );
    assert_eq!(std::fs::read_to_string(file).unwrap(), "keep");
}
#[tokio::test]
async fn identical_edit_rejected_with_no_side_effect() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source");
    std::fs::write(&file, "same").unwrap();
    let result = executor(root.path())
        .execute(ToolCall::new(
            "edit",
            "edit",
            json!({"path":file,"oldString":"same","newString":"same"}),
        ))
        .await;
    assert!(!result.success, "identical edit must not report a change");
    assert_eq!(std::fs::read_to_string(file).unwrap(), "same");
}
#[tokio::test]
async fn read_directory_returns_real_bounded_entries() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("visible.txt"), "content").unwrap();
    let result = executor(root.path())
        .execute(ToolCall::new("read", "read", json!({"path":root.path()})))
        .await;
    assert!(result.success, "directory read failed: {:?}", result.error);
    assert!(result.output.contains("visible.txt"));
}
#[tokio::test]
async fn actual_edit_returns_replayable_diff_facts() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source");
    std::fs::write(&file, "before\n").unwrap();
    let result = executor(root.path())
        .execute(ToolCall::new(
            "edit",
            "edit",
            json!({"path":file,"oldString":"before","newString":"after"}),
        ))
        .await;
    assert!(result.success, "edit failed: {:?}", result.error);
    let value: serde_json::Value =
        serde_json::from_str(&result.output).expect("edit result must carry typed diff facts");
    assert_eq!(value["format"], "opencode.file-edit.v1");
    assert!(value["diff"].as_str().unwrap().contains("-before"));
    assert!(value["diff"].as_str().unwrap().contains("+after"));
    assert_eq!(std::fs::read_to_string(file).unwrap(), "after\n");
}
#[tokio::test]
async fn out_of_root_edit_does_not_leak_match_existence() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let file = outside.path().join("source");
    std::fs::write(&file, "private-value").unwrap();
    let result = executor(root.path())
        .execute(ToolCall::new(
            "edit",
            "edit",
            json!({"path":file,"oldString":"not-present","newString":"replacement"}),
        ))
        .await;
    assert!(!result.success);
    assert!(
        !result.error.unwrap_or_default().contains("match count"),
        "outside source was read before boundary validation"
    );
}
#[cfg(unix)]
#[tokio::test]
async fn read_rejects_symlink_ancestor() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("source"), "private-value").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
    let result = executor(root.path())
        .execute(ToolCall::new(
            "read",
            "read",
            json!({"path":root.path().join("link/source")}),
        ))
        .await;
    assert!(!result.success, "read followed a symlink ancestor");
    assert!(!result.output.contains("private-value"));
}

#[cfg(unix)]
#[tokio::test]
async fn read_rejects_hardlinked_secret_alias() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join(".env");
    std::fs::write(&secret, "PRIVATE_SECRET_VALUE").unwrap();
    let alias = root.path().join("innocent.txt");
    std::fs::hard_link(secret, &alias).unwrap();
    let result = executor(root.path())
        .execute(ToolCall::new("read", "read", json!({"path":alias})))
        .await;
    assert!(!result.success, "hardlink alias bypassed secret boundary");
    assert!(!result.output.contains("PRIVATE_SECRET_VALUE"));
}
