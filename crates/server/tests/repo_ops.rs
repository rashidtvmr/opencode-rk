use opencode_rk_server::repo_ops::{plan_repo_op, RepoOp};

#[test]
fn repo_t01_status_plan_readonly() {
    let plan = plan_repo_op("/repo", &RepoOp::Status).expect("status plans");
    assert_eq!(plan.op, "status");
    assert_eq!(plan.args, vec!["status", "--short", "--branch"]);
    assert!(plan.read_only);
}

#[test]
fn repo_t02_commit_plan_with_message() {
    let plan = plan_repo_op(
        "/repo",
        &RepoOp::Commit {
            message: "hello".to_string(),
        },
    )
    .expect("commit plans");
    assert_eq!(plan.op, "commit");
    assert_eq!(plan.args, vec!["commit", "-m", "hello"]);
    assert!(!plan.read_only);
}

#[test]
fn repo_t03_empty_path_rejected() {
    let err = plan_repo_op("", &RepoOp::Status).expect_err("empty path rejected");
    assert_eq!(err, opencode_rk_server::repo_ops::RepoOpsError::EmptyPath);
}

#[test]
fn repo_t04_empty_message_rejected() {
    let err = plan_repo_op(
        "/repo",
        &RepoOp::Commit {
            message: String::new(),
        },
    )
    .expect_err("empty message rejected");
    assert_eq!(
        err,
        opencode_rk_server::repo_ops::RepoOpsError::EmptyMessage
    );
    let err_ws = plan_repo_op(
        "/repo",
        &RepoOp::Commit {
            message: "   ".to_string(),
        },
    )
    .expect_err("whitespace message rejected");
    assert_eq!(
        err_ws,
        opencode_rk_server::repo_ops::RepoOpsError::EmptyMessage
    );
}

#[test]
fn repo_t05_long_message_truncated() {
    let long = "x".repeat(600);
    let plan = plan_repo_op(
        "/repo",
        &RepoOp::Commit {
            message: long.clone(),
        },
    )
    .expect("long message plans");
    assert_eq!(plan.op, "commit");
    assert_eq!(plan.args.len(), 3);
    let msg = &plan.args[2];
    assert!(msg.ends_with("..."), "truncated with ellipsis");
    assert_eq!(msg.len(), 503);
    assert_eq!(&msg[..500], &long[..500]);
}
