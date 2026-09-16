//! WEB-007 FROZEN RED: transcript geometry + per-message action bars.
//! Server-side geometry/capability model only; `web/` DOM owned elsewhere.
//! Includes the lane module via path so shared `lib.rs` is never touched.
#[path = "../src/transcript_lane.rs"]
mod transcript_lane;

use transcript_lane::{
    animation_ms, geometry_for, is_branch_boundary, normalize_persisted_role, popover_items,
    reading_order, row_actions, trunc_preview, ActionId, FocusTarget, MessageRole, PopoverState,
};

// WEB-007-T01: rows full-width, role alignment, below-message bar with Fork.
#[test]
fn web007_t01_row_action_happy_path() {
    let user = geometry_for(MessageRole::User, 0);
    assert!(user.full_width);
    assert_eq!(user.align_right, true);
    assert!(user.actions_below);
    let asst = geometry_for(MessageRole::Assistant, 1);
    assert!(asst.full_width);
    assert_eq!(asst.align_right, false);
    assert!(asst.actions_below);
    for role in [MessageRole::User, MessageRole::Assistant] {
        let acts = row_actions(role, true);
        assert!(acts.iter().any(|a| a.id == ActionId::Fork && a.visible));
        assert_eq!(popover_items(ActionId::Fork), &["Branch in new chat"]);
    }
}

// WEB-007-T02: unavailable actions disabled/omitted with reason, never fake-ok.
#[test]
fn web007_t02_unavailable_action_honesty() {
    let acts = row_actions(MessageRole::Assistant, false);
    let retry = acts.iter().find(|a| a.id == ActionId::Retry).unwrap();
    assert!(!retry.enabled);
    assert!(retry.explanation.is_some());
    assert!(!retry.simulated_success);
    // Fork stays available: it has a real backend boundary (branch creation).
    assert!(acts.iter().any(|a| a.id == ActionId::Fork && a.enabled));
}

// WEB-007-T03: chronological reading order, named actions, fork kb open/esc.
#[test]
fn web007_t03_accessibility() {
    let roles = [MessageRole::User, MessageRole::Assistant, MessageRole::User];
    assert_eq!(reading_order(&roles), vec![0, 1, 2]);
    for role in [MessageRole::User, MessageRole::Assistant] {
        for a in row_actions(role, true) {
            assert!(!a.accessible_name.is_empty(), "action {:?} unnamed", a.id);
        }
    }
    let mut pop = PopoverState::closed();
    pop.open_fork();
    assert!(pop.open);
    assert_eq!(pop.focus, FocusTarget::PopoverFirstItem);
    pop.escape();
    assert!(!pop.open);
    assert_eq!(pop.focus, FocusTarget::ForkTrigger);
}

// WEB-007-T04: bounds on large messages/actions; reduced-motion respected.
#[test]
fn web007_t04_bounds_and_reduced_motion() {
    let big = "x".repeat(200_000);
    let prev = trunc_preview(&big);
    assert!(prev.len() <= transcript_lane::MAX_PREVIEW_BYTES + 32);
    assert!(prev.ends_with("..."));
    let acts = row_actions(MessageRole::Assistant, true);
    assert!(acts.len() <= transcript_lane::MAX_ACTIONS);
    assert_eq!(animation_ms(true), 0);
    assert!(animation_ms(false) > 0);
}

// WEB-007-T05: legacy persisted + fresh streamed rows share geometry.
#[test]
fn web007_t05_persistence_render_compat() {
    for legacy in ["user", "assistant"] {
        let role = normalize_persisted_role(legacy).expect("legacy role known");
        let streamed = if legacy == "user" {
            MessageRole::User
        } else {
            MessageRole::Assistant
        };
        assert_eq!(geometry_for(role, 3), geometry_for(streamed, 9));
        assert!(is_branch_boundary(role));
    }
    assert!(normalize_persisted_role("system").is_some());
    assert!(normalize_persisted_role("bogus-role").is_none());
}
