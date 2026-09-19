#[path = "../src/delegation_live.rs"]
mod delegation_live;

use delegation_live::{
    build_spawn_plan, complete_handback, CompositionError, DelegationRequest, Handback, SpawnPlan,
};

use delegation_live::{ChildId, ChildState, Delegations, Ownership, OwnerToken, IndependentEffort};
use delegation_live::{AgentSession, SessionId};

fn parent_session(id: &str, agent: &str) -> AgentSession {
    AgentSession::new(id.to_owned(), agent.to_owned())
}

// ── T01: spawn plan gets fresh unique SessionId ──

#[test]
fn t01_spawn_plan_fresh_session_id() {
    let parent = parent_session("parent-sess-1", "agent-a");
    let req = DelegationRequest {
        agent_id: "agent-b".to_owned(),
        task: "review code".to_owned(),
        max_tokens: 4096,
        max_tool_calls: 50,
    };
    let plan = build_spawn_plan(&parent, &req);
    assert_ne!(plan.session_id, parent.id, "child SessionId must differ from parent");
    assert!(!plan.session_id.is_empty(), "child SessionId must not be empty");
}

// ── T02: spawn plan forks parent context variables ──

#[test]
fn t02_spawn_plan_forks_parent_context() {
    let parent = parent_session("parent-sess-2", "agent-a");
    let req = DelegationRequest {
        agent_id: "agent-c".to_owned(),
        task: "write tests".to_owned(),
        max_tokens: 2048,
        max_tool_calls: 25,
    };
    let plan = build_spawn_plan(&parent, &req);
    assert_eq!(plan.child_agent_id, "agent-c");
    assert_eq!(plan.task, "write tests");
    assert_eq!(plan.child_limits.max_tokens, Some(2048));
    assert_eq!(plan.child_limits.max_tool_calls, Some(25));
}

// ── T03: spawn plan budget inheritance ──

#[test]
fn t03_spawn_plan_budget_inheritance() {
    let parent = parent_session("parent-sess-3", "agent-a");
    let req = DelegationRequest {
        agent_id: "agent-d".to_owned(),
        task: "heavy work".to_owned(),
        max_tokens: 100_000,
        max_tool_calls: 500,
    };
    let plan = build_spawn_plan(&parent, &req);
    assert_eq!(plan.child_limits.max_tokens, Some(100_000));
    assert_eq!(plan.child_limits.max_tool_calls, Some(500));
}

// ── T04: spawn plan links cancellation token to delegation child ──

#[test]
fn t04_spawn_plan_cancellation_link() {
    let parent = parent_session("parent-sess-4", "agent-a");
    let mut delegations = Delegations::with_defaults();
    let owner = OwnerToken::new(1);
    let handle = delegations
        .spawn(100, owner, Ownership::ForegroundWait, IndependentEffort::new(5))
        .expect("spawn must succeed");
    let req = DelegationRequest {
        agent_id: "agent-e".to_owned(),
        task: "linked task".to_owned(),
        max_tokens: 4096,
        max_tool_calls: 50,
    };
    let mut plan = build_spawn_plan(&parent, &req);
    let linked = plan.link_to_delegation(handle.child);
    assert!(linked.is_ok(), "linking to live child must succeed");
    assert_eq!(plan.linked_child(), Some(handle.child));
}

// ── T05: handback reports aggregated output bounds ──

#[test]
fn t05_handback_output_bounds() {
    let handback = Handback::new("child-sess-1", "agent-f", "completed");
    assert_eq!(handback.session_id, "child-sess-1");
    assert_eq!(handback.agent_id, "agent-f");
    assert_eq!(handback.status, "completed");
    let large_output = "x".repeat(100_000);
    let hb_with_output = handback.with_output(&large_output);
    assert!(hb_with_output.output_len() <= 8192, "output must be bounded to 8KiB");
}

// ── T06: handback status transitions ──

#[test]
fn t06_handback_status_values() {
    let completed = Handback::new("s1", "a1", "completed");
    assert_eq!(completed.status, "completed");

    let failed = Handback::new("s2", "a2", "failed");
    assert_eq!(failed.status, "failed");

    let cancelled = Handback::new("s3", "a3", "cancelled");
    assert_eq!(cancelled.status, "cancelled");
}

// ── T07: full round trip parent→spawn→handback→transcript ──

#[test]
fn t07_full_round_trip() {
    let parent = parent_session("parent-rt", "orchestrator");
    let mut delegations = Delegations::with_defaults();
    let owner = OwnerToken::new(42);

    let req = DelegationRequest {
        agent_id: "worker-1".to_owned(),
        task: "implement feature X".to_owned(),
        max_tokens: 8192,
        max_tool_calls: 100,
    };

    // 2. build spawn plan
    let mut plan = build_spawn_plan(&parent, &req);

    // 3. register child in delegation controller
    let handle = delegations
        .spawn(1, owner, Ownership::BackgroundOwned, IndependentEffort::new(10))
        .expect("spawn must succeed");
    let linked = plan.link_to_delegation(handle.child);
    assert!(linked.is_ok());

    // 4. simulate child work and produce handback
    let handback = Handback::new(&plan.session_id, &plan.child_agent_id, "completed")
        .with_output("feature X implemented successfully");

    // 5. complete handback through delegation controller
    let result = complete_handback(&mut delegations, handle.child, owner, &handback);
    assert!(result.is_ok(), "handback completion must succeed");

    // 6. verify delegation is terminal
    assert!(delegations.task_joined(handle.child));

    // 7. verify parent can record transcript entry
    let transcript_entry = handback.to_transcript_entry();
    assert!(transcript_entry.contains("worker-1"));
    assert!(transcript_entry.contains("completed"));
}

// ── T08: handback on cancelled child reports cancellation ──

#[test]
fn t08_handback_cancelled_child() {
    let mut delegations = Delegations::with_defaults();
    let owner = OwnerToken::new(7);
    let handle = delegations
        .spawn(200, owner, Ownership::BackgroundOwned, IndependentEffort::new(3))
        .expect("spawn must succeed");

    let cancel_result = delegations.cancel(handle.child, owner);
    assert!(cancel_result.is_ok());

    let handback = Handback::new("cancelled-child", "agent-g", "cancelled");
    let result = complete_handback(&mut delegations, handle.child, owner, &handback);
    assert!(result.is_err(), "handback on cancelled child must fail");
}

// ── T09: multiple spawn plans get distinct session IDs ──

#[test]
fn t09_distinct_session_ids() {
    let parent = parent_session("parent-distinct", "agent-a");
    let req = DelegationRequest {
        agent_id: "agent-h".to_owned(),
        task: "task 1".to_owned(),
        max_tokens: 4096,
        max_tool_calls: 50,
    };
    let plan1 = build_spawn_plan(&parent, &req);
    let plan2 = build_spawn_plan(&parent, &req);
    assert_ne!(plan1.session_id, plan2.session_id, "each spawn plan must get unique SessionId");
}

// ── T10: spawn plan carries parent link metadata ──

#[test]
fn t10_spawn_plan_parent_link() {
    let parent = parent_session("parent-link", "agent-a");
    let req = DelegationRequest {
        agent_id: "agent-i".to_owned(),
        task: "linked work".to_owned(),
        max_tokens: 4096,
        max_tool_calls: 50,
    };
    let plan = build_spawn_plan(&parent, &req);
    assert_eq!(plan.parent_session_id, "parent-link");
    assert_eq!(plan.parent_agent_id, "agent-a");
}
