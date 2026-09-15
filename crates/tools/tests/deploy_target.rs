use opencode_rk_tools::deploy_target::{DeployTarget2, DeployTargetError, qualify_target};

#[test]
fn dpt_t01_local() {
    assert!(matches!(
        qualify_target("local"),
        Ok(DeployTarget2::Local)
    ));
}

#[test]
fn dpt_t02_remote() {
    assert!(matches!(
        qualify_target("remote"),
        Ok(DeployTarget2::Remote)
    ));
}

#[test]
fn dpt_t03_empty() {
    assert!(matches!(
        qualify_target(""),
        Err(DeployTargetError::EmptyTarget)
    ));
}

#[test]
fn dpt_t04_unknown() {
    match qualify_target("moon") {
        Err(DeployTargetError::UnknownTarget { name }) => assert_eq!(name, "moon"),
        other => panic!("expected UnknownTarget, got {other:?}"),
    }
}

#[test]
fn dpt_t05_case() {
    assert!(matches!(
        qualify_target("LOCAL"),
        Ok(DeployTarget2::Local)
    ));
    assert!(matches!(
        qualify_target("Remote"),
        Ok(DeployTarget2::Remote)
    ));
}
