//! OPS-008 frozen tests T01..T05. Self-contained via #[path] include;
//! integrator wires `pub mod ops_repo_ref` into lib.rs later.
#[path = "../src/ops_repo_ref.rs"]
mod ops_repo_ref;

use ops_repo_ref::{
    cache_identity, cache_path, normalize_ref, normalize_ref_with_base, RepoError,
};

#[test]
fn ops_repo_ref_t01_happy_path() {
    let n = normalize_ref("owner/repo", None).unwrap();
    assert_eq!(n.host, "github.com");
    assert_eq!(n.path, "owner/repo");
    assert_eq!(n.branch, "main");
    assert_eq!(n.canonical, "github.com/owner/repo@main");
    let m = normalize_ref("https://example.com/a/b", Some("dev")).unwrap();
    assert_eq!(m.host, "example.com");
    assert_eq!(m.path, "a/b");
    assert_eq!(m.branch, "dev");
    assert_eq!(m.canonical, "example.com/a/b@dev");
    assert_eq!(cache_path("/c", &n), "/c/github.com/owner/repo@main");
    assert_eq!(cache_identity(&n), "github.com/owner/repo@main");
}

#[test]
fn ops_repo_ref_t02_determinism_branch_isolation() {
    let a = normalize_ref("owner/repo", None).unwrap();
    let b = normalize_ref("owner/repo", None).unwrap();
    assert_eq!(a, b);
    assert_eq!(cache_path("/c", &a), cache_path("/c", &b));
    assert_eq!(cache_identity(&a), cache_identity(&b));
    let dev = normalize_ref("owner/repo", Some("dev")).unwrap();
    assert_ne!(cache_path("/c", &a), cache_path("/c", &dev));
    assert_ne!(cache_identity(&a), cache_identity(&dev));
    let mut ids: Vec<String> = (0..32)
        .map(|_| cache_identity(&normalize_ref("owner/repo", None).unwrap()))
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids, vec!["github.com/owner/repo@main".to_string()]);
}

#[test]
fn ops_repo_ref_t03_bounds() {
    let big = "x".repeat(2049);
    assert!(matches!(
        normalize_ref(&big, None),
        Err(RepoError::InputTooLong)
    ));
    let edge = "x".repeat(2048);
    assert!(matches!(
        normalize_ref(&edge, None),
        Err(RepoError::Malformed)
    ));
    let branch = "a".repeat(256);
    assert!(matches!(
        normalize_ref("owner/repo", Some(&branch)),
        Err(RepoError::UnsafeBranch)
    ));
    for _ in 0..10_000 {
        let n = normalize_ref("owner/repo", None).unwrap();
        assert_eq!(n.branch, "main");
    }
}

#[test]
fn ops_repo_ref_t04_failure_states() {
    for bad in ["", "  ", "://bad", "a/"] {
        assert!(
            matches!(normalize_ref(bad, None), Err(RepoError::Malformed)),
            "input {bad:?}"
        );
    }
    for branch in ["../x", "-evil", "a b", "~h", "a*b"] {
        assert!(
            matches!(
                normalize_ref("owner/repo", Some(branch)),
                Err(RepoError::UnsafeBranch)
            ),
            "branch {branch:?}"
        );
    }
}

#[test]
fn ops_repo_ref_t05_env_isolation_safety() {
    let before = normalize_ref("owner/repo", None).unwrap();
    std::env::set_var(
        "OPS_008_DECOY_GITHUB_BASE_URL",
        "https://decoy.example.invalid/s3cr3t",
    );
    let after = normalize_ref("owner/repo", None).unwrap();
    assert_eq!(before, after);
    assert_eq!(after.host, "github.com");
    std::env::remove_var("OPS_008_DECOY_GITHUB_BASE_URL");
    let based = normalize_ref_with_base("owner/repo", None, Some("ghe.example.com")).unwrap();
    assert_eq!(based.host, "ghe.example.com");
    assert_eq!(based.canonical, "ghe.example.com/owner/repo@main");
    let plain_equiv = normalize_ref_with_base("owner/repo", None, None).unwrap();
    assert_eq!(plain_equiv, before);
    let explicit =
        normalize_ref_with_base("https://example.com/a/b", Some("dev"), Some("ghe.example.com"))
            .unwrap();
    assert_eq!(explicit.host, "example.com");
    assert_eq!(explicit.branch, "dev");
    let secret = "s3cr3t-t0ken-xyz";
    let overlong = format!("{secret}{}", "y".repeat(2049));
    let err = normalize_ref(&overlong, None).unwrap_err();
    let rendered = format!("{err:?}|{err}");
    assert!(!rendered.contains(secret), "error leaks input bytes");
    let bad_branch = format!("{secret} branch");
    let err2 = normalize_ref("owner/repo", Some(&bad_branch)).unwrap_err();
    let rendered2 = format!("{err2:?}|{err2}");
    assert!(!rendered2.contains(secret), "error leaks input bytes");
}
