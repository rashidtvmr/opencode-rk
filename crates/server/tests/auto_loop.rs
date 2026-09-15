use opencode_rk_server::auto_loop::{parse_loop_state, LoopError, LoopState, LoopTracker};

#[test]
fn loop_t01_parse() {
    assert_eq!(parse_loop_state("idle"), Ok(LoopState::Idle));
    assert_eq!(parse_loop_state("RUNNING"), Ok(LoopState::Running));
    assert_eq!(parse_loop_state("Stopped"), Ok(LoopState::Stopped));
}

#[test]
fn loop_t02_tick_cycle() {
    let mut t = LoopTracker::new();
    assert_eq!(t.ticks, 0);
    t.state = "running".to_string();
    t.tick().unwrap();
    assert_eq!(t.ticks, 1);
    t.tick().unwrap();
    assert_eq!(t.ticks, 2);
    assert_eq!(t.get_state(), Ok(LoopState::Running));
}

#[test]
fn loop_t03_tick_idle_rejected() {
    let mut t = LoopTracker::new();
    assert!(matches!(t.tick(), Err(LoopError::NotRunning)));
    assert_eq!(t.ticks, 0);
}

#[test]
fn loop_t04_stop_cycle() {
    let mut t = LoopTracker::new();
    t.state = "running".to_string();
    t.stop().unwrap();
    assert_eq!(t.get_state(), Ok(LoopState::Stopped));
    assert!(matches!(t.stop(), Err(LoopError::NotRunning)));
    assert!(matches!(t.tick(), Err(LoopError::NotRunning)));
}

#[test]
fn loop_t05_unknown_rejected() {
    let _ = LoopError::AlreadyRunning;
    assert!(matches!(
        parse_loop_state("bogus"),
        Err(LoopError::NotRunning)
    ));
    assert!(matches!(parse_loop_state(""), Err(LoopError::NotRunning)));
}
