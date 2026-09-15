use opencode_rk_agents::turn_state::{
    TurnId, TurnPhase, TurnStateError, TurnSubmission, TurnSubmissionState,
};

fn submission(id: u64) -> TurnSubmission {
    TurnSubmission::new(TurnId::new(id), format!("prompt-{id}"))
}

#[test]
fn auto_007_t01_submit_owns_one_pending_turn() {
    let mut state = TurnSubmissionState::new();
    let turn = submission(1);

    state
        .submit(turn.clone())
        .expect("idle state should accept one turn");

    assert_eq!(state.phase(), TurnPhase::Submitted);
    assert_eq!(state.active(), Some(&turn));
}

#[test]
fn auto_007_t02_start_moves_submitted_turn_to_running() {
    let mut state = TurnSubmissionState::new();
    let turn = submission(2);
    let id = turn.id();
    state.submit(turn.clone()).expect("submit should succeed");

    state.start(id).expect("submitted turn should start");

    assert_eq!(state.phase(), TurnPhase::Running);
    assert_eq!(state.active(), Some(&turn));
}

#[test]
fn auto_007_t03_complete_releases_active_turn_and_allows_next_submit() {
    let mut state = TurnSubmissionState::new();
    let first = submission(3);
    let first_id = first.id();
    state.submit(first).expect("submit should succeed");
    state.start(first_id).expect("start should succeed");

    state
        .complete(first_id)
        .expect("running turn should complete");

    assert_eq!(state.phase(), TurnPhase::Idle);
    assert_eq!(state.active(), None);

    let next = submission(4);
    state
        .submit(next.clone())
        .expect("completion cleanup should release the single active slot");
    assert_eq!(state.active(), Some(&next));
}

#[test]
fn auto_007_t04_interrupt_releases_active_turn_and_allows_next_submit() {
    let mut state = TurnSubmissionState::new();
    let first = submission(5);
    let first_id = first.id();
    state.submit(first).expect("submit should succeed");
    state.start(first_id).expect("start should succeed");

    state
        .interrupt(first_id)
        .expect("running turn should be interruptible");

    assert_eq!(state.phase(), TurnPhase::Idle);
    assert_eq!(state.active(), None);

    let next = submission(6);
    state
        .submit(next.clone())
        .expect("interrupt cleanup should release the single active slot");
    assert_eq!(state.active(), Some(&next));
}

#[test]
fn auto_007_t05_duplicate_and_invalid_transitions_are_typed_and_non_mutating() {
    let mut state = TurnSubmissionState::new();
    let first = submission(7);
    let first_id = first.id();
    state
        .submit(first.clone())
        .expect("first submit should succeed");

    let duplicate = state
        .submit(submission(8))
        .expect_err("a second active submission must not be queued");
    assert_eq!(
        duplicate,
        TurnStateError::AlreadyActive { active: first_id }
    );
    assert_eq!(state.phase(), TurnPhase::Submitted);
    assert_eq!(state.active(), Some(&first));

    let invalid_complete = state
        .complete(first_id)
        .expect_err("submitted turn cannot complete before it starts");
    assert_eq!(
        invalid_complete,
        TurnStateError::InvalidTransition {
            from: TurnPhase::Submitted,
            operation: "complete",
        }
    );
    assert_eq!(state.phase(), TurnPhase::Submitted);
    assert_eq!(state.active(), Some(&first));

    let wrong_id = TurnId::new(999);
    let invalid_start = state
        .start(wrong_id)
        .expect_err("only the owned submitted turn may start");
    assert_eq!(
        invalid_start,
        TurnStateError::WrongTurn {
            active: first_id,
            requested: wrong_id,
        }
    );
    assert_eq!(state.phase(), TurnPhase::Submitted);
    assert_eq!(state.active(), Some(&first));
}
