//! WEB-008 RED: edit, retry, regenerate and branch semantics (REQ-040).
//! Fork copies parent history through the selected message inclusive into
//! exactly one child, records parent-session plus fork-message provenance,
//! never mutates the parent, and navigates to the child. Retry resolves the
//! user request without appending a fake assistant record; cancel releases
//! the held turn slot. Pure: no IO, no clock, no network.
//! `#[path]` include: `web_008_lane.rs` is owned by this lane and is NOT
//! wired into `lib.rs` (integrator assembles shared files).
#[path = "../src/web_008_lane.rs"]
mod web_008_lane;

use web_008_lane::{
    begin_retry, branch_child_title, cancel_retry, complete_retry, decode_snapshot,
    encode_snapshot, fork_depth, fork_from_message, fork_provenance, keyboard_actions,
    move_focus, open_branch, BranchError, BranchFocus, BranchKey, BranchMessage, BranchRole,
    BranchSession, RetryTurn, TurnState, MAX_ACTIVE_TURNS, MAX_FORK_COPY_MESSAGES, MAX_FORK_DEPTH,
    MAX_TITLE,
};

fn user(id: &str, seq: u64, body: &str) -> BranchMessage {
    BranchMessage {
        id: id.to_owned(),
        seq,
        role: BranchRole::User,
        body: body.to_owned(),
    }
}

fn assistant(id: &str, seq: u64, body: &str) -> BranchMessage {
    BranchMessage {
        id: id.to_owned(),
        seq,
        role: BranchRole::Assistant,
        body: body.to_owned(),
    }
}

fn parent_with_four() -> Vec<BranchSession> {
    vec![BranchSession {
        id: "p".to_owned(),
        title: "Parent chat".to_owned(),
        messages: vec![
            user("u1", 1, "first request"),
            assistant("a2", 2, "first answer"),
            user("u3", 3, "second request"),
            assistant("a4", 4, "second answer"),
        ],
        parent_session_id: None,
        fork_message_id: None,
        fork_seq: None,
    }]
}

// WEB-008-T01: Fork on a persisted message creates exactly one child with
// parent history through that message inclusive, stores parent-session plus
// parent-message provenance, leaves source/later messages unchanged, and
// navigates to the child. Covers user and assistant boundaries.
#[test]
fn web008_t01_real_branch_user_and_assistant() {
    let mut sessions = parent_with_four();

    fork_from_message(&mut sessions, "p", "u3", "c1").unwrap();
    assert_eq!(sessions.len(), 2);
    let child = sessions.iter().find(|s| s.id == "c1").unwrap();
    assert_eq!(
        child.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        vec!["u1", "a2", "u3"]
    );
    assert_eq!(child.title, "Branch: Parent chat");
    let provenance = fork_provenance(child).unwrap();
    assert_eq!(provenance.parent_session_id, "p");
    assert_eq!(provenance.fork_message_id, "u3");
    assert_eq!(provenance.fork_seq, 3);

    let parent = sessions.iter().find(|s| s.id == "p").unwrap();
    assert_eq!(parent.messages.len(), 4);
    assert_eq!(parent.messages[3].id, "a4");
    assert!(fork_provenance(parent).is_none());

    fork_from_message(&mut sessions, "p", "a2", "c2").unwrap();
    let child = sessions.iter().find(|s| s.id == "c2").unwrap();
    assert_eq!(
        child.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        vec!["u1", "a2"]
    );
    let provenance = fork_provenance(child).unwrap();
    assert_eq!(provenance.fork_seq, 2);

    let mut active = "p".to_owned();
    let mut focus = BranchFocus::Popover;
    open_branch(&mut active, &mut focus, "c1").unwrap();
    assert_eq!(active, "c1");
    assert_eq!(focus, BranchFocus::NewSession);
}

// WEB-008-T02: provider failure retains the user-visible branch/history
// state, produces no fake assistant record, and can be retried from a
// deliberate action.
#[test]
fn web008_t02_retry_failure_no_fake_record() {
    let sessions = parent_with_four();
    let history = &sessions[0].messages;
    let mut active: usize = 0;
    let mut turn = RetryTurn::new();

    let request = begin_retry(&mut active, &mut turn, history, "a4").unwrap();
    assert_eq!(request, "second request");
    assert_eq!(turn.state, TurnState::Running);
    assert_eq!(history.len(), 4);

    web_008_lane::fail_retry(&mut active, &mut turn).unwrap();
    assert_eq!(turn.state, TurnState::Failed);
    assert_eq!(active, 0);
    assert_eq!(history.len(), 4);
    assert_eq!(history[3].id, "a4");
    assert!(matches!(
        history.last().unwrap().role,
        BranchRole::Assistant
    ));

    let request = begin_retry(&mut active, &mut turn, history, "u3").unwrap();
    assert_eq!(request, "second request");
    assert_eq!(turn.state, TurnState::Running);
    assert_eq!(history.len(), 4);
}

// WEB-008-T03: edit/retry/branch controls and the branch chooser are
// keyboard/screen-reader operable; Fork has an accessible name, its popover
// announces Branch in new chat, Escape restores focus, and successful
// branching moves navigation to the new session without focus loss.
#[test]
fn web008_t03_accessible_actions_navigation() {
    assert_eq!(web_008_lane::fork_accessible_name(), "Fork");
    assert_eq!(
        web_008_lane::popover_announcement(),
        "Branch in new chat"
    );

    let user_actions = keyboard_actions(BranchRole::User);
    assert!(user_actions.contains(&"edit"));
    assert!(user_actions.contains(&"retry"));
    assert!(user_actions.contains(&"fork"));
    assert!(user_actions.contains(&"branch-in-new-chat"));
    let assistant_actions = keyboard_actions(BranchRole::Assistant);
    assert!(assistant_actions.contains(&"retry"));
    assert!(assistant_actions.contains(&"fork"));

    assert_eq!(
        move_focus(BranchFocus::ForkButton, BranchKey::Enter),
        BranchFocus::Popover
    );
    assert_eq!(
        move_focus(BranchFocus::Popover, BranchKey::Escape),
        BranchFocus::ForkButton
    );
    assert_eq!(
        move_focus(BranchFocus::Popover, BranchKey::Enter),
        BranchFocus::NewSession
    );

    let mut active = "p".to_owned();
    let mut focus = BranchFocus::Popover;
    open_branch(&mut active, &mut focus, "c9").unwrap();
    assert_eq!(active, "c9");
    assert_eq!(focus, BranchFocus::NewSession);
}

// WEB-008-T04: fork depth/history copy limits and turn concurrency limits
// are enforced; cancelling a retry releases its provider/runtime resources.
#[test]
fn web008_t04_bounds_and_cancel() {
    assert_eq!(MAX_FORK_DEPTH, 8);
    assert_eq!(MAX_TITLE, 512);

    let mut sessions = vec![BranchSession {
        id: "s0".to_owned(),
        title: "root".to_owned(),
        messages: vec![user("m1", 1, "hi"), assistant("m2", 2, "hello")],
        parent_session_id: None,
        fork_message_id: None,
        fork_seq: None,
    }];
    let mut parent = "s0".to_owned();
    for depth in 1..=MAX_FORK_DEPTH {
        let child = format!("c{depth}");
        fork_from_message(&mut sessions, &parent, "m2", &child).unwrap();
        parent = child;
    }
    assert_eq!(fork_depth(&sessions, &parent), MAX_FORK_DEPTH);
    let before = sessions.len();
    assert!(matches!(
        fork_from_message(&mut sessions, &parent, "m2", "too-deep"),
        Err(BranchError::DepthExceeded { .. })
    ));
    assert_eq!(sessions.len(), before);

    let mut wide = vec![BranchSession {
        id: "w".to_owned(),
        title: "wide".to_owned(),
        messages: (0..MAX_FORK_COPY_MESSAGES + 1)
            .map(|i| user(&format!("w{i}"), i as u64 + 1, "bulk"))
            .collect(),
        parent_session_id: None,
        fork_message_id: None,
        fork_seq: None,
    }];
    let last = format!("w{}", MAX_FORK_COPY_MESSAGES);
    let before = wide.len();
    assert!(matches!(
        fork_from_message(&mut wide, "w", &last, "wide-child"),
        Err(BranchError::HistoryTooLarge { .. })
    ));
    assert_eq!(wide.len(), before);

    let mut missing = parent_with_four();
    let before = missing.len();
    assert!(matches!(
        fork_from_message(&mut missing, "p", "ghost", "no-child"),
        Err(BranchError::MessageNotFound(_))
    ));
    assert_eq!(missing.len(), before);

    let long = "t".repeat(MAX_TITLE);
    let titled = branch_child_title(&long);
    assert!(titled.len() <= MAX_TITLE);
    assert!(titled.starts_with("Branch: "));

    let sessions = parent_with_four();
    let history = &sessions[0].messages;
    let mut active: usize = 0;
    let mut first = RetryTurn::new();
    let mut second = RetryTurn::new();
    begin_retry(&mut active, &mut first, history, "u1").unwrap();
    assert_eq!(active, 1);
    assert!(matches!(
        begin_retry(&mut active, &mut second, history, "u3"),
        Err(BranchError::TurnBusy)
    ));
    assert_eq!(MAX_ACTIVE_TURNS, 1);
    cancel_retry(&mut active, &mut first).unwrap();
    assert_eq!(first.state, TurnState::Cancelled);
    assert_eq!(active, 0);
    begin_retry(&mut active, &mut second, history, "u3").unwrap();
    assert_eq!(second.state, TurnState::Running);
    complete_retry(&mut active, &mut second, &mut sessions.clone()[0].messages, "a5", "fresh").unwrap();
    assert_eq!(active, 0);
}

// WEB-008-T05: original and branched histories, active branch and fork
// provenance survive reload without duplicate messages.
#[test]
fn web008_t05_reload_fidelity() {
    let mut sessions = parent_with_four();
    fork_from_message(&mut sessions, "p", "a2", "c1").unwrap();
    sessions
        .iter_mut()
        .find(|s| s.id == "c1")
        .unwrap()
        .messages
        .push(user("u9", 3, "branched follow-up"));

    let text = encode_snapshot(&sessions, "c1");
    let back = decode_snapshot(&text).unwrap();
    assert_eq!(back.active_session_id, "c1");
    assert_eq!(back.sessions, sessions);
    let parent = back.sessions.iter().find(|s| s.id == "p").unwrap();
    assert_eq!(parent.messages.len(), 4);
    let child = back.sessions.iter().find(|s| s.id == "c1").unwrap();
    assert_eq!(child.messages.len(), 3);
    let provenance = fork_provenance(child).unwrap();
    assert_eq!(provenance.parent_session_id, "p");
    assert_eq!(provenance.fork_message_id, "a2");

    let again = decode_snapshot(&encode_snapshot(&back.sessions, &back.active_session_id)).unwrap();
    assert_eq!(again.sessions, back.sessions);

    assert_eq!(
        decode_snapshot("not-a-branch-record").unwrap_err(),
        BranchError::BadEncoding
    );
}
