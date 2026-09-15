use opencode_rk_sessions::ui_006::{
    build_panel, StatusCounts, StatusPanelError, MAX_PANEL_SESSIONS,
};

#[test]
fn ui006_t01_healthy_panel() {
    let c = StatusCounts {
        sessions: 2,
        active_turns: 1,
        errors: 0,
    };
    let panel = build_panel("Status", &c).unwrap();
    assert_eq!(panel.title, "Status");
    assert_eq!(panel.counts.sessions, 2);
    assert_eq!(panel.counts.active_turns, 1);
    assert_eq!(panel.counts.errors, 0);
    assert!(panel.healthy);
}

#[test]
fn ui006_t02_unhealthy_on_errors() {
    let c = StatusCounts {
        sessions: 2,
        active_turns: 1,
        errors: 3,
    };
    let panel = build_panel("Status", &c).unwrap();
    assert!(!panel.healthy);
}

#[test]
fn ui006_t03_empty_title_rejected() {
    let c = StatusCounts {
        sessions: 1,
        active_turns: 0,
        errors: 0,
    };
    assert!(matches!(
        build_panel("", &c),
        Err(StatusPanelError::EmptyTitle)
    ));
    assert!(matches!(
        build_panel("   ", &c),
        Err(StatusPanelError::EmptyTitle)
    ));
}

#[test]
fn ui006_t04_overflow_rejected() {
    let c = StatusCounts {
        sessions: MAX_PANEL_SESSIONS + 1,
        active_turns: 0,
        errors: 0,
    };
    match build_panel("Status", &c) {
        Err(StatusPanelError::TooManySessions { max, actual }) => {
            assert_eq!(max, MAX_PANEL_SESSIONS);
            assert_eq!(actual, MAX_PANEL_SESSIONS + 1);
        }
        other => panic!("expected TooManySessions, got {other:?}"),
    }
}

#[test]
fn ui006_t05_boundary_accepted() {
    let c = StatusCounts {
        sessions: MAX_PANEL_SESSIONS,
        active_turns: 5,
        errors: 0,
    };
    let panel = build_panel("  padded  ", &c).unwrap();
    assert_eq!(panel.title, "padded");
    assert_eq!(panel.counts.sessions, MAX_PANEL_SESSIONS);
    assert!(panel.healthy);
}
