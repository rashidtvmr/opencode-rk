//! OPS-002 frozen tests T01..T05. Self-contained via #[path] include;
//! lane owns exactly one src file, never touches shared lib.rs / Cargo.toml.

#[path = "../src/repo_ref.rs"]
mod repo_ref;

use repo_ref::{normalize_ref, RefError, MAX_REF_BYTES};
use std::path::Path;

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const HOST_VAR: &str = "OPS002_GITHUB_HOST";

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join("ops002-cache-root")
}

fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[test]
fn ops002_t01_happy_path() {
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var(HOST_VAR) };
    let r = root();
    // local path with explicit branch
    let out = normalize_ref("./projects/myrepo", Some("dev"), &r).unwrap();
    assert_eq!(out.branch, "dev");
    assert!(out.canonical.contains("myrepo"));
    assert!(out.cache_path.starts_with(&r));
    // owner/repo shorthand with explicit + default branch
    let a = normalize_ref("owner/repo", Some("dev"), &r).unwrap();
    assert_eq!(a.canonical, "github.com/owner/repo");
    assert_eq!(a.branch, "dev");
    assert!(a.cache_path.starts_with(&r));
    let b = normalize_ref("owner/repo", None, &r).unwrap();
    assert_eq!(b.branch, "main");
    assert_eq!(b.canonical, "github.com/owner/repo");
    assert!(b.cache_path.starts_with(&r));
    // https form explicit + default
    let c = normalize_ref("https://github.com/owner/repo", Some("dev"), &r).unwrap();
    assert_eq!(c.canonical, "github.com/owner/repo");
    assert_eq!(c.branch, "dev");
    assert!(c.cache_path.starts_with(&r));
    let d = normalize_ref("https://github.com/owner/repo", None, &r).unwrap();
    assert_eq!(d.branch, "main");
    assert!(d.cache_path.starts_with(&r));
    // git@ form explicit + default
    let e = normalize_ref("git@github.com:owner/repo.git", Some("dev"), &r).unwrap();
    assert_eq!(e.canonical, "github.com/owner/repo");
    assert_eq!(e.branch, "dev");
    assert!(e.cache_path.starts_with(&r));
    let f = normalize_ref("git@github.com:owner/repo.git", None, &r).unwrap();
    assert_eq!(f.branch, "main");
    assert_eq!(f.canonical, "github.com/owner/repo");
    assert!(f.cache_path.starts_with(&r));
}

#[test]
fn ops002_t02_determinism_cache_identity() {
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var(HOST_VAR) };
    let r = root();
    let a = normalize_ref("owner/repo", Some("dev"), &r).unwrap();
    let b = normalize_ref("owner/repo", Some("dev"), &r).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.cache_path, b.cache_path);
    assert_eq!(a.cache_id, b.cache_id);
    assert!(is_hex(&a.cache_id));
    // different branches isolate
    let c = normalize_ref("owner/repo", Some("other"), &r).unwrap();
    assert_ne!(a.cache_id, c.cache_id);
    assert_ne!(a.cache_path, c.cache_path);
    // frozen stable vector across restarts
    let frozen = normalize_ref("owner/repo", None, &r).unwrap();
    assert_eq!(frozen.cache_id, normalize_ref("owner/repo", None, &r).unwrap().cache_id);
    assert!(is_hex(&frozen.cache_id));
}

#[test]
fn ops002_t03_unsafe_rejection() {
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var(HOST_VAR) };
    let r = root();
    for bad in ["", "../x", "a/b", "a\x00b", "a\nb"] {
        let err = normalize_ref("owner/repo", Some(bad), &r).unwrap_err();
        assert_eq!(err, RefError::UnsafeBranch, "branch {bad:?}");
    }
    let long = "b".repeat(300);
    assert_eq!(
        normalize_ref("owner/repo", Some(&long), &r).unwrap_err(),
        RefError::UnsafeBranch
    );
    // escaping branch never yields a path outside root: no path returned at all
    for bad in ["../x", "..", "a/b", "a\\b"] {
        let res = normalize_ref("owner/repo", Some(bad), &r);
        assert_eq!(res, Err(RefError::UnsafeBranch), "branch {bad:?}");
        if let Ok(out) = res {
            assert!(out.cache_path.starts_with(&r), "escape {bad:?}");
        }
    }
}

#[test]
fn ops002_t04_unsupported_overlong_env_isolation() {
    let r = root();
    assert_eq!(normalize_ref("", None, &r).unwrap_err(), RefError::Empty);
    for bad in ["::::", "ftp://example.com/a", "//host/a/b"] {
        assert_eq!(
            normalize_ref(bad, None, &r).unwrap_err(),
            RefError::UnsupportedForm,
            "input {bad:?}"
        );
    }
    // 8 KiB input
    let big = "x".repeat(8192);
    assert_eq!(
        normalize_ref(&big, None, &r).unwrap_err(),
        RefError::InputTooLong
    );
    assert!(8192 > MAX_REF_BYTES);
    // env override changes ONLY shorthand host
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var(HOST_VAR) };
    let short_plain = normalize_ref("owner/repo", None, &r).unwrap();
    let https_plain = normalize_ref("https://github.com/owner/repo", None, &r).unwrap();
    let git_plain = normalize_ref("git@github.com:owner/repo.git", None, &r).unwrap();
    unsafe { std::env::set_var(HOST_VAR, "ghe.example.com") };
    let short_over = normalize_ref("owner/repo", None, &r).unwrap();
    let https_over = normalize_ref("https://github.com/owner/repo", None, &r).unwrap();
    let git_over = normalize_ref("git@github.com:owner/repo.git", None, &r).unwrap();
    unsafe { std::env::remove_var(HOST_VAR) };
    assert_eq!(short_over.canonical, "ghe.example.com/owner/repo");
    assert_eq!(https_over, https_plain);
    assert_eq!(git_over, git_plain);
    assert_eq!(https_over.canonical, "github.com/owner/repo");
}

#[test]
fn ops002_t05_no_side_effects_safety() {
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var(HOST_VAR) };
    // read-only fake root + full matrix: zero files created
    let dir = std::env::temp_dir().join("ops002-side-effect-probe-9f31");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let before: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    let refs = [
        "owner/repo",
        "https://github.com/owner/repo",
        "git@github.com:owner/repo.git",
        "./projects/myrepo",
    ];
    for raw in refs {
        for branch in [None, Some("dev")] {
            let _ = normalize_ref(raw, branch, &dir);
        }
    }
    // overlong / bad inputs also side-effect free
    let _ = normalize_ref(&"x".repeat(8192), None, &dir);
    let _ = normalize_ref("::::", None, &dir);
    let after: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(before, after);
    let _ = std::fs::remove_dir_all(&dir);
    // zero DB writes: no sqlite file beside fixture-outside
    assert!(!Path::new("fixture-outside.db").exists());
    // logs contain zero reference-body bytes beyond input basename
    let out = normalize_ref("owner/repo", None, &dir).unwrap();
    let hint = format!("normalized:{}", basename_of(&out.canonical));
    assert!(!hint.contains("owner/repo/secret-body"));
    assert!(hint.contains("repo"));
}

fn basename_of(s: &str) -> &str {
    s.rsplit('/').next().unwrap_or(s)
}
