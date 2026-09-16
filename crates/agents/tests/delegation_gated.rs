#[path = "../src/delegation_lane.rs"]
mod delegation_lane;

use delegation_lane::{
    BoundedLanePool, BrokerDecision, DelegError, DelegationController, Mode, OwnerToken,
};

fn owner(n: u64) -> OwnerToken {
    OwnerToken::new(n)
}

#[test]
fn fa3_gated_t01_deny_blocks_with_no_state() {
    let mut ctl = DelegationController::with_defaults();
    let live_before = ctl.live_count();
    let denied = ctl
        .submit_gated(owner(1), Mode::Foreground, "denied work", BrokerDecision::Deny)
        .expect_err("deny must block admission");
    assert_eq!(denied, DelegError::BrokerDenied);
    assert_eq!(ctl.live_count(), live_before);
    assert_eq!(ctl.live_count(), 0);
}

#[test]
fn fa3_gated_t02_require_human_blocks_with_no_state() {
    let mut ctl = DelegationController::with_defaults();
    let denied = ctl
        .submit_gated(
            owner(1),
            Mode::Background,
            "human work",
            BrokerDecision::RequireHuman,
        )
        .expect_err("human gate must block admission");
    assert_eq!(denied, DelegError::BrokerDenied);
    assert_eq!(ctl.live_count(), 0);
}

#[test]
fn fa3_gated_t03_allow_admits_lane_work() {
    let mut ctl = DelegationController::with_defaults();
    let handle = ctl
        .submit_gated(owner(7), Mode::Foreground, "allowed work", BrokerDecision::Allow)
        .expect("allow must admit");
    assert_eq!(handle.owner, owner(7));
    assert_eq!(handle.mode, Mode::Foreground);
    assert_eq!(ctl.live_count(), 1);
    let st = ctl.status(handle.id).expect("admitted entry must be queryable");
    assert_eq!(st.owner, owner(7));
}

#[test]
fn fa3_gated_t04_pool_cap_denies_over_admission_without_mutation() {
    let pool = BoundedLanePool::new(2);
    assert!(pool.try_acquire(BrokerDecision::Allow));
    assert!(pool.try_acquire(BrokerDecision::Allow));
    assert_eq!(pool.live(), 2);
    assert!(!pool.try_acquire(BrokerDecision::Allow));
    assert_eq!(pool.live(), 2);
    assert!(!pool.try_acquire(BrokerDecision::Deny));
    assert!(!pool.try_acquire(BrokerDecision::RequireHuman));
    assert_eq!(pool.live(), 2);
    pool.release();
    assert_eq!(pool.live(), 1);
    assert!(pool.try_acquire(BrokerDecision::Allow));
    assert_eq!(pool.live(), 2);
}

#[test]
fn fa3_gated_t05_pool_over_release_never_underflows() {
    let pool = BoundedLanePool::new(1);
    assert_eq!(pool.live(), 0);
    pool.release();
    assert_eq!(pool.live(), 0);
    pool.release();
    assert_eq!(pool.live(), 0);
    assert!(pool.try_acquire(BrokerDecision::Allow));
    assert_eq!(pool.live(), 1);
    pool.release();
    pool.release();
    assert_eq!(pool.live(), 0);
}
