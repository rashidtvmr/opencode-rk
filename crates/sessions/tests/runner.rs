// RUN-001 contract tests: bounded per-session runner with busy rejection.
// Maps to obligations RUN-001-T01..T05 in tasks/RUN-001.md.
use opencode_rk_sessions::runner::{RunKind, RunnerError, RunnerSet, MAX_STEPS};

#[test]
fn run001_t01_happy_path() {
    let mut set = RunnerSet::new();
    let normal = set.start("sess-1", RunKind::Normal).unwrap();
    assert!(matches!(
        set.status("sess-1"),
        opencode_rk_sessions::runner::RunnerState::Running { .. }
    ));
    let shell = set.start("sess-1", RunKind::Shell).unwrap();
    assert_eq!(set.live_count("sess-1"), 2);
    normal.complete();
    shell.complete();
    assert!(matches!(
        set.status("sess-1"),
        opencode_rk_sessions::runner::RunnerState::Idle
    ));
    assert_eq!(set.live_count("sess-1"), 0);
}

#[test]
fn run001_t02_busy_rejection() {
    let mut set = RunnerSet::new();
    let first = set.start("sess-1", RunKind::Normal).unwrap();
    let err = set.start("sess-1", RunKind::Normal).unwrap_err();
    assert!(matches!(err, RunnerError::Busy { .. }));
    assert_eq!(set.live_count("sess-1"), 1);
    assert!(matches!(
        set.status("sess-1"),
        opencode_rk_sessions::runner::RunnerState::Running { .. }
    ));
    drop(first);
    assert_eq!(set.live_count("sess-1"), 0);
    let shell1 = set.start("sess-1", RunKind::Shell).unwrap();
    assert!(set.start("sess-1", RunKind::Shell).is_err());
    shell1.complete();
}

#[test]
fn run001_t03_cancel_drop() {
    let mut set = RunnerSet::new();
    let g = set.start("sess-1", RunKind::Normal).unwrap();
    g.cancel();
    assert!(matches!(
        set.status("sess-1"),
        opencode_rk_sessions::runner::RunnerState::Idle
    ));
    {
        let _shell = set.start("sess-2", RunKind::Shell).unwrap();
    }
    assert_eq!(set.live_count("sess-2"), 0);
    assert!(matches!(
        set.status("sess-2"),
        opencode_rk_sessions::runner::RunnerState::Idle
    ));
    assert_eq!(set.cancel_all("ghost"), 0);
}

#[test]
fn run001_t04_validation_steps() {
    let mut set = RunnerSet::new();
    assert!(matches!(
        set.start("", RunKind::Normal),
        Err(RunnerError::InvalidInput)
    ));
    let mut g = set.start("sess-1", RunKind::Normal).unwrap();
    assert_eq!(g.steps_remaining(), MAX_STEPS);
    for _ in 0..MAX_STEPS {
        g.take_step().unwrap();
    }
    assert_eq!(g.steps_remaining(), 0);
    assert!(g.take_step().is_err());
    g.complete();
}

#[test]
fn run001_t05_no_side_effect_safety() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("marker");
    let mut set = RunnerSet::new();
    let g = set.start("sess-1", RunKind::Normal).unwrap();
    assert!(!marker.exists());
    let dbg = format!("{g:?} {set:?}");
    assert!(!dbg.contains("prompt"));
    g.complete();
    assert!(!marker.exists());
}
