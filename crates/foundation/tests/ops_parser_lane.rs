//! OPS-006 frozen tests T01..T05 for ops_parser_lane. #[path] include;
//! owns exactly one src file, never touches lib.rs / Cargo.toml.

#[path = "../src/ops_parser_lane.rs"]
mod ops_parser_lane;

use ops_parser_lane::{ops_lane_basename, ops_lane_failure_hint, ops_lane_normalize, OpsLaneError, OpsLaneRoot};

fn fixture_root() -> OpsLaneRoot {
    OpsLaneRoot::new("target/fixtures/ops006-lane-cache").expect("fixture root")
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[test]
fn ops006_t01_happy_path() {
    let root = fixture_root();
    let out = ops_lane_normalize("owner/repo", None, &root).expect("happy path");
    assert_eq!(out.branch, "main");
    assert_eq!(out.canonical, "owner/repo");
    assert_eq!(out.cache_path, root.join("owner/repo").join("main"));
    assert_eq!(out.cache_id.len(), 64);
    assert!(is_hex64(&out.cache_id), "cache_id must be 64 hex chars");
}

#[test]
fn ops006_t02_determinism_and_isolation() {
    let root = fixture_root();
    let run1 = ops_lane_normalize("owner/repo", None, &root).expect("run1");
    let run2 = ops_lane_normalize("owner/repo", None, &root).expect("run2");
    assert_eq!(run1, run2);
    assert_eq!(run1.cache_id, run2.cache_id);
    let main_out = ops_lane_normalize("owner/repo", Some("main"), &root).expect("main");
    let dev_out = ops_lane_normalize("owner/repo", Some("dev"), &root).expect("dev");
    assert_ne!(main_out.cache_id, dev_out.cache_id);
    assert_ne!(main_out.cache_path, dev_out.cache_path);
}

#[test]
fn ops006_t03_caps() {
    let root = fixture_root();
    let raw257 = "a".repeat(257);
    assert_eq!(
        ops_lane_normalize(&raw257, None, &root),
        Err(OpsLaneError::Invalid)
    );
    let branch129 = "b".repeat(129);
    assert_eq!(
        ops_lane_normalize("owner/repo", Some(&branch129), &root),
        Err(OpsLaneError::UnsafeBranch)
    );
    let root1025 = "r".repeat(1025);
    assert_eq!(OpsLaneRoot::new(&root1025), Err(OpsLaneError::BadCacheRoot));
}

#[test]
fn ops006_t04_failure_states() {
    let root = fixture_root();
    assert_eq!(ops_lane_normalize("", None, &root), Err(OpsLaneError::Invalid));
    assert_eq!(
        ops_lane_normalize(":::bad", None, &root),
        Err(OpsLaneError::Invalid)
    );
    assert_eq!(
        ops_lane_normalize("owner/repo", Some("../evil"), &root),
        Err(OpsLaneError::UnsafeBranch)
    );
    assert_eq!(
        ops_lane_normalize("owner/repo", Some("a/b"), &root),
        Err(OpsLaneError::UnsafeBranch)
    );
    assert_eq!(
        OpsLaneRoot::new("cache/../evil"),
        Err(OpsLaneError::BadCacheRoot)
    );
    assert_eq!(OpsLaneRoot::new(".."), Err(OpsLaneError::BadCacheRoot));
}

#[test]
fn ops006_t05_purity_and_safety() {
    let root = fixture_root();
    let before = ops_lane_normalize("owner/repo", None, &root).expect("before");
    unsafe { std::env::set_var("OPS006_LANE_PROBE_UNRELATED", "tamper") };
    let after = ops_lane_normalize("owner/repo", None, &root).expect("after");
    unsafe { std::env::remove_var("OPS006_LANE_PROBE_UNRELATED") };
    assert_eq!(before, after);
    assert!(is_hex64(&after.cache_id));

    let probe_root = OpsLaneRoot::new("target/fixtures/ops006-lane-probe-7f3a").expect("probe root");
    let probe = ops_lane_normalize("owner/repo", Some("main"), &probe_root).expect("probe");
    assert!(!std::path::Path::new(probe.cache_path.as_str()).exists());

    let bad_raw = ":::bad-secret-abc123";
    let err = ops_lane_normalize(bad_raw, None, &root).expect_err("must fail");
    assert_eq!(err, OpsLaneError::Invalid);
    let hint = ops_lane_failure_hint(bad_raw, &err);
    assert!(!hint.contains(bad_raw), "hint must not echo full ref");
    assert!(hint.contains(&ops_lane_basename(bad_raw)));
    assert!(!hint.contains(":::"));
}
