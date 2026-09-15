use opencode_rk_sessions::ui_008::{Approval, ApprovalCard};

#[test]
fn ui008_t01_allow() {
    let mut card = ApprovalCard::new("bash").unwrap();
    assert_eq!(card.decide(Approval::Allow), "allowed:bash");
    assert!(!card.is_pending());
}

#[test]
fn ui008_t02_deny() {
    let mut card = ApprovalCard::new("write").unwrap();
    assert_eq!(card.decide(Approval::Deny), "denied:write");
    assert!(!card.is_pending());
}

#[test]
fn ui008_t03_empty_rejected() {
    assert!(matches!(
        ApprovalCard::new(""),
        Err(opencode_rk_sessions::ui_008::ApprovalError::EmptyTool)
    ));
}

#[test]
fn ui008_t04_pending_flag() {
    let card = ApprovalCard::new("read").unwrap();
    assert!(card.pending);
    assert!(card.is_pending());
}

#[test]
fn ui008_t05_decide_twice_keeps_decided() {
    let mut card = ApprovalCard::new("bash").unwrap();
    assert_eq!(card.decide(Approval::Allow), "allowed:bash");
    assert_eq!(card.decide(Approval::Allow), "allowed:bash");
    assert!(!card.is_pending());
    assert!(!card.pending);
}
