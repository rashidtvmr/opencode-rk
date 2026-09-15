use opencode_rk_server::desktop_bridge::{parse_channel, topic_for, BridgeError, Channel};

#[test]
fn bridge_t01_stable() {
    let ch = parse_channel("stable").unwrap();
    assert!(matches!(ch, Channel::Stable));
    let ch = parse_channel("  stable  ").unwrap();
    assert!(matches!(ch, Channel::Stable));
}

#[test]
fn bridge_t02_beta_case() {
    let ch = parse_channel("BETA").unwrap();
    assert!(matches!(ch, Channel::Beta));
    let ch = parse_channel("  Beta  ").unwrap();
    assert!(matches!(ch, Channel::Beta));
}

#[test]
fn bridge_t03_unknown_rejected() {
    let r = parse_channel("canary");
    assert!(matches!(r, Err(BridgeError::UnknownChannel { .. })));
    let r = parse_channel("  ");
    assert!(matches!(r, Err(BridgeError::UnknownChannel { .. })));
}

#[test]
fn bridge_t04_empty_topic() {
    let ch = Channel::Stable;
    assert!(matches!(topic_for(&ch, ""), Err(BridgeError::EmptyTopic)));
    assert!(matches!(topic_for(&ch, "   "), Err(BridgeError::EmptyTopic)));
}

#[test]
fn bridge_t05_topic_format() {
    assert_eq!(topic_for(&Channel::Stable, "events").unwrap(), "stable:events");
    assert_eq!(
        topic_for(&Channel::Beta, "  events  ").unwrap(),
        "beta:events"
    );
}
