//! TOOL-PROCESS-CWD-BYTES-RED: raw-byte cwd budget regression (Unix only).
//!
//! Fails on current behavior because `prepare_canonical_cwd`
//! (`crates/tools/src/executor.rs:193-223`) measures the cwd budget with
//! `Path::to_string_lossy().len()`, which expands non-UTF-8 OS bytes (e.g.
//! `0xff` becomes U+FFFD, 3 UTF-8 bytes) instead of counting raw Unix path
//! bytes. GREEN must count raw OS bytes so a one-byte non-UTF-8 cwd with
//! `max_cwd_bytes = 1` passes the budget check and then fails at
//! canonicalization (`cwd could not be canonicalized`), with no spawn and no
//! authorizer side effect.

#![cfg(unix)]

use opencode_rk_contracts::SessionId;
use opencode_rk_security::app_policy::ExpectedScope;
use opencode_rk_security::tool_authorize::ToolAuthorizer;
use opencode_rk_security::{PermissionBroker, SecurityPolicy};
use opencode_rk_tools::executor::{ProcessError, ProcessLimits, ProcessRequest, ToolExecutor};
use std::collections::BTreeMap;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

fn expected_scope(broker: &PermissionBroker) -> ExpectedScope {
    let session = SessionId::new();
    ExpectedScope {
        workspace: PathBuf::from("/work/project"),
        session,
        requester: "local-user".to_owned(),
        now: SystemTime::UNIX_EPOCH + Duration::from_secs(1_750_000_000),
        policy_version: broker.generation(),
    }
}

fn limits_with_cwd_budget(max_cwd_bytes: usize) -> ProcessLimits {
    ProcessLimits {
        timeout: Duration::from_secs(5),
        startup_timeout: Duration::from_secs(5),
        cleanup_timeout: Duration::from_secs(5),
        max_stdout_bytes: 1024,
        max_stderr_bytes: 1024,
        max_argv_bytes: 1024,
        max_argv_elements: 16,
        max_env_bytes: 1024,
        max_env_elements: 16,
        max_cwd_bytes,
    }
}

/// Raw-byte accounting: a 1-byte non-UTF-8 cwd fits `max_cwd_bytes = 1`, so
/// the failure must be canonicalization (path never created), not the budget.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cwd_budget_counts_raw_os_bytes_not_lossy_utf8() {
    // One raw OS byte, invalid UTF-8; lossy rendering expands to 3 bytes.
    let raw = std::ffi::OsString::from_vec(vec![0xff]);
    assert_eq!(raw.len(), 1, "fixture must be exactly one raw OS byte");
    assert_eq!(
        PathBuf::from(raw.clone()).to_string_lossy().len(),
        3,
        "lossy rendering must expand the fixture (else no defect signal)"
    );
    let cwd = PathBuf::from(raw);
    assert!(
        !cwd.exists(),
        "fixture path must not exist so canonicalization fails"
    );

    let executor = ToolExecutor::new();
    let broker = PermissionBroker::new(SecurityPolicy::lean_default("/work/project"));
    let mut authorizer = ToolAuthorizer::new(&broker);
    let expected = expected_scope(&broker);
    let request = ProcessRequest {
        program: "/bin/echo".to_owned(),
        args: vec!["hi".to_owned()],
        cwd,
        env: BTreeMap::new(),
    };
    let (_cancel_tx, cancellation) = tokio::sync::watch::channel(false);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();

    let audit_before = authorizer.audit_len();
    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        executor.execute_authorized_process(
            request,
            &mut authorizer,
            None,
            &expected,
            limits_with_cwd_budget(1),
            cancellation,
            ready_tx,
        ),
    )
    .await
    .expect("bounded execution");

    match outcome {
        Err(ProcessError::InvalidCwd { reason }) => assert_eq!(
            reason, "cwd could not be canonicalized",
            "raw 1-byte cwd must pass a 1-byte budget and fail canonicalization; got budget rejection instead"
        ),
        other => panic!(
            "expected InvalidCwd canonicalization failure for admitted raw-byte cwd, got {other:?}"
        ),
    }
    // No spawn: readiness is never sent; receiver sees sender-drop.
    assert!(
        ready_rx.await.is_err(),
        "no process may be spawned on an invalid cwd"
    );
    assert_eq!(
        authorizer.audit_len(),
        audit_before,
        "cwd validation precedes the broker: no authorization side effect"
    );
}
