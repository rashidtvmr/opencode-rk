#![cfg(unix)]

use opencode_rk_contracts::SessionId;
use opencode_rk_security::app_policy::ExpectedScope;
use opencode_rk_security::tool_authorize::ToolAuthorizer;
use opencode_rk_security::{PermissionBroker, PermissionRule, PermissionSet, RuleEffect, SecurityPolicy};
use opencode_rk_tools::executor::{
    CleanupStatus, CleanupStep, ProcessError, ProcessLimits, ProcessRequest, ProcessTerminal,
    ToolExecutor,
};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::process::Command;
use tokio::sync::{oneshot, watch};

const PROCESS_TREE_FIXTURE: &str = r##"#!/bin/sh
printf '%s\n' "$$" > "$1"
sleep 30 &
descendant="$!"
printf '%s\n' "$descendant" > "$2"
wait "$descendant"
printf '%s\n' sentinel > "$3"
"##;

const EXIT_FIXTURE: &str = r##"#!/bin/sh
printf '%s\n' ready > "$1"
"##;

const OUTPUT_FIXTURE: &str = r##"#!/bin/sh
printf '0123456789'
"##;

struct Fixture {
    dir: tempfile::TempDir,
    executable: PathBuf,
}

impl Fixture {
    fn new(source: &str) -> Self {
        let dir = tempfile::tempdir().expect("disposable fixture directory");
        let executable = dir.path().join("fixture.sh");
        fs::write(&executable, source).expect("write direct executable fixture");
        let mut permissions = fs::metadata(&executable)
            .expect("fixture metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make fixture executable");
        Self { dir, executable }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn request(&self, args: Vec<String>) -> ProcessRequest {
        ProcessRequest {
            program: self.executable.to_string_lossy().into_owned(),
            args,
            cwd: self.dir.path().to_path_buf(),
            env: BTreeMap::new(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for name in ["parent.pid", "descendant.pid"] {
            let path = self.dir.path().join(name);
            let Ok(pid) = fs::read_to_string(path) else {
                continue;
            };
            let Ok(pid) = pid.trim().parse::<u32>() else {
                continue;
            };
            let _ = std::process::Command::new("/bin/kill")
                .args(["-KILL", &pid.to_string()])
                .status();
        }
    }
}

fn expected_scope(broker: &PermissionBroker, workspace: &Path) -> ExpectedScope {
    ExpectedScope {
        workspace: workspace.to_path_buf(),
        session: SessionId::new(),
        requester: "phase1-cancellation-test".to_owned(),
        now: SystemTime::UNIX_EPOCH + Duration::from_secs(1_750_000_000),
        policy_version: broker.generation(),
    }
}

fn limits() -> ProcessLimits {
    ProcessLimits {
        timeout: Duration::from_secs(5),
        startup_timeout: Duration::from_secs(2),
        cleanup_timeout: Duration::from_secs(2),
        max_stdout_bytes: 1024,
        max_stderr_bytes: 1024,
        max_argv_bytes: 4096,
        max_argv_elements: 16,
        max_env_bytes: 4096,
        max_env_elements: 16,
        max_cwd_bytes: 4096,
    }
}

fn authorized_fixture(
    fixture: &Fixture,
) -> (
    ToolExecutor,
    PermissionBroker,
    ToolAuthorizer<'static>,
    ExpectedScope,
) {
    let broker = Box::leak(Box::new(PermissionBroker::new(
        SecurityPolicy::lean_default(fixture.dir.path()),
    )));
    let expected = expected_scope(broker, fixture.dir.path());
    let authorizer = ToolAuthorizer::new(broker);
    (ToolExecutor::new(), broker.clone(), authorizer, expected)
}

async fn read_pid(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

async fn is_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .output()
        .await
        .expect("kill -0 probe")
        .status
        .success()
}

async fn wait_for_pids(parent: &Path, descendant: &Path) -> (u32, u32) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let (Some(parent), Some(descendant)) =
                (read_pid(parent).await, read_pid(descendant).await)
            {
                return (parent, descendant);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("fixture published bounded PIDs")
}

async fn assert_dead(parent: u32, descendant: u32) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if !is_alive(parent).await && !is_alive(descendant).await {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("owned process tree was reaped")
}

async fn assert_owner_result(
    owner: impl std::future::Future<Output = Result<opencode_rk_tools::executor::ProcessResult, ProcessError>>,
) -> Result<opencode_rk_tools::executor::ProcessResult, ProcessError> {
    tokio::time::timeout(Duration::from_secs(10), owner)
        .await
        .expect("bounded process owner")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_already_true_skips_authorization_readiness_and_spawn() {
    let fixture = Fixture::new(PROCESS_TREE_FIXTURE);
    let parent = fixture.path("parent.pid");
    let descendant = fixture.path("descendant.pid");
    let sentinel = fixture.path("sentinel");
    let (executor, broker, mut authorizer, expected) = authorized_fixture(&fixture);
    let (cancel_tx, cancellation) = watch::channel(true);
    let (ready_tx, ready_rx) = oneshot::channel();

    let result = assert_owner_result(executor.execute_authorized_process(
        ProcessRequest {
            program: "/bin/sleep".to_owned(),
            args: vec!["30".to_owned()],
            cwd: fixture.dir.path().to_path_buf(),
            env: BTreeMap::new(),
        },
        &mut authorizer,
        None,
        &expected,
        limits(),
        cancellation,
        ready_tx,
    ))
    .await
    .expect("pre-start cancellation is a result");

    assert_eq!(result.terminal, ProcessTerminal::CancelledBeforeStart);
    assert_eq!(result.cleanup, CleanupStatus::NotRequired);
    assert_eq!(authorizer.audit_len(), 0, "cancel before auth must not audit");
    assert_eq!(broker.audit_len(), 0, "cancel before auth must not audit broker");
    assert!(ready_rx.await.is_err(), "cancel before spawn emits no readiness");
    assert!(!parent.exists() && !descendant.exists() && !sentinel.exists());
    drop(cancel_tx);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broker_deny_and_human_gate_spawn_nothing() {
    let fixture = Fixture::new(OUTPUT_FIXTURE);
    let marker = fixture.path("marker");
    for effect in [RuleEffect::Deny, RuleEffect::Ask] {
        let broker = Box::leak(Box::new(
            PermissionBroker::new(SecurityPolicy::lean_default(fixture.dir.path()))
                .with_permissions(PermissionSet::new(vec![PermissionRule::new("*", effect)])),
        ));
        let expected = expected_scope(broker, fixture.dir.path());
        let mut authorizer = ToolAuthorizer::new(broker);
        let (_cancel_tx, cancellation) = watch::channel(false);
        let (ready_tx, ready_rx) = oneshot::channel();
        let result = assert_owner_result(fixture_executor_process(
            fixture.request(vec![marker.to_string_lossy().into_owned()]),
            &mut authorizer,
            &expected,
            broker,
            cancellation,
            ready_tx,
        ))
        .await;
        match effect {
            RuleEffect::Deny => assert!(matches!(result, Err(ProcessError::Denied { .. }))),
            RuleEffect::Ask => assert!(matches!(result, Err(ProcessError::HumanRequired { .. }))),
            RuleEffect::Allow => unreachable!(),
        }
        assert!(ready_rx.await.is_err());
        assert!(!marker.exists());
    }
}

async fn fixture_executor_process(
    request: ProcessRequest,
    authorizer: &mut ToolAuthorizer<'_>,
    expected: &ExpectedScope,
    _broker: &PermissionBroker,
    cancellation: watch::Receiver<bool>,
    ready: oneshot::Sender<opencode_rk_tools::executor::ProcessReady>,
) -> Result<opencode_rk_tools::executor::ProcessResult, ProcessError> {
    ToolExecutor::new().execute_authorized_process(
        request,
        authorizer,
        None,
        expected,
        limits(),
        cancellation,
        ready,
    ).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_after_readiness_reaps_owned_process_tree_and_prevents_marker() {
    let fixture = Fixture::new(PROCESS_TREE_FIXTURE);
    let parent = fixture.path("parent.pid");
    let descendant = fixture.path("descendant.pid");
    let sentinel = fixture.path("sentinel");
    let (executor, _broker, mut authorizer, expected) = authorized_fixture(&fixture);
    let (cancel_tx, cancellation) = watch::channel(false);
    let (ready_tx, mut ready_rx) = oneshot::channel();
    let owner = executor.execute_authorized_process(
        fixture.request(vec![
            parent.to_string_lossy().into_owned(),
            descendant.to_string_lossy().into_owned(),
            sentinel.to_string_lossy().into_owned(),
        ]),
        &mut authorizer,
        None,
        &expected,
        limits(),
        cancellation,
        ready_tx,
    );
    tokio::pin!(owner);
    let ready = tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            ready = &mut ready_rx => ready.expect("one readiness event"),
            result = &mut owner => panic!("owner completed before readiness: {result:?}"),
        }
    })
    .await
    .expect("bounded readiness");
    assert!(ready.pid > 0);
    let (parent_pid, descendant_pid) = wait_for_pids(&parent, &descendant).await;
    cancel_tx.send(true).expect("owner still observes cancellation");

    let result = assert_owner_result(&mut owner)
        .await
        .expect("post-readiness cancellation is a result");
    assert_eq!(result.terminal, ProcessTerminal::Cancelled);
    assert!(matches!(result.cleanup, CleanupStatus::Reaped { .. }));
    assert_dead(parent_pid, descendant_pid).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!sentinel.exists(), "cancelled descendant wrote delayed marker");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timeout_reaps_owned_process_tree() {
    let fixture = Fixture::new(PROCESS_TREE_FIXTURE);
    let parent = fixture.path("parent.pid");
    let descendant = fixture.path("descendant.pid");
    let sentinel = fixture.path("sentinel");
    let (executor, _broker, mut authorizer, expected) = authorized_fixture(&fixture);
    let (cancel_tx, cancellation) = watch::channel(false);
    let (ready_tx, ready_rx) = oneshot::channel();
    let mut short = limits();
    short.timeout = Duration::from_millis(100);
    short.startup_timeout = Duration::from_millis(100);
    let owner = executor.execute_authorized_process(
        fixture.request(vec![
            parent.to_string_lossy().into_owned(),
            descendant.to_string_lossy().into_owned(),
            sentinel.to_string_lossy().into_owned(),
        ]),
        &mut authorizer,
        None,
        &expected,
        short,
        cancellation,
        ready_tx,
    );
    tokio::pin!(owner);
    let ready = tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            ready = ready_rx => ready.expect("one readiness event"),
            result = &mut owner => panic!("owner completed before readiness: {result:?}"),
        }
    })
    .await
    .expect("bounded readiness");
    let (parent_pid, descendant_pid) = wait_for_pids(&parent, &descendant).await;
    let result = assert_owner_result(&mut owner)
        .await
        .expect("timeout is a result");
    assert_eq!(result.terminal, ProcessTerminal::TimedOut);
    assert!(matches!(result.cleanup, CleanupStatus::Reaped { .. }));
    assert_eq!(ready.process_group, ready.pid);
    assert_dead(parent_pid, descendant_pid).await;
    assert!(!sentinel.exists(), "timed-out descendant wrote delayed marker");
    drop(cancel_tx);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropped_readiness_receiver_reaps_before_typed_error() {
    let fixture = Fixture::new(PROCESS_TREE_FIXTURE);
    let parent = fixture.path("parent.pid");
    let descendant = fixture.path("descendant.pid");
    let sentinel = fixture.path("sentinel");
    let (executor, _broker, mut authorizer, expected) = authorized_fixture(&fixture);
    let (_cancel_tx, cancellation) = watch::channel(false);
    let (ready_tx, ready_rx) = oneshot::channel();
    drop(ready_rx);
    let result = assert_owner_result(executor.execute_authorized_process(
        fixture.request(vec![
            parent.to_string_lossy().into_owned(),
            descendant.to_string_lossy().into_owned(),
            sentinel.to_string_lossy().into_owned(),
        ]),
        &mut authorizer,
        None,
        &expected,
        limits(),
        cancellation,
        ready_tx,
    ))
    .await;
    match result {
        Err(ProcessError::ReadinessReceiverClosed { pid: error_pid }) => {
            assert!(!is_alive(error_pid).await, "dropped receiver leaked child");
        }
        other => panic!("expected typed readiness receiver error, got {other:?}"),
    }
    assert!(!sentinel.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_cap_drains_and_marks_without_retaining_over_cap() {
    let fixture = Fixture::new(OUTPUT_FIXTURE);
    let (executor, _broker, mut authorizer, expected) = authorized_fixture(&fixture);
    let (_cancel_tx, cancellation) = watch::channel(false);
    let (ready_tx, ready_rx) = oneshot::channel();
    let mut bounded = limits();
    bounded.max_stdout_bytes = 4;
    let result = assert_owner_result(executor.execute_authorized_process(
        fixture.request(Vec::new()),
        &mut authorizer,
        None,
        &expected,
        bounded,
        cancellation,
        ready_tx,
    ))
    .await
    .expect("bounded output process result");
    assert!(matches!(result.terminal, ProcessTerminal::Exited { .. }));
    assert_eq!(result.stdout, "0123\n[truncated]");
    assert!(result.stdout_truncated);
    assert!(result.stdout.len() <= 4 + "\n[truncated]".len());
    assert!(ready_rx.await.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closed_false_cancellation_sender_does_not_cancel() {
    let fixture = Fixture::new(OUTPUT_FIXTURE);
    let (executor, _broker, mut authorizer, expected) = authorized_fixture(&fixture);
    let (cancel_tx, cancellation) = watch::channel(false);
    let (ready_tx, ready_rx) = oneshot::channel();
    drop(cancel_tx);
    let result = assert_owner_result(executor.execute_authorized_process(
        fixture.request(Vec::new()),
        &mut authorizer,
        None,
        &expected,
        limits(),
        cancellation,
        ready_tx,
    ))
    .await
    .expect("closed false cancellation is not cancellation");
    assert!(matches!(result.terminal, ProcessTerminal::Exited { .. }));
    assert!(matches!(result.cleanup, CleanupStatus::Reaped { .. }));
    assert!(ready_rx.await.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completed_child_wins_over_later_cancellation() {
    let fixture = Fixture::new(EXIT_FIXTURE);
    let marker = fixture.path("exit.marker");
    let (executor, _broker, mut authorizer, expected) = authorized_fixture(&fixture);
    let (cancel_tx, cancellation) = watch::channel(false);
    let (ready_tx, ready_rx) = oneshot::channel();
    let owner = executor.execute_authorized_process(
        fixture.request(vec![marker.to_string_lossy().into_owned()]),
        &mut authorizer,
        None,
        &expected,
        limits(),
        cancellation,
        ready_tx,
    );
    tokio::pin!(owner);
    tokio::time::timeout(Duration::from_secs(2), async {
        tokio::select! {
            ready = ready_rx => ready.expect("one readiness event"),
            result = &mut owner => panic!("owner completed before readiness: {result:?}"),
        }
    })
    .await
    .expect("bounded readiness");
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if marker.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("fixture reached terminal side effect");
    cancel_tx.send(true).expect("owner still has cancellation receiver");
    let result = assert_owner_result(&mut owner)
        .await
        .expect("completed child returns result");
    assert!(matches!(result.terminal, ProcessTerminal::Exited { .. }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_completion_reports_reaped_cleanup() {
    let fixture = Fixture::new(OUTPUT_FIXTURE);
    let (executor, _broker, mut authorizer, expected) = authorized_fixture(&fixture);
    let (_cancel_tx, cancellation) = watch::channel(false);
    let (ready_tx, ready_rx) = oneshot::channel();
    let result = assert_owner_result(executor.execute_authorized_process(
        fixture.request(Vec::new()),
        &mut authorizer,
        None,
        &expected,
        limits(),
        cancellation,
        ready_tx,
    ))
    .await
    .expect("owner completion");
    assert!(matches!(result.cleanup, CleanupStatus::Reaped {
        group_kill: CleanupStep::NotRequired,
        child_kill: CleanupStep::NotRequired,
        wait: CleanupStep::Succeeded,
        readers: CleanupStep::Succeeded,
    }));
    assert!(ready_rx.await.is_ok());
}
