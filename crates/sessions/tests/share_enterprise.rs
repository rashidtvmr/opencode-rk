//! Frozen RED suite for SHARE-003 enterprise-remote boundary.
//!
//! Task: tasks/SHARE-003.md. Observable contract: every `EnterpriseOp`
//! resolves to `Err(BoundaryError::Refused { op, partition, reason })`
//! naming the missing in-surface specification partition. Pure local
//! lifecycle only: no network, no credentials, no DB mutation, no logging.

#[path = "../src/share_enterprise.rs"]
mod share_enterprise;

use share_enterprise::{
    EnterpriseBoundary, EnterpriseOp, partition, refusal_reason, BoundaryError,
};

/// All five ops with their exact required gap partitions
/// (sources/enterprise-remote-spec-gap.json
/// `requiredDecompositionPartitions`).
const ALL_OPS: [(EnterpriseOp, &str); 5] = [
    (EnterpriseOp::ShareHttp, "enterprise-share-http"),
    (
        EnterpriseOp::SyncServerRelay,
        "function-syncserver-websocket-r2",
    ),
    (EnterpriseOp::SupportRelay, "function-support-relay"),
    (
        EnterpriseOp::GithubTokenExchange,
        "function-github-token-exchange-installation",
    ),
    (
        EnterpriseOp::DeploymentLifecycle,
        "deployment-resource-lifecycle",
    ),
];

/// SHARE-003-T01: every op variant is refused with its exact partition.
#[test]
fn share003_t01_all_ops_refused() {
    for (op, expected_partition) in ALL_OPS {
        let err = match EnterpriseBoundary::authorize(op) {
            Ok(()) => panic!("op {op:?} must be refused, got Ok"),
            Err(e) => e,
        };
        assert!(
            matches!(err, BoundaryError::Refused { .. }),
            "op {op:?} must refuse with BoundaryError::Refused, got {err:?}"
        );
        match err {
            BoundaryError::Refused {
                op: got_op,
                partition: got_partition,
                ..
            } => {
                assert_eq!(got_op, op, "refusal must echo the attempted op");
                assert_eq!(
                    got_partition, expected_partition,
                    "wrong partition for op {op:?}"
                );
            }
        }
    }
}

/// SHARE-003-T02: partition mapping is exact; reason names the missing
/// `spec` kind and cites the spec-gap file.
#[test]
fn share003_t02_partition_mapping_and_reason() {
    for (op, expected) in ALL_OPS {
        assert_eq!(partition(op), expected, "wrong partition for op {op:?}");
        let reason = refusal_reason(op);
        assert!(
            reason.contains("`spec`"),
            "reason for {op:?} must name missing kind `spec`, got: {reason}"
        );
        assert!(
            reason.contains("enterprise-remote-spec-gap"),
            "reason for {op:?} must cite the spec-gap file, got: {reason}"
        );
    }
    for (op, _) in ALL_OPS {
        let Err(BoundaryError::Refused { reason, .. }) =
            EnterpriseBoundary::authorize(op)
        else {
            panic!("op {op:?} must be refused");
        };
        assert!(
            reason.contains("`spec`"),
            "authorize error reason for {op:?} must name missing kind `spec`"
        );
    }
}

/// Snapshot a fixture dir: sorted (relative path, bytes) pairs.
fn snapshot_dir(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("read fixture dir")
            .map(|e| e.expect("dir entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .expect("prefix")
                    .to_string_lossy()
                    .into_owned();
                let bytes = std::fs::read(&path).expect("read fixture file");
                out.push((rel, bytes));
            }
        }
    }
    out.sort();
    out
}

/// Best-effort open-handle count (Linux `/proc/self/fd`).
fn open_handle_count() -> Option<usize> {
    std::fs::read_dir("/proc/self/fd")
        .ok()
        .map(|entries| entries.count())
}

/// SHARE-003-T03: attempted ops leave no file trace and open no new
/// handles; the fixture dir is byte-identical before and after.
#[test]
fn share003_t03_no_side_effects() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    std::fs::write(dir.path().join("sentinel.txt"), b"fixture-bytes").unwrap();
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("inner.txt"), b"inner-bytes").unwrap();

    let before = snapshot_dir(dir.path());
    let handles_before = open_handle_count();
    for (op, _) in ALL_OPS {
        let _ = EnterpriseBoundary::authorize(op);
        let _ = partition(op);
        let _ = refusal_reason(op);
    }
    let after = snapshot_dir(dir.path());
    let handles_after = open_handle_count();

    assert_eq!(
        before, after,
        "boundary must leave zero file trace in the fixture dir"
    );
    assert_eq!(
        handles_before, handles_after,
        "boundary must open zero new handles"
    );
}

/// SHARE-003-T04: support-admin removal is refused without consuming any
/// bearer input, and legacy-vs-org endpoint selection is refused rather
/// than defaulting to either endpoint.
#[test]
fn share003_t04_support_admin_and_endpoint_selection() {
    // No bearer parameter exists: authorize takes only the op value, so no
    // bearer check can be performed locally. Refusal carries no credential.
    let Err(BoundaryError::Refused { partition, .. }) =
        EnterpriseBoundary::authorize(EnterpriseOp::SupportRelay)
    else {
        panic!("support relay (admin removal) must be refused");
    };
    assert_eq!(partition, "function-support-relay");

    // Legacy-vs-org selection is refused under enterprise-share-http; the
    // error carries no endpoint (no URL, no default), so the caller cannot
    // silently pick one.
    let Err(BoundaryError::Refused {
        partition, reason, ..
    }) = EnterpriseBoundary::authorize(EnterpriseOp::ShareHttp)
    else {
        panic!("share-http selection must be refused, never defaulted");
    };
    assert_eq!(partition, "enterprise-share-http");
    assert!(
        !reason.contains("://"),
        "refusal must not name or select an endpoint, got: {reason}"
    );
}

/// SHARE-003-T05: repeated authorize calls are identical; rendered errors
/// carry zero URL/secret bytes (only op + partition names).
#[test]
fn share003_t05_safety_and_determinism() {
    for (op, _) in ALL_OPS {
        let first = EnterpriseBoundary::authorize(op).unwrap_err();
        let second = EnterpriseBoundary::authorize(op).unwrap_err();
        assert_eq!(
            first, second,
            "repeated authorize must be deterministic for {op:?}"
        );
        let rendered = format!("{first} {first:?}");
        assert!(
            rendered.contains(partition(op)),
            "rendered refusal must name the partition for {op:?}"
        );
        for marker in [
            "://",
            "bearer",
            "ghp_",
            "gho_",
            "sk-",
            "api_key",
            "passwd",
            "password",
        ] {
            assert!(
                !rendered.to_lowercase().contains(marker),
                "rendered refusal for {op:?} must not carry URL/secret bytes (found {marker:?}): {rendered}"
            );
        }
        // A fake secret living next to the caller must not leak into logs.
        assert!(!rendered.contains("sentinel-secret-9f3k7q"));
    }
}
