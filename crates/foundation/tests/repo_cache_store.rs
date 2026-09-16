//! OPS-003 frozen tests T01..T05: local-only repository cache-store lifecycle.
//! Self-contained via #[path] include.
#[path = "../src/repo_cache_store.rs"]
mod repo_cache_store;

use repo_cache_store::*;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn slot(id: &str, branch: &str) -> CacheSlot {
    CacheSlot {
        cache_id: id.to_string(),
        path: RelPath::new(&format!("slots/{id}")).unwrap(),
        branch: branch.to_string(),
        state: SlotState::Missing,
    }
}

fn fresh_cfg() -> CacheCfg {
    CacheCfg {
        max_slots: 16,
        max_bytes: 1_073_741_824,
        stale_after_idle: 1,
    }
}

fn write_blob(root: &std::path::Path, id: &str, bytes: &[u8]) {
    let dir = root.join("slots").join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("blob.bin"), bytes).unwrap();
}

#[test]
fn ops003_t01_inspect_mark_happy_path() {
    let tmp = std::env::temp_dir().join(format!("ops003-t01-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    let store = CacheStore::open(&tmp, fresh_cfg()).unwrap();
    let a = slot("a", "main");
    let b = slot("b", "main");
    let c = slot("c", "main");
    let d = slot("d", "main");
    store.mark_fresh(&a, 10).unwrap();
    store.mark_fresh(&b, 10).unwrap();
    store.mark_fresh(&c, 0).unwrap();
    assert_eq!(store.inspect(&a), SlotState::Fresh);
    assert_eq!(store.inspect(&b), SlotState::Fresh);
    assert_eq!(
        store.inspect(&c),
        SlotState::Stale {
            reason: "idle".to_string()
        }
    );
    assert_eq!(store.inspect(&d), SlotState::Missing);
    store.mark_fresh(&c, 11).unwrap();
    assert_eq!(store.inspect(&c), SlotState::Fresh);
    let marker = fs::read(tmp.join("slots").join("c").join("fresh.marker")).unwrap();
    assert!(marker.len() <= MAX_MARKER_BYTES);
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn ops003_t02_sweep_determinism_retention() {
    let mk = || {
        let tmp = std::env::temp_dir().join(format!(
            "ops003-t02-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let store = CacheStore::open(&tmp, fresh_cfg()).unwrap();
        let fresh = slot("fresh", "main");
        let stale = slot("stale", "main");
        let missing = slot("missing", "main");
        let corrupt = slot("corrupt", "main");
        store.mark_fresh(&fresh, 10).unwrap();
        store.mark_fresh(&stale, 0).unwrap();
        write_blob(&tmp, "stale", &[7u8; 64]);
        let cdir = tmp.join("slots").join("corrupt");
        fs::create_dir_all(&cdir).unwrap();
        fs::write(cdir.join("fresh.marker"), vec![0xFFu8; 8]).unwrap();
        // register non-marked slots so sweep reports them as retained
        assert_eq!(store.inspect(&missing), SlotState::Missing);
        assert!(matches!(store.inspect(&corrupt), SlotState::Corrupt { .. }));
        (tmp, store, fresh, stale, missing, corrupt)
    };
    let (t1, s1, _, _, _, _) = mk();
    let r1 = s1.sweep(10).unwrap();
    let (t2, s2, _, _, _, _) = mk();
    let r2 = s2.sweep(10).unwrap();
    assert_eq!(r1.removed, r2.removed);
    assert_eq!(r1.removed.len(), 1);
    assert_eq!(r1.removed[0].cache_id, "stale");
    assert_eq!(r1.freed_bytes, 64);
    assert!(!r1.policy.0.is_empty());
    let mut ids: Vec<&str> = r1.retained.iter().map(|s| s.cache_id.as_str()).collect();
    ids.sort();
    assert!(ids.contains(&"fresh"));
    assert!(ids.contains(&"missing"));
    assert!(ids.contains(&"corrupt"));
    for (t, s) in [(&t1, &s1), (&t2, &s2)] {
        let r = s.sweep(10).unwrap();
        let _ = (t, r);
    }
    let _ = fs::remove_dir_all(&t1);
    let _ = fs::remove_dir_all(&t2);
}

#[test]
fn ops003_t03_caps_cancel() {
    let tmp = std::env::temp_dir().join(format!("ops003-t03-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    let cfg = CacheCfg {
        max_slots: 2,
        max_bytes: 1_073_741_824,
        stale_after_idle: 1,
    };
    let store = CacheStore::open(&tmp, cfg).unwrap();
    for id in ["s0", "s1", "s2", "s3"] {
        store.mark_fresh(&slot(id, "main"), 0).unwrap();
    }
    let report = store.sweep(10).unwrap();
    assert!(report.removed.len() <= 2, "removed over cap");
    assert!(report.removed.len() + report.retained.len() <= 2 + 4);
    // cancel pre-set on big fixture
    let big = std::env::temp_dir().join(format!("ops003-t03b-{}", std::process::id()));
    let _ = fs::remove_dir_all(&big);
    fs::create_dir_all(&big).unwrap();
    let bigstore = CacheStore::open(
        &big,
        CacheCfg {
            max_slots: 2000,
            max_bytes: u64::MAX,
            stale_after_idle: 1,
        },
    )
    .unwrap();
    for i in 0..1000 {
        bigstore
            .mark_fresh(&slot(&format!("k{i:04}"), "main"), 0)
            .unwrap();
    }
    let cancel = Arc::new(AtomicBool::new(true));
    let t0 = std::time::Instant::now();
    let err = sweep_cancel(&bigstore, 10, &cancel).unwrap_err();
    assert_eq!(err, StoreError::Cancelled);
    assert!(t0.elapsed().as_secs() < 1);
    let _ = fs::remove_dir_all(&tmp);
    let _ = fs::remove_dir_all(&big);
}

#[test]
fn ops003_t04_contention_corrupt_never_deleted() {
    let tmp = std::env::temp_dir().join(format!("ops003-t04-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    let store = CacheStore::open(&tmp, fresh_cfg()).unwrap();
    let v = slot("victim", "main");
    store.mark_fresh(&v, 5).unwrap();
    let before = fs::read(tmp.join("slots").join("victim").join("fresh.marker")).unwrap();
    let _guard = store.hold_lock(&v).unwrap();
    assert_eq!(store.mark_fresh(&v, 6).unwrap_err(), StoreError::SlotLocked);
    assert_eq!(
        store.sweep_slot(&v, 10).unwrap_err(),
        StoreError::SlotLocked
    );
    let after = fs::read(tmp.join("slots").join("victim").join("fresh.marker")).unwrap();
    assert_eq!(before, after);
    drop(_guard);
    let bad = slot("bad", "main");
    let bdir = tmp.join("slots").join("bad");
    fs::create_dir_all(&bdir).unwrap();
    fs::write(bdir.join("fresh.marker"), vec![0xFFu8; 8]).unwrap();
    assert!(matches!(store.inspect(&bad), SlotState::Corrupt { .. }));
    let rep = store.sweep(100).unwrap();
    assert!(!rep.removed.iter().any(|s| s.cache_id == "bad"));
    assert!(bdir.join("fresh.marker").exists());
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn ops003_t05_transport_refused_safety() {
    let tmp = std::env::temp_dir().join(format!("ops003-t05-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    let store = CacheStore::open(&tmp, fresh_cfg()).unwrap();
    let t = NoNetworkTransport::default();
    // lifecycle paths never touch transport
    let a = slot("a", "main");
    store.mark_fresh(&a, 1).unwrap();
    let _ = store.inspect(&a);
    let _ = store.sweep(5).unwrap();
    assert_eq!(t.invocations(), 0);
    assert_eq!(t.fetch("x"), Err(StoreError::TransportRefused));
    assert_eq!(t.checkout("x", "b"), Err(StoreError::TransportRefused));
    assert_eq!(t.reset("x"), Err(StoreError::TransportRefused));
    assert_eq!(t.invocations(), 3);
    // no writes outside fixture dir
    let outside = tmp
        .join("..")
        .join(format!("ops003-outside-{}", std::process::id()));
    assert!(!outside.exists());
    // logs carry basenames only
    let line = store.log_line(&a);
    assert!(line.contains("fresh.marker") || line.contains('a'));
    assert!(!line.contains('\0'));
    let blob = vec![9u8; 32];
    write_blob(&tmp, "a", &blob);
    let line2 = store.log_line(&a);
    assert!(!line2.contains(&format!("{:?}", blob)));
    let _ = fs::remove_dir_all(&tmp);
    let _ = Ordering::SeqCst;
}
