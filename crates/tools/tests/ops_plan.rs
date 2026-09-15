use opencode_rk_tools::ops_plan::{DeployTarget, PlanError, parse_target, plan_deploy};

#[test]
fn plan_t01_local() {
    let out = plan_deploy("api", "local").expect("local plan");
    assert_eq!(out, "api@local");
}

#[test]
fn plan_t02_remote() {
    let out = plan_deploy("api", "remote").expect("remote plan");
    assert_eq!(out, "api@remote");
}

#[test]
fn plan_t03_empty() {
    let err = plan_deploy("", "local").expect_err("empty rejected");
    assert!(matches!(err, PlanError::EmptyName));
}

#[test]
fn plan_t04_unknown() {
    let err = plan_deploy("api", "moon").expect_err("unknown rejected");
    assert!(matches!(err, PlanError::UnknownTarget { .. }));
    if let PlanError::UnknownTarget { name } = err {
        assert_eq!(name, "moon");
    }
}

#[test]
fn plan_t05_case() {
    assert!(matches!(parse_target("LOCAL"), Ok(DeployTarget::Local)));
    assert!(matches!(parse_target("Remote"), Ok(DeployTarget::Remote)));
    let out = plan_deploy("api", "LOCAL").expect("case-insensitive");
    assert_eq!(out, "api@local");
}
