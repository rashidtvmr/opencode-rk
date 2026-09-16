//! SHARE-004 frozen tests T01..T05: local secret-free share-metadata lifecycle.
//!
//! `#[path]` include: `share_store.rs` is owned by this lane and is NOT wired
//! into `lib.rs` (integrator assembles shared files).

#[path = "../src/share_store.rs"]
mod share_store;

use opencode_rk_contracts::SessionId;
use share_store::{ShareId, ShareSecret, ShareStore, StoreError, MAX_SHARES};

fn session() -> SessionId {
    SessionId::new()
}

fn share_id(tag: &str) -> ShareId {
    ShareId::new(tag)
}

fn secret(bytes: &str) -> ShareSecret {
    ShareSecret::new(bytes.as_bytes().to_vec())
}

const URL_A: &str = "https://share.example.com/s/abc123";

// SHARE-004-T01 (lifecycle happy path).
#[test]
fn share004_t01_lifecycle_happy_path() {
    let mut store = ShareStore::new();
    assert!(store.is_empty());
    let sess = session();
    let sec = secret("t01-owner-secret");
    let meta = store.create(sess, share_id("sh-t01"), URL_A, &sec).unwrap();
    assert_eq!(meta.share_id.as_str(), "sh-t01");
    assert_eq!(meta.public_url, URL_A);
    assert_eq!(meta.last_synced_seq, 0);
    let got = store.get(sess).unwrap();
    assert_eq!(got.share_id, share_id("sh-t01"));
    assert_eq!(got.public_url, URL_A);
    assert_eq!(store.len(), 1);
    store.mark_synced(sess, 7).unwrap();
    assert_eq!(store.get(sess).unwrap().last_synced_seq, 7);
    store.remove(sess).unwrap();
    assert!(store.get(sess).is_none());
    assert!(store.is_empty());
}

// SHARE-004-T02 (cascade + duplicate).
#[test]
fn share004_t02_duplicate_and_cascade() {
    let mut store = ShareStore::new();
    let sess = session();
    let other = session();
    let sec = secret("t02-owner-secret");
    store
        .create(sess, share_id("sh-first"), URL_A, &sec)
        .unwrap();
    store
        .create(other, share_id("sh-other"), URL_A, &sec)
        .unwrap();
    let err = store
        .create(
            sess,
            share_id("sh-second"),
            "https://share.example.com/s/other",
            &sec,
        )
        .unwrap_err();
    assert_eq!(err, StoreError::AlreadyShared);
    // Existing record unchanged.
    let got = store.get(sess).unwrap();
    assert_eq!(got.share_id.as_str(), "sh-first");
    assert_eq!(got.public_url, URL_A);
    // Session deletion cascades only its own record.
    store.remove_session(sess);
    assert!(store.get(sess).is_none());
    assert!(store.get(other).is_some());
    assert_eq!(store.len(), 1);
}

// SHARE-004-T03 (failure states).
#[test]
fn share004_t03_failure_states() {
    let mut store = ShareStore::new();
    let sec = secret("t03-owner-secret");
    let unknown = session();
    assert_eq!(
        store.mark_synced(unknown, 1).unwrap_err(),
        StoreError::NotFound
    );
    assert_eq!(store.remove(unknown).unwrap_err(), StoreError::NotFound);
    // Seq regression rejected, marker unchanged.
    let sess = session();
    store.create(sess, share_id("sh-r"), URL_A, &sec).unwrap();
    store.mark_synced(sess, 9).unwrap();
    assert_eq!(
        store.mark_synced(sess, 4).unwrap_err(),
        StoreError::SeqRegression
    );
    assert_eq!(store.get(sess).unwrap().last_synced_seq, 9);
    // Bad URLs rejected, nothing stored.
    for bad in [
        "http://share.example.com/s/x",
        "notaurl",
        "",
        "https://",
        "https://exa mple.com/s",
    ] {
        let s = session();
        assert_eq!(
            store.create(s, share_id("sh-bad"), bad, &sec).unwrap_err(),
            StoreError::InvalidUrl,
            "url {bad:?}"
        );
        assert!(store.get(s).is_none());
    }
    // Oversized URL rejected.
    let long = format!("https://example.com/{}", "x".repeat(2048));
    assert!(long.len() > 2048);
    let s = session();
    assert_eq!(
        store
            .create(s, share_id("sh-long"), &long, &sec)
            .unwrap_err(),
        StoreError::InvalidUrl
    );
    assert!(store.get(s).is_none());
    // Oversized store rejected.
    let mut full = ShareStore::new();
    for i in 0..MAX_SHARES {
        full.create(session(), ShareId::new(format!("sh-{i:04}")), URL_A, &sec)
            .unwrap();
    }
    assert_eq!(full.len(), MAX_SHARES);
    assert_eq!(
        full.create(session(), share_id("sh-over"), URL_A, &sec)
            .unwrap_err(),
        StoreError::Full
    );
    assert_eq!(full.len(), MAX_SHARES);
}

// SHARE-004-T04 (no secret retention).
#[test]
fn share004_t04_no_secret_retention() {
    const CANARY: &str = "t04-super-secret-canary-9Zq7";
    let mut store = ShareStore::new();
    let sess = session();
    let mut sec = secret(CANARY);
    store.create(sess, share_id("sh-s"), URL_A, &sec).unwrap();
    // Owner zeroizes after the call.
    sec.zeroize();
    assert!(sec.as_bytes().iter().all(|b| *b == 0));
    // Store memory/debug carries zero secret bytes.
    let store_dbg = format!("{:?}", store);
    assert!(!store_dbg.contains(CANARY), "leak: {store_dbg}");
    let meta_dbg = format!("{:?}", store.get(sess).unwrap());
    assert!(!meta_dbg.contains(CANARY), "leak: {meta_dbg}");
    let err_dbg = format!(
        "{:?}",
        store
            .create(sess, share_id("sh-dup"), URL_A, &secret(CANARY))
            .unwrap_err()
    );
    assert!(!err_dbg.contains(CANARY), "leak: {err_dbg}");
    // Record itself is intact and secret-free by construction.
    let got = store.get(sess).unwrap();
    assert_eq!(got.public_url, URL_A);
    assert_eq!(got.share_id.as_str(), "sh-s");
}

// SHARE-004-T05 (safety + determinism).
#[test]
fn share004_t05_safety_determinism() {
    const SECRET: &str = "t05-owner-secret-canary-K7";
    const QUERY: &str = "qry-canary-t05-ABC";
    const FRAG: &str = "frg-canary-t05-XYZ";
    let dir = tempfile::tempdir().unwrap();
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, "untouched").unwrap();
    let url = format!("https://share.example.com/s/item?tok={QUERY}#{FRAG}");
    let build = || {
        let mut st = ShareStore::new();
        let sec = ShareSecret::new(SECRET.as_bytes().to_vec());
        let a: SessionId = "11111111-1111-1111-1111-111111111111".parse().unwrap();
        let b: SessionId = "22222222-2222-2222-2222-222222222222".parse().unwrap();
        st.create(a, ShareId::new("sh-a"), &url, &sec).unwrap();
        st.create(b, ShareId::new("sh-b"), URL_A, &sec).unwrap();
        st.mark_synced(a, 5).unwrap();
        st.remove(b).unwrap();
        st
    };
    let first = build();
    let second = build();
    // Same op sequence => identical stores.
    assert_eq!(first, second);
    assert_eq!(format!("{:?}", first), format!("{:?}", second));
    // Full URL retained in storage (debug redacts query/fragment).
    let a: SessionId = "11111111-1111-1111-1111-111111111111".parse().unwrap();
    assert!(first.get(a).unwrap().public_url.contains(QUERY));
    // Captured logs carry zero secret bytes and zero URL query/fragment bytes.
    let logs = vec![
        format!("{:?}", first),
        format!("{:?}", StoreError::AlreadyShared),
        format!("{:?}", StoreError::NotFound),
        format!("{:?}", StoreError::SeqRegression),
        format!("{:?}", StoreError::InvalidUrl),
        format!("{:?}", StoreError::Full),
    ];
    for line in &logs {
        assert!(!line.contains(SECRET), "secret leak: {line}");
        assert!(!line.contains(QUERY), "query leak: {line}");
        assert!(!line.contains(FRAG), "fragment leak: {line}");
    }
    // No writes outside the disposable fixture dir.
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "untouched");
}
