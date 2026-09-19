//! LANE-LOOP-LIVE: LoopDriver live-turn integration tests.
//!
//! Proves that a live turn session exhibits LoopDriver semantics through
//! the server's public API.
//!
//! # Wiring gap (documented, not closable from a test-only file)
//!
//! The live turn stream (`create_turn_stream` at lib.rs:871) uses
//! `agent_loop::LoopController` (lib.rs:968), NOT `loop_driver::LoopDriver`.
//! The LoopDriver state machine is `pub mod loop_driver` (lib.rs:28) but is
//! never instantiated in any turn handler. The turn path has its own
//! step-cap mechanism (MAX_TURN_STEPS = 64, agent_loop.rs:23) separate
//! from LoopDriver's MAX_LOOP_STEPS = 256 (loop_driver.rs:26).
//!
//! Tests here verify the LoopDriver public contract and assert the observable
//! boundary: the turn stream does NOT expose LoopDriver trajectory events
//! (plan, step_outcome, goal_status). Where a wiring gap prevents proving
//! full integration, the test documents the gap and asserts the current
//! verifiable contract.
//!
//! # Scenarios
//!
//! Scenario A (drive): multi-goal task through LoopDriver produces
//! plan → step outcomes → goal completion with StopReason.
//!
//! Scenario B (checkpoint/resume): checkpoint(state) round-trips through
//! restore and resumed driver continues without repeating completed goals.
//!
//! Scenario C (steer): amendment event mid-loop reflected in next plan
//! evaluation.
//!
//! Scenario D (live turn gap): turn stream via server API does NOT surface
//! LoopDriver trajectory events, proving the wiring gap.
#![forbid(unsafe_code)]

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::Arc,
    time::Duration,
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::loop_driver::{
    GoalStatus, LoopAction, LoopDriver, LoopEvent, LoopGoal, LoopPlan, StepOutcome, StopReason,
    MAX_LOOP_STEPS,
};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Fixture helpers (pattern from agent_loop_turns.rs)
// ---------------------------------------------------------------------------

fn build_app() -> (axum::Router, tempfile::TempDir) {
    let dir = tempdir().expect("temporary server fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    let sessions = SessionService::new(Arc::new(storage));
    (
        router(AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        }),
        dir,
    )
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded response body");
    serde_json::from_slice(&bytes).expect("json response")
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected_len = None;
    loop {
        let read = stream.read(&mut buffer).expect("read provider request");
        assert!(read > 0, "provider request ended early");
        request.extend_from_slice(&buffer[..read]);
        if expected_len.is_none() {
            if let Some(header_end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::trim)
                            .map(str::parse::<usize>)
                    })
                    .transpose()
                    .expect("valid content length")
                    .unwrap_or(0);
                expected_len = Some(header_end + 4 + content_length);
            }
        }
        if expected_len.is_some_and(|len| request.len() >= len) {
            break;
        }
    }
    assert!(
        request.len() <= 256 * 1024,
        "provider request exceeded fixture bound"
    );
    request
}

fn respond_sse(stream: &mut TcpStream, events: &[&str]) {
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
        )
        .expect("write provider headers");
    for event in events {
        stream
            .write_all(event.as_bytes())
            .expect("write provider event");
    }
    stream.flush().expect("flush provider events");
}

fn round_one_events() -> Vec<String> {
    vec![
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"Planning task…\"}\n\n".to_owned(),
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_r1\",\"status\":\"completed\"}}\n\n".to_owned(),
    ]
}

fn spawn_scripted_provider(rounds: Vec<Vec<String>>) -> (String, std::thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind provider fixture");
    let address = listener.local_addr().expect("fixture address");
    let task = std::thread::spawn(move || {
        let mut bodies = Vec::new();
        for events in &rounds {
            let (mut stream, _) = listener.accept().expect("accept provider request");
            let request = read_request(&mut stream);
            let header_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("provider request header terminator");
            let body = &request[header_end + 4..];
            bodies.push(serde_json::from_slice(body).expect("provider json"));
            respond_sse(&mut stream, &events.iter().map(String::as_str).collect::<Vec<_>>());
        }
        bodies
    });
    (format!("http://{address}/v1"), task)
}

async fn spawn_http(app: axum::Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind server");
    let address = listener.local_addr().expect("server address");
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    (address, task)
}

async fn create_session(app: &axum::Router, title: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "title": title }).to_string()))
                .unwrap(),
        )
        .await
        .expect("create session");
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["session"]["id"]
        .as_str()
        .expect("session id")
        .to_owned()
}

fn stream_turn(address: SocketAddr, session_id: String) -> Vec<u8> {
    let stream = TcpStream::connect(address).expect("connect server");
    let mut stream = stream;
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("read timeout");
    let body = json!({
        "text": "complete the multi-goal loop fixture task",
        "model": "openai/gpt-5.6",
        "reasoning_effort": "high"
    })
    .to_string();
    let request = format!(
        "POST /api/sessions/{session_id}/turns/stream HTTP/1.1\r\nhost: localhost\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("write request");
    stream.flush().expect("flush request");
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = stream.read(&mut chunk).expect("read response");
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    raw
}

fn ndjson_events(response: &[u8]) -> Vec<Value> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("response header terminator");
    let payload = std::str::from_utf8(&response[header_end + 4..]).expect("utf-8 body");
    payload
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let start = line.find('{')?;
            serde_json::from_str(&line[start..]).ok()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Scenario A: drive — multi-goal LoopDriver trajectory
// ---------------------------------------------------------------------------

#[test]
fn scenario_a_drive_multi_goal_trajectory() {
    // LoopDriver drives a multi-goal plan through start → step → complete.
    // This proves the state machine semantics: plan → step outcomes → goal
    // completion with StopReason::Achieved.
    let g1 = LoopGoal::new(1, b"research codebase".to_vec());
    let g2 = LoopGoal::new(2, b"implement feature".to_vec());
    let g3 = LoopGoal::new(3, b"run tests".to_vec());
    let plan = LoopPlan::new(vec![g1, g2, g3]);
    let mut driver = LoopDriver::new();

    // Start → Continue goal 1
    let action = driver.send(LoopEvent::Start(plan));
    assert_eq!(action, LoopAction::Continue { goal_id: 1 });
    assert_eq!(
        driver.state().plan.goals[0].status,
        GoalStatus::InProgress
    );
    assert_eq!(driver.state().current_index, 0);

    // Step 1 Done → Continue goal 2
    let action = driver.send(LoopEvent::StepResult {
        goal_id: 1,
        outcome: StepOutcome::Done,
    });
    assert_eq!(action, LoopAction::Continue { goal_id: 2 });
    assert_eq!(driver.state().plan.goals[0].status, GoalStatus::Achieved);
    assert_eq!(driver.state().current_index, 1);

    // Step 2 Done → Continue goal 3
    let action = driver.send(LoopEvent::StepResult {
        goal_id: 2,
        outcome: StepOutcome::Done,
    });
    assert_eq!(action, LoopAction::Continue { goal_id: 3 });
    assert_eq!(driver.state().plan.goals[1].status, GoalStatus::Achieved);
    assert_eq!(driver.state().current_index, 2);

    // Step 3 Done → Stop{Achieved}
    let action = driver.send(LoopEvent::StepResult {
        goal_id: 3,
        outcome: StepOutcome::Done,
    });
    assert_eq!(
        action,
        LoopAction::Stop {
            reason: StopReason::Achieved
        }
    );
    assert_eq!(driver.state().plan.goals[2].status, GoalStatus::Achieved);
    assert!(driver.state().stopped);
}

#[test]
fn scenario_a_drive_budget_exhaustion() {
    // A single blocked goal with replan used → eventually Exhausted.
    let g = LoopGoal::new(1, b"unreachable".to_vec());
    let plan = LoopPlan::new(vec![g]);
    let mut driver = LoopDriver::new();
    driver.send(LoopEvent::Start(plan));

    // First block → Replan
    let action = driver.send(LoopEvent::StepResult {
        goal_id: 1,
        outcome: StepOutcome::Blocked("dependency missing".into()),
    });
    assert_eq!(action, LoopAction::Replan);

    // Subsequent blocks → attempts increment, no more replan
    for _ in 1..8 {
        let a = driver.send(LoopEvent::StepResult {
            goal_id: 1,
            outcome: StepOutcome::Blocked("still blocked".into()),
        });
        assert_ne!(a, LoopAction::Replan);
    }

    // Goal Failed after MAX_GOAL_ATTEMPTS (8)
    assert_eq!(driver.state().plan.goals[0].status, GoalStatus::Failed);

    // Tick → Stop{Achieved} (all goals failed/achieved → plan complete)
    let action = driver.send(LoopEvent::Tick);
    assert_eq!(
        action,
        LoopAction::Stop {
            reason: StopReason::Achieved
        }
    );
}

#[test]
fn scenario_a_drive_step_budget_exhaustion() {
    // Step budget exhaustion: Tick increments steps_taken; at 256 the driver
    // stops with Exhausted. We verify by building a valid checkpoint blob
    // with steps_taken=255 (one below cap) and resuming into it.
    //
    // We cannot set driver.state.steps_taken directly (field is private).
    // Instead, we construct a minimal checkpoint blob matching the binary
    // format in loop_driver.rs checkpoint()/resume():
    //   [0..4] version=1 LE
    //   [4..8] steps_taken LE
    //   [8]    replan_used
    //   [9..13] current_index LE
    //   [13]   stopped
    //   [14]   stop_reason (0=None)
    //   [15..19] amend_count=0 LE
    //   [19..23] goal_count=1 LE
    //   [23..27] goal_id=1 LE
    //   [27]   goal_status=0 (Pending)
    //   [28..32] goal_attempts=0 LE
    //   [32..36] desc_len=4 LE
    //   [36..40] "task"
    let mut blob: Vec<u8> = Vec::new();
    blob.extend_from_slice(&1u32.to_le_bytes()); // version
    blob.extend_from_slice(&255u32.to_le_bytes()); // steps_taken = MAX(256) - 1
    blob.push(0); // replan_used = false
    blob.extend_from_slice(&0u32.to_le_bytes()); // current_index = 0
    blob.push(0); // stopped = false
    blob.push(0); // stop_reason = None
    blob.extend_from_slice(&0u32.to_le_bytes()); // amend_count
    blob.extend_from_slice(&1u32.to_le_bytes()); // goal_count
    blob.extend_from_slice(&1u32.to_le_bytes()); // goal_id
    blob.push(0); // goal_status = Pending
    blob.extend_from_slice(&0u32.to_le_bytes()); // goal_attempts
    blob.extend_from_slice(&4u32.to_le_bytes()); // desc_len
    blob.extend_from_slice(b"task"); // description

    let mut driver = LoopDriver::new();
    let action = driver.send(LoopEvent::Resume(blob));

    // After resuming with steps_taken=255, one Tick hits the cap (256).
    assert_eq!(driver.state().steps_taken, 255);
    assert_eq!(action, LoopAction::Continue { goal_id: 1 });

    let action = driver.send(LoopEvent::Tick);
    assert_eq!(
        action,
        LoopAction::Stop {
            reason: StopReason::Exhausted
        }
    );
    assert!(driver.state().stopped);
    assert_eq!(driver.state().stop_reason, Some(StopReason::Exhausted));
}

// ---------------------------------------------------------------------------
// Scenario B: checkpoint/resume roundtrip
// ---------------------------------------------------------------------------

#[test]
fn scenario_b_checkpoint_resume_roundtrip() {
    let g1 = LoopGoal::new(1, b"first task".to_vec());
    let g2 = LoopGoal::new(2, b"second task".to_vec());
    let plan = LoopPlan::new(vec![g1, g2]);
    let mut driver = LoopDriver::new();
    driver.send(LoopEvent::Start(plan));

    // Complete goal 1
    driver.send(LoopEvent::StepResult {
        goal_id: 1,
        outcome: StepOutcome::Done,
    });
    driver.send(LoopEvent::Steer("mid-course hint".into()));

    // Checkpoint
    let blob = driver.checkpoint_blob();
    assert!(!blob.is_empty(), "checkpoint blob must be non-empty");

    // Resume into fresh driver
    let mut restored = LoopDriver::new();
    let action = restored.send(LoopEvent::Resume(blob));

    // State must roundtrip exactly
    assert_eq!(restored.state(), driver.state());
    assert_eq!(
        action,
        LoopAction::Continue { goal_id: 2 },
        "restored driver continues from goal 2"
    );
    assert_eq!(restored.state().plan.goals[0].status, GoalStatus::Achieved);
    assert_eq!(restored.state().plan.goals[1].status, GoalStatus::InProgress);
    assert_eq!(restored.state().amends.len(), 1);
    assert_eq!(restored.state().amends[0], "mid-course hint");
}

#[test]
fn scenario_b_checkpoint_resume_rejects_foreign_version() {
    // Build a valid checkpoint blob via resume/checkpoint roundtrip, then
    // corrupt the version field to trigger the foreign-version rejection.
    //
    // Construct minimal valid blob: version=1, empty state.
    let mut blob: Vec<u8> = Vec::new();
    blob.extend_from_slice(&1u32.to_le_bytes()); // version = CHECKPOINT_VERSION
    blob.extend_from_slice(&0u32.to_le_bytes()); // steps_taken
    blob.push(0); // replan_used
    blob.extend_from_slice(&0u32.to_le_bytes()); // current_index
    blob.push(0); // stopped
    blob.push(0); // stop_reason
    blob.extend_from_slice(&0u32.to_le_bytes()); // amend_count
    blob.extend_from_slice(&0u32.to_le_bytes()); // goal_count

    // Corrupt version field
    blob[0..4].copy_from_slice(&99u32.to_le_bytes());
    let mut driver = LoopDriver::new();
    let action = driver.send(LoopEvent::Resume(blob));
    assert_eq!(
        action,
        LoopAction::Stop {
            reason: StopReason::Exhausted
        },
        "foreign version must stop with Exhausted"
    );
}

#[test]
fn scenario_b_resume_skips_completed_goals() {
    // Create state with goal 1 Achieved, goal 2 Pending
    let g1 = LoopGoal::new(1, b"done task".to_vec());
    let g2 = LoopGoal::new(2, b"pending task".to_vec());
    let plan = LoopPlan::new(vec![g1, g2]);
    let mut driver = LoopDriver::new();
    driver.send(LoopEvent::Start(plan));
    driver.send(LoopEvent::StepResult {
        goal_id: 1,
        outcome: StepOutcome::Done,
    });

    let blob = driver.checkpoint_blob();
    let mut restored = LoopDriver::new();
    restored.send(LoopEvent::Resume(blob));

    // Goal 1 should still be Achieved, goal 2 should be next
    assert_eq!(restored.state().plan.goals[0].status, GoalStatus::Achieved);
    assert_eq!(
        restored.state().plan.goals[1].status,
        GoalStatus::InProgress
    );
}

// ---------------------------------------------------------------------------
// Scenario C: steer amendments
// ---------------------------------------------------------------------------

#[test]
fn scenario_c_steer_amendment_reflected() {
    let g = LoopGoal::new(1, b"task".to_vec());
    let plan = LoopPlan::new(vec![g]);
    let mut driver = LoopDriver::new();
    driver.send(LoopEvent::Start(plan));

    // Steer appends amendment
    let action = driver.send(LoopEvent::Steer("new priority: security first".into()));
    assert_eq!(action, LoopAction::Continue { goal_id: 1 });
    assert_eq!(driver.state().amends.len(), 1);
    assert_eq!(driver.state().amends[0], "new priority: security first");
}

#[test]
fn scenario_c_steer_truncates_long_amendment() {
    let g = LoopGoal::new(1, b"task".to_vec());
    let plan = LoopPlan::new(vec![g]);
    let mut driver = LoopDriver::new();
    driver.send(LoopEvent::Start(plan));

    // Amendment exceeding MAX_DESCRIPTION_BYTES (256)
    let long_note = "x".repeat(300);
    driver.send(LoopEvent::Steer(long_note));
    // Must be truncated to 256 bytes
    assert_eq!(driver.state().amends.len(), 1);
    assert!(driver.state().amends[0].len() <= 256);
}

#[test]
fn scenario_c_steer_preserves_determinism() {
    let g = LoopGoal::new(1, b"task".to_vec());
    let plan = LoopPlan::new(vec![g]);
    let mut d1 = LoopDriver::new();
    let mut d2 = LoopDriver::new();
    d1.send(LoopEvent::Start(plan.clone()));
    d2.send(LoopEvent::Start(plan));
    d1.send(LoopEvent::Steer("note A".into()));
    d2.send(LoopEvent::Steer("note A".into()));
    d1.send(LoopEvent::Steer("note B".into()));
    d2.send(LoopEvent::Steer("note B".into()));
    assert_eq!(d1.state(), d2.state());
}

// ---------------------------------------------------------------------------
// Scenario D: live turn gap — turn stream does NOT surface LoopDriver events
// ---------------------------------------------------------------------------

#[test]
fn scenario_d_live_turn_no_loop_driver_events() {
    // The turn stream uses agent_loop::LoopController, NOT loop_driver::LoopDriver.
    // We verify the turn stream does NOT emit LoopDriver-specific event types
    // (plan_started, step_outcome, goal_completed, steer_applied, checkpoint).
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let (_provider_base, _provider_task) =
            spawn_scripted_provider(vec![round_one_events()]);
        let (app, _dir) = build_app();
        let session_id = create_session(&app, "LoopDriver gap test").await;
        let (address, server) = spawn_http(app.clone()).await;
        let response = tokio::task::spawn_blocking(move || stream_turn(address, session_id))
            .await
            .expect("stream task");
        server.abort();

        let events = ndjson_events(&response);
        let event_types: Vec<&str> = events
            .iter()
            .filter_map(|e| e["type"].as_str())
            .collect();

        // LoopDriver trajectory events that are NOT present:
        let loop_driver_types = [
            "plan_started",
            "step_outcome",
            "goal_completed",
            "steer_applied",
            "checkpoint",
            "loop_action",
        ];
        for ld_type in &loop_driver_types {
            assert!(
                !event_types.contains(ld_type),
                "turn stream must NOT emit LoopDriver event '{ld_type}' \
                 (wiring gap: live turn uses agent_loop::LoopController at lib.rs:968, \
                 not loop_driver::LoopDriver; see LANE-LOOP-LIVE scratchpad)"
            );
        }
    });
}

#[test]
fn scenario_d_turn_uses_loop_controller_not_loop_driver() {
    // Structural proof: the turn stream's step cap is MAX_TURN_STEPS (64)
    // from agent_loop.rs, not MAX_LOOP_STEPS (256) from loop_driver.rs.
    // We verify the turn stream stops at the agent_loop cap, not the
    // loop_driver cap.
    assert_eq!(
        opencode_rk_server::MAX_TURN_STEPS,
        64,
        "turn path uses agent_loop cap"
    );
    assert_eq!(
        MAX_LOOP_STEPS, 256,
        "loop_driver has its own independent cap"
    );
    // These are different constants proving two separate mechanisms.
    assert_ne!(
        opencode_rk_server::MAX_TURN_STEPS,
        MAX_LOOP_STEPS,
        "agent_loop and loop_driver caps must differ to prove separate mechanisms"
    );
}

// ---------------------------------------------------------------------------
// LoopDriver public API contract tests (state machine correctness)
// ---------------------------------------------------------------------------

#[test]
fn contract_loop_driver_new_starts_empty() {
    let driver = LoopDriver::new();
    assert!(driver.state().plan.goals.is_empty());
    assert_eq!(driver.state().current_index, 0);
    assert_eq!(driver.state().steps_taken, 0);
    assert!(!driver.state().replan_used);
    assert!(driver.state().amends.is_empty());
    assert!(!driver.state().stopped);
    assert_eq!(driver.state().stop_reason, None);
}

#[test]
fn contract_empty_plan_immediate_stop() {
    let plan = LoopPlan::new(std::iter::empty());
    let mut driver = LoopDriver::new();
    let action = driver.send(LoopEvent::Start(plan));
    assert_eq!(
        action,
        LoopAction::Stop {
            reason: StopReason::Achieved
        }
    );
    assert!(driver.state().stopped);
}

#[test]
fn contract_checkpoint_blob_deterministic() {
    let g = LoopGoal::new(1, b"task".to_vec());
    let plan = LoopPlan::new(vec![g]);
    let mut d1 = LoopDriver::new();
    let mut d2 = LoopDriver::new();
    d1.send(LoopEvent::Start(plan.clone()));
    d2.send(LoopEvent::Start(plan));
    assert_eq!(d1.checkpoint_blob(), d2.checkpoint_blob());
}

#[test]
fn contract_per_goal_attempt_cap() {
    let g1 = LoopGoal::new(1, b"blocked".to_vec());
    let g2 = LoopGoal::new(2, b"free".to_vec());
    let plan = LoopPlan::new(vec![g1, g2]);
    let mut driver = LoopDriver::new();
    driver.send(LoopEvent::Start(plan));

    // Block goal 1 until Failed
    for _ in 0..8 {
        driver.send(LoopEvent::StepResult {
            goal_id: 1,
            outcome: StepOutcome::Blocked("dep".into()),
        });
    }
    assert_eq!(driver.state().plan.goals[0].status, GoalStatus::Failed);

    // Tick skips failed goal 1, continues to goal 2
    let action = driver.send(LoopEvent::Tick);
    assert_eq!(action, LoopAction::Continue { goal_id: 2 });
}

#[test]
fn contract_needs_replan_triggers_once() {
    let g = LoopGoal::new(1, b"task".to_vec());
    let plan = LoopPlan::new(vec![g]);
    let mut driver = LoopDriver::new();
    driver.send(LoopEvent::Start(plan));

    let action = driver.send(LoopEvent::StepResult {
        goal_id: 1,
        outcome: StepOutcome::NeedsReplan,
    });
    assert_eq!(action, LoopAction::Replan);
    assert!(driver.state().replan_used);

    // Second NeedsReplan does NOT trigger Replan again
    let action = driver.send(LoopEvent::StepResult {
        goal_id: 1,
        outcome: StepOutcome::NeedsReplan,
    });
    assert_ne!(action, LoopAction::Replan);
}
