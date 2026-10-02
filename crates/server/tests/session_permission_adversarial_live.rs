//! Adversarial live permission lifecycle contracts.
#![forbid(unsafe_code)]

#[path = "support/s1_adversarial.rs"]
mod s1_adversarial;

#[tokio::test]
async fn reject_permission_does_not_continue_later_provider_tool_call() {
    s1_adversarial::reject_does_not_continue().await;
}

#[tokio::test]
async fn interrupt_cancels_pending_turn_and_reclaims_slot() {
    s1_adversarial::interrupt_pending_and_reclaims().await;
}
