#[path = "../src/driver_lane.rs"]
mod driver_lane;

use std::collections::HashSet;

use driver_lane::{
    Driver, GateVerdict, LaneStatus, LeaseError, LeaseTable, OwnerToken, Plan, ReadyQueue, Stopped,
    TaskSpec,
};

fn set(ids: &[&str]) -> HashSet<String> {
    ids.iter().map(|s| (*s).to_owned()).collect()
}

fn fixture_plan() -> Plan {
    let mut plan = Plan::new();
    plan.add(TaskSpec::new("AUTO-001", 0, &[]));
    plan.add(TaskSpec::new("AUTO-002", 0, &[]));
    plan.add(TaskSpec::new("AUTO-006", 6, &[]));
    plan.add(TaskSpec::new("AUTO-007", 6, &["AUTO-006"]));
    plan
}

#[test]
fn auto_006_t01_ready_queue_respects_ranks() {
    let plan = fixture_plan();
    // Synthesized rank edges: rank-6 work is not ready while rank-0 is pending.
    let ready = ReadyQueue::next(&plan, &set(&[]), &set(&[]), 8);
    assert!(ready.iter().any(|t| t == "AUTO-001"));
    assert!(!ready.iter().any(|t| t == "AUTO-006"));
    // After lower ranks accepted, AUTO-006 becomes ready; its dependent waits.
    let accepted = set(&["AUTO-001", "AUTO-002"]);
    let ready = ReadyQueue::next(&plan, &accepted, &set(&[]), 8);
    assert!(ready.iter().any(|t| t == "AUTO-006"));
    assert!(!ready.iter().any(|t| t == "AUTO-007"));
    // Output sorted by (rank asc, id asc).
    assert!(ready
        .windows(2)
        .all(|w| plan.rank_of(&w[0]).unwrap() <= plan.rank_of(&w[1]).unwrap()));
    // Limit truncates the sorted queue.
    let one = ReadyQueue::next(&plan, &accepted, &set(&[]), 1);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0], ready[0]);
    // Blocked dep => dependent absent from ready, present in blocked_dependents.
    let blocked = set(&["AUTO-001"]);
    let ready = ReadyQueue::next(&plan, &set(&["AUTO-002"]), &blocked, 8);
    assert!(!ready.iter().any(|t| t == "AUTO-006"));
    let waiting = ReadyQueue::blocked_dependents(&plan, &set(&["AUTO-002"]), &blocked);
    assert!(waiting.contains("AUTO-006"));
}

#[test]
fn auto_006_t02_one_file_lease_enforced() {
    let mut leases = LeaseTable::new();
    let owner = OwnerToken::new(1);
    let other = OwnerToken::new(2);
    leases
        .acquire("AUTO-006", owner, "crates/agents/src/driver_lane.rs")
        .expect("first lease must succeed");
    let err = leases
        .acquire("AUTO-006", owner, "crates/agents/src/other.rs")
        .expect_err("second file for one lane must fail");
    assert_eq!(err, LeaseError::LeaseDenied);
    let err = leases
        .acquire("AUTO-006", other, "crates/agents/src/driver_lane.rs")
        .expect_err("live file is exclusive");
    assert_eq!(err, LeaseError::LeaseDenied);
    let err = leases
        .release("AUTO-006", other)
        .expect_err("non-owner release must fail");
    assert_eq!(err, LeaseError::LeaseDenied);
    assert!(
        leases.get("AUTO-006").is_some(),
        "lease retained after denied release"
    );
    leases
        .release("AUTO-006", owner)
        .expect("owner release must succeed");
    leases
        .acquire("AUTO-006", other, "crates/agents/src/driver_lane.rs")
        .expect("file reusable after owner release");
}

#[test]
fn auto_006_t03_gate_blocks_stub() {
    let mut driver = Driver::new(2);
    let owner = OwnerToken::new(7);
    driver
        .acquire("AUTO-006", owner, "lane/a.rs")
        .expect("lease must succeed");
    driver.stage("lane/a.rs", "placeholder");
    let verdict = driver.gate("AUTO-006");
    assert!(
        matches!(verdict, GateVerdict::Stub | GateVerdict::Incomplete),
        "stub body must not pass, got {verdict:?}"
    );
    assert!(!driver.committed("AUTO-006"));
    let status = driver.drive_task("AUTO-006");
    assert!(
        matches!(status, LaneStatus::Blocked { .. }),
        "lane must block after attempt cap, got {status:?}"
    );
    let err = status.last_error().unwrap_or("");
    assert!(
        err.contains("verification failed"),
        "unexpected last_error: {err}"
    );
    assert!(!driver.committed("AUTO-006"));
}

#[test]
fn auto_006_t04_commit_per_milestone() {
    let mut plan = Plan::new();
    plan.add(TaskSpec::new("M-LOW", 0, &[]));
    plan.add(TaskSpec::new("M-MID", 3, &[]));
    plan.add(TaskSpec::new("M-HIGH", 6, &[]));
    let mut driver = Driver::new(4);
    for (i, task) in ["M-LOW", "M-MID", "M-HIGH"].iter().enumerate() {
        driver
            .acquire(
                task,
                OwnerToken::new(10 + i as u64),
                &format!("lane/{task}.rs"),
            )
            .expect("lease must succeed");
        driver.stage(
            &format!("lane/{task}.rs"),
            "body verified:gate-pass with real content over twenty bytes",
        );
    }
    // Autonomous loop: merge each integrated batch into accepted, drive again.
    let mut accepted: HashSet<String> = HashSet::new();
    let blocked: HashSet<String> = HashSet::new();
    let mut order: Vec<String> = Vec::new();
    for _ in 0..4 {
        let done = driver.drive_ready(&plan, &accepted, &blocked);
        if done.is_empty() {
            break;
        }
        for task in &done {
            accepted.insert(task.clone());
        }
        order.extend(done);
    }
    assert_eq!(order, vec!["M-LOW", "M-MID", "M-HIGH"]);
    assert!(order
        .windows(2)
        .all(|w| plan.rank_of(&w[0]).unwrap() <= plan.rank_of(&w[1]).unwrap()));
    // Fast-forward conflict: stays accepted, never force-pushed, branch kept.
    let mut conflicted = Driver::new(1);
    conflicted
        .acquire("M-FF", OwnerToken::new(99), "lane/ff.rs")
        .expect("lease must succeed");
    conflicted.stage(
        "lane/ff.rs",
        "body verified:gate-pass with real content over twenty bytes",
    );
    conflicted.set_ff_conflict("M-FF", true);
    let status = conflicted.drive_task("M-FF");
    assert!(matches!(status, LaneStatus::Accepted { .. }));
    let err = conflicted
        .commit("M-FF")
        .expect_err("ff-conflict must not commit");
    assert!(
        err.to_string().contains("accepted but not integrated"),
        "unexpected commit error: {err}"
    );
    assert!(matches!(
        conflicted.status("M-FF"),
        LaneStatus::Accepted { .. }
    ));
    assert!(conflicted
        .status("M-FF")
        .last_error()
        .unwrap_or("")
        .contains("accepted but not integrated"));
    assert!(!conflicted.committed("M-FF"));
    assert!(conflicted.branch_preserved("M-FF"));
}

#[test]
fn auto_006_t05_stop_when_empty() {
    let mut driver = Driver::new(1);
    assert_eq!(driver.drive(), Stopped::NoReadyWork);
    assert_eq!(driver.workers_spawned(), 0);
    assert_eq!(driver.commits(), 0);
    let report = driver.stop_report();
    assert_eq!(report.code, 0);
    assert!(!report.receipt.is_empty());
    // Second drive is identical: no spin, no state drift.
    assert_eq!(driver.drive(), Stopped::NoReadyWork);
    assert_eq!(driver.stop_report(), report);
    assert_eq!(driver.workers_spawned(), 0);
    assert_eq!(driver.commits(), 0);
}
