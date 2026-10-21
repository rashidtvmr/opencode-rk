use opencode_rk_tools::shell_tool::{ShellConfig, ShellError, ShellTool};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
fn config() -> ShellConfig {
    ShellConfig {
        allowed_commands: vec!["bash".into()],
        timeout_secs: 2,
        max_output_mb: 1,
    }
}
fn invocation(command: &str, marker: &std::path::Path) -> ShellTool {
    ShellTool::new("bash", vec!["-c".into(), command.into()])
        .env(HashMap::from([(
            "MARKER".into(),
            marker.to_string_lossy().into_owned(),
        )]))
        .timeout_millis(40)
}
#[tokio::test]
async fn deadline_kills_descendants_even_after_parent_exits() {
    let root = tempfile::tempdir().unwrap();
    let marker = root.path().join("late");
    let start = Instant::now();
    let result = invocation(
        "(sleep 0.3; printf forbidden > \"$MARKER\") & exit 0",
        &marker,
    )
    .execute(config())
    .await;
    assert!(
        matches!(result, Err(ShellError::Timeout(_))),
        "whole owned lifetime ignored deadline: {result:?}"
    );
    assert!(start.elapsed() < Duration::from_millis(250));
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!marker.exists(), "orphaned descendant survived timeout");
}
#[tokio::test]
async fn cancellation_kills_group_before_return_and_before_spawn() {
    let root = tempfile::tempdir().unwrap();
    let marker = root.path().join("late");
    let (tx, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(
        invocation(
            "(sleep 0.3; printf forbidden > \"$MARKER\") & wait",
            &marker,
        )
        .timeout_millis(2000)
        .cancellation(rx)
        .execute(config()),
    );
    tokio::time::sleep(Duration::from_millis(30)).await;
    tx.send_replace(true);
    assert!(matches!(task.await.unwrap(), Err(ShellError::Cancelled)));
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!marker.exists());
    let (_tx, rx) = tokio::sync::watch::channel(true);
    assert!(matches!(
        invocation("printf forbidden > \"$MARKER\"", &marker)
            .cancellation(rx)
            .execute(config())
            .await,
        Err(ShellError::Cancelled)
    ));
    assert!(!marker.exists());
}
#[tokio::test]
async fn flooded_pipe_is_drained_with_bounded_retention() {
    let root = tempfile::tempdir().unwrap();
    let result = invocation("head -c 2097152 /dev/zero", &root.path().join("unused"))
        .timeout_millis(2000)
        .execute(config())
        .await
        .unwrap();
    assert!(result.success);
    assert!(result.stdout.len() <= 1024 * 1024 + 32);
    assert!(result.stdout.ends_with("[truncated]"));
}
