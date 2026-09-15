//! OPS-006 frozen tests: offline repository-reference normalization.
//! Uses #[path] include so the lane owns exactly one src file and never
//! touches shared lib.rs / Cargo.toml.

#[path = "../src/repo_ref_ext.rs"]
mod repo_ref_ext;

use repo_ref_ext::{failure_hint, normalize_reference, safe_basename, RefError, RelPath};

fn fixture_root() -> RelPath {
    RelPath::new("target/fixtures/ops006-cache").expect("fixture root")
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[test]
fn ops006_t01_happy_path() {
    let root = fixture_root();
    let out = normalize_reference("owner/repo", None, &root).expect("happy path");
    assert_eq!(out.branch, "main");
    assert_eq!(out.canonical, "owner/repo");
    assert_eq!(out.cache_path, root.join("owner/repo").join("main"));
    assert_eq!(out.cache_id.len(), 64);
    assert!(is_hex64(&out.cache_id), "cache_id must be 64 hex chars");
}

#[test]
fn ops006_t02_determinism_and_isolation() {
    let root = fixture_root();
    let run1 = normalize_reference("owner/repo", None, &root).expect("run1");
    let run2 = normalize_reference("owner/repo", None, &root).expect("run2");
    assert_eq!(run1, run2);
    assert_eq!(run1.cache_id, run2.cache_id);
    let main_out = normalize_reference("owner/repo", Some("main"), &root).expect("main");
    let dev_out = normalize_reference("owner/repo", Some("dev"), &root).expect("dev");
    assert_ne!(main_out.cache_id, dev_out.cache_id);
    assert_ne!(main_out.cache_path, dev_out.cache_path);
}

#[test]
fn ops006_t03_caps() {
    let root = fixture_root();
    let raw257 = "a".repeat(257);
    assert_eq!(
        normalize_reference(&raw257, None, &root),
        Err(RefError::Invalid)
    );
    let branch129 = "b".repeat(129);
    assert_eq!(
        normalize_reference("owner/repo", Some(&branch129), &root),
        Err(RefError::UnsafeBranch)
    );
    let root1025 = "r".repeat(1025);
    assert_eq!(RelPath::new(&root1025), Err(RefError::BadCacheRoot));
}

#[test]
fn ops006_t04_failure_states() {
    let root = fixture_root();
    assert_eq!(normalize_reference("", None, &root), Err(RefError::Invalid));
    assert_eq!(
        normalize_reference(":::bad", None, &root),
        Err(RefError::Invalid)
    );
    assert_eq!(
        normalize_reference("owner/repo", Some("../evil"), &root),
        Err(RefError::UnsafeBranch)
    );
    assert_eq!(
        normalize_reference("owner/repo", Some("a/b"), &root),
        Err(RefError::UnsafeBranch)
    );
    assert_eq!(
        RelPath::new("cache/../evil"),
        Err(RefError::BadCacheRoot)
    );
    assert_eq!(RelPath::new(".."), Err(RefError::BadCacheRoot));
}

#[test]
fn ops006_t05_purity_and_safety() {
    // Deterministic pure compute: env changes must not affect output.
    let root = fixture_root();
    let before = normalize_reference("owner/repo", None, &root).expect("before");
    unsafe { std::env::set_var("OPS006_PROBE_UNRELATED", "tamper") };
    let after = normalize_reference("owner/repo", None, &root).expect("after");
    unsafe { std::env::remove_var("OPS006_PROBE_UNRELATED") };
    assert_eq!(before, after);
    assert!(is_hex64(&after.cache_id));

    // No filesystem mutation: impl constructs the path but never creates it.
    let probe_root = RelPath::new("target/fixtures/ops006-purity-probe-7f3a").expect("probe root");
    let probe = normalize_reference("owner/repo", Some("main"), &probe_root).expect("probe");
    assert!(!std::path::Path::new(probe.cache_path.as_str()).exists());

    // Secret safety: hint carries failure kind + basename only, never full ref.
    let bad_raw = ":::bad-secret-abc123";
    let err = normalize_reference(bad_raw, None, &root).expect_err("must fail");
    assert_eq!(err, RefError::Invalid);
    let hint = failure_hint(bad_raw, &err);
    assert!(!hint.contains(bad_raw), "hint must not echo full ref");
    assert!(hint.contains(&safe_basename(bad_raw)));
    assert!(!hint.contains(":::"));
}
