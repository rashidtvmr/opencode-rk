#[path = "../src/delegation_lane.rs"]
mod delegation_lane;

use std::time::{Duration, Instant};

use delegation_lane::{DelegError, DelegationController, DelegationId, Mode, OwnerToken, State};

fn owner(n: u64) -> OwnerToken {
    OwnerToken::new(n)
}

#[test]
fn auto_004_t01_foreground_submit_returns_owned_handle() {
    let mut ctl = DelegationController::with_defaults();
    let owner_a = owner(1);
    let handle = ctl
        .submit(owner_a, Mode::Foreground, "foreground work")
        .expect("submit should succeed");
    assert_eq!(handle.owner, owner_a);
    assert_eq!(handle.mode, Mode::Foreground);
    let st = ctl.status(handle.id).expect("entry must be visible");
    assert_eq!(st.state, State::Running);
    assert_eq!(st.owner, owner_a);
    assert_eq!(st.mode, Mode::Foreground);
}

#[test]
fn auto_004_t02_background_detach_keeps_owner() {
    let mut ctl = DelegationController::with_defaults();
    let owner_a = owner(1);
    let owner_b = owner(2);
    let handle = ctl
        .submit(owner_a, Mode::Foreground, "detachable work")
        .expect("submit should succeed");
    let st = ctl
        .detach(handle.id, owner_a)
        .expect("owner detach should succeed");
    assert_eq!(st.owner, owner_a);
    assert_eq!(st.state, State::Detached);
    let denied = ctl
        .detach(handle.id, owner_b)
        .expect_err("non-owner detach must fail");
    assert_eq!(denied, DelegError::NotOwner);
    let after = ctl.status(handle.id).expect("status stays queryable");
    assert_eq!(after.owner, owner_a);
    assert_eq!(after.state, State::Detached);
}

#[test]
fn auto_004_t03_status_queryable_without_logs_unknown_id() {
    let mut ctl = DelegationController::with_defaults();
    let owner_a = owner(1);
    let handle = ctl
        .submit(owner_a, Mode::Background, "queryable work")
        .expect("submit should succeed");
    // No logging is configured or read here: status comes from controller state.
    let running = ctl.status(handle.id).expect("running must be queryable");
    assert_eq!(running.state, State::Running);
    ctl.detach(handle.id, owner_a)
        .expect("detach should succeed");
    let detached = ctl.status(handle.id).expect("detached must be queryable");
    assert_eq!(detached.state, State::Detached);
    ctl.complete(handle.id, owner_a, "done")
        .expect("complete should succeed");
    let done = ctl.status(handle.id).expect("terminal must be queryable");
    assert_eq!(done.state, State::Complete);
    let unknown = ctl
        .status(DelegationId::new(9999))
        .expect_err("unknown id must fail");
    assert_eq!(unknown, DelegError::Unknown);
}

#[test]
fn auto_004_t04_owner_cancel_reclaims_within_bound() {
    let mut ctl = DelegationController::new(16, 4096, Duration::from_millis(1000));
    let owner_a = owner(1);
    let owner_b = owner(2);
    let victim = ctl
        .submit(owner_a, Mode::Background, "cancellable work")
        .expect("submit should succeed");
    let denied = ctl
        .cancel(victim.id, owner_b)
        .expect_err("non-owner cancel must fail");
    assert_eq!(denied, DelegError::NotOwner);
    assert!(
        !ctl.task_joined(victim.id),
        "non-owner cancel must leave the task live"
    );
    let start = Instant::now();
    let st = ctl
        .cancel(victim.id, owner_a)
        .expect("owner cancel should succeed");
    let elapsed = start.elapsed();
    assert_eq!(st.state, State::Cancelled);
    assert!(
        ctl.task_joined(victim.id),
        "cancel must reclaim the task, not just rename state"
    );
    assert!(
        elapsed <= ctl.cancel_timeout(),
        "cancel took {elapsed:?}, bound {:?}",
        ctl.cancel_timeout()
    );
    // Idempotent second cancel never resurrects work.
    let again = ctl
        .cancel(victim.id, owner_a)
        .expect("double cancel must be idempotent");
    assert_eq!(again.state, State::Cancelled);
    assert!(ctl.task_joined(victim.id));
    // Cancel after natural completion is a typed terminal error.
    let finished = ctl
        .submit(owner_a, Mode::Foreground, "short work")
        .expect("submit should succeed");
    ctl.complete(finished.id, owner_a, "done")
        .expect("complete should succeed");
    let terminal = ctl
        .cancel(finished.id, owner_a)
        .expect_err("cancel after complete must fail");
    assert_eq!(terminal, DelegError::Terminal);
}

#[test]
fn auto_004_t05_no_os_process_bounded_capacity_and_summary() {
    let mut ctl = DelegationController::with_defaults();
    let _handles: Vec<_> = (0..8)
        .map(|i| {
            ctl.submit(owner(1), Mode::Background, format!("work-{i}"))
                .expect("submit should succeed")
        })
        .collect();
    assert_eq!(ctl.child_process_count(), 0);
    // Admission is bounded: over-cap submit fails without queueing.
    let mut small = DelegationController::new(2, 64, Duration::from_millis(50));
    small
        .submit(owner(1), Mode::Foreground, "one")
        .expect("first submit should succeed");
    small
        .submit(owner(1), Mode::Foreground, "two")
        .expect("second submit should succeed");
    let live_before = small.live_count();
    let capped = small
        .submit(owner(1), Mode::Foreground, "three")
        .expect_err("over-cap submit must fail");
    assert_eq!(capped, DelegError::AtCapacity);
    assert_eq!(small.live_count(), live_before);
    // Summaries are byte-budgeted with a truncation marker.
    let big = "x".repeat(1024);
    let mut roomy = DelegationController::new(16, 64, Duration::from_millis(50));
    let hb = roomy
        .submit(owner(1), Mode::Foreground, big.clone())
        .expect("submit should succeed");
    roomy
        .complete(hb.id, owner(1), big.clone())
        .expect("complete should succeed");
    let st = roomy.status(hb.id).expect("status must be queryable");
    assert!(st.summary.len() <= roomy.max_summary_bytes());
    // Work failure lands in Failed with a bounded summary, still queryable.
    let hf = roomy
        .submit(owner(1), Mode::Foreground, "fragile")
        .expect("submit should succeed");
    roomy.fail(hf.id, big.clone());
    let failed = roomy.status(hf.id).expect("failed must be queryable");
    assert_eq!(failed.state, State::Failed);
    assert!(failed.summary.len() <= roomy.max_summary_bytes());
    // Process-local restart drops the live set; only terminal records remain.
    let live = roomy
        .submit(owner(1), Mode::Background, "live work")
        .expect("submit should succeed");
    roomy.restart();
    assert_eq!(roomy.child_process_count(), 0);
    assert!(roomy.task_joined(live.id));
    let dropped = roomy
        .status(live.id)
        .expect("post-restart must be queryable");
    assert_eq!(dropped.state, State::Failed);
    assert!(roomy.status(hb.id).is_ok());
}
