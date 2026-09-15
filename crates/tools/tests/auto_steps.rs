use opencode_rk_tools::auto_steps::{MAX_STEPS, StepsError, plan_steps};

#[test]
fn steps_t01_valid() {
    let out = plan_steps(&["build", "test", "ship"]).expect("valid steps");
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].name, "build");
    assert_eq!(out[1].name, "test");
    assert_eq!(out[2].name, "ship");
}

#[test]
fn steps_t02_empty() {
    let err = plan_steps(&["build", ""]).expect_err("empty rejected");
    assert!(matches!(err, StepsError::EmptyName));
}

#[test]
fn steps_t03_dup() {
    let err = plan_steps(&["build", "test", "build"]).expect_err("dup rejected");
    assert!(matches!(err, StepsError::DuplicateName { .. }));
    if let StepsError::DuplicateName { name } = err {
        assert_eq!(name, "build");
    }
}

#[test]
fn steps_t04_overflow() {
    let owned: Vec<String> = (0..MAX_STEPS + 1).map(|i| format!("step-{i}")).collect();
    let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
    let err = plan_steps(&refs).expect_err("overflow rejected");
    assert!(matches!(err, StepsError::TooManySteps { .. }));
    if let StepsError::TooManySteps { max, actual } = err {
        assert_eq!(max, MAX_STEPS);
        assert_eq!(actual, MAX_STEPS + 1);
    }
}

#[test]
fn steps_t05_order_index() {
    let out = plan_steps(&["a", "b", "c"]).expect("valid steps");
    assert_eq!(out[0].order, 0);
    assert_eq!(out[1].order, 1);
    assert_eq!(out[2].order, 2);
}
