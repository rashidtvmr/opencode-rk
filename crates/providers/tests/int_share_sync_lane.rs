//! INT-010 frozen tests T01..T05: share-descriptor validation + last-write-wins merge.
//!
//! Fallback lane file: implementation under test is included by path so this
//! lane never edits the shared `crates/providers/src/lib.rs`
//! (integrator-owned). Distinct from `share_descriptor.rs` (same slice
//! fallback, no collision).

#[path = "../src/int_share_sync_lane.rs"]
mod int_share_sync_lane;

use int_share_sync_lane::{
    HostedKind, ShareDesc, ShareError, ShareSync, SyncOp, MAX_ID_LEN, MAX_KEYREF_LEN,
    MAX_SHARES,
};

fn desc(id: &str, key_ref: &str, digest: &str, seq: u64) -> ShareDesc {
    ShareDesc {
        id: id.to_owned(),
        key_ref: key_ref.to_owned(),
        digest: digest.to_owned(),
        seq,
    }
}

fn hex_digest(byte: u8) -> String {
    format!("{:02x}", byte).repeat(32)
}

#[test]
fn int_010_t01_upsert_replaces_on_fresh_seq_last_write_wins() {
    let mut sync = ShareSync::new();
    let first_digest = hex_digest(0xab);
    let view = sync
        .apply(SyncOp::Upsert(desc("a", "k1", &first_digest, 1)))
        .expect("upsert seq 1 should succeed");
    assert_eq!(view.map(|v| v.seq), Some(1));

    let latest_digest = hex_digest(0xcd);
    let view = sync
        .apply(SyncOp::Upsert(desc("a", "k1", &latest_digest, 3)))
        .expect("upsert seq 3 should succeed")
        .expect("fresh upsert returns the new view");
    assert_eq!(view.seq, 3);
    assert_eq!(view.digest, latest_digest);
    assert_eq!(view.key_ref, "k1");

    let list = sync.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].digest, latest_digest);
    assert_eq!(sync.get("a"), Some(view));
}

#[test]
fn int_010_t02_stale_upsert_and_remove_keep_stored_fresh_remove_deletes() {
    let mut sync = ShareSync::new();
    let seed_digest = hex_digest(0x01);
    sync.apply(SyncOp::Upsert(desc("a", "k1", &seed_digest, 5)))
        .expect("seed seq 5 should succeed");

    let kept = sync
        .apply(SyncOp::Upsert(desc("a", "k1", &hex_digest(0x02), 4)))
        .expect("stale upsert stays Ok");
    assert_eq!(kept.map(|v| v.seq), Some(5));
    assert_eq!(sync.get("a").map(|v| v.seq), Some(5));
    assert_eq!(sync.get("a").map(|v| v.digest), Some(seed_digest));

    let kept = sync
        .apply(SyncOp::Remove {
            id: "a".to_owned(),
            seq: 5,
        })
        .expect("stale remove stays Ok");
    assert_eq!(kept.map(|v| v.seq), Some(5));
    assert!(sync.get("a").is_some());

    assert_eq!(
        sync.apply(SyncOp::Remove {
            id: "a".to_owned(),
            seq: 6,
        })
        .expect("fresh remove should succeed"),
        None
    );
    assert_eq!(sync.get("a"), None);

    assert_eq!(
        sync.apply(SyncOp::Remove {
            id: "ghost".to_owned(),
            seq: 1,
        })
        .expect("unknown-id remove is harmless"),
        None
    );
    assert!(sync.list().is_empty());
}

#[test]
fn int_010_t03_validation_and_overflow_leave_store_byte_identical() {
    assert_eq!(MAX_SHARES, 256);
    assert_eq!(MAX_ID_LEN, 64);
    assert_eq!(MAX_KEYREF_LEN, 64);

    let mut sync = ShareSync::new();
    sync.apply(SyncOp::Upsert(desc("a", "k1", &hex_digest(0x09), 1)))
        .expect("seed should succeed");
    let before = format!("{:?}", sync.list());

    let long_id = "a".repeat(MAX_ID_LEN + 1);
    for bad in [
        "",
        "!",
        "-lead",
        ".lead",
        "_lead",
        "has space",
        "slash/x",
        long_id.as_str(),
    ] {
        let err = sync
            .apply(SyncOp::Upsert(desc(bad, "k1", &hex_digest(0x09), 2)))
            .expect_err("bad id must be typed");
        assert_eq!(err, ShareError::InvalidId);
        assert_eq!(format!("{:?}", sync.list()), before);
    }

    let long_key = "k".repeat(MAX_KEYREF_LEN + 1);
    for bad in [
        "",
        long_key.as_str(),
        "bad/key",
        "has space",
        "-lead",
    ] {
        let err = sync
            .apply(SyncOp::Upsert(desc("b", bad, &hex_digest(0x09), 1)))
            .expect_err("bad key_ref must be typed");
        assert_eq!(err, ShareError::SecretInvalid);
        assert_eq!(format!("{:?}", sync.list()), before);
    }

    let short_digest = "a".repeat(63);
    let long_digest = "a".repeat(65);
    let non_hex = "g".repeat(64);
    for bad in [
        "",
        "abc",
        "zzzz",
        short_digest.as_str(),
        long_digest.as_str(),
        non_hex.as_str(),
    ] {
        let err = sync
            .apply(SyncOp::Upsert(desc("c", "k1", bad, 1)))
            .expect_err("bad digest must be typed");
        assert_eq!(err, ShareError::InvalidDigest);
        assert_eq!(format!("{:?}", sync.list()), before);
    }

    let err = sync
        .apply(SyncOp::Remove {
            id: String::new(),
            seq: 9,
        })
        .expect_err("empty remove id must be typed");
    assert_eq!(err, ShareError::InvalidId);
    assert_eq!(format!("{:?}", sync.list()), before);

    let mut full = ShareSync::new();
    for index in 0..MAX_SHARES {
        full.apply(SyncOp::Upsert(desc(
            &format!("s-{index:03}"),
            "k",
            &hex_digest(index as u8),
            1,
        )))
        .expect("exact share bound should succeed");
    }
    assert_eq!(full.list().len(), MAX_SHARES);
    let before_full = format!("{:?}", full.list());
    let err = full
        .apply(SyncOp::Upsert(desc(
            "overflow-1",
            "k",
            &hex_digest(0x07),
            1,
        )))
        .expect_err("new id past the cap must overflow");
    assert_eq!(err, ShareError::Overflow);
    assert_eq!(full.list().len(), MAX_SHARES);
    assert_eq!(format!("{:?}", full.list()), before_full);

    let updated = full
        .apply(SyncOp::Upsert(desc("s-000", "k", &hex_digest(0x08), 2)))
        .expect("update of an existing id needs no new slot");
    assert_eq!(updated.map(|v| v.seq), Some(2));
    assert_eq!(full.list().len(), MAX_SHARES);
}

#[test]
fn int_010_t04_all_hosted_partitions_refused_by_type_store_unchanged() {
    let mut sync = ShareSync::new();
    let key_ref = "keyref-PROBE-7";
    let seed_digest = hex_digest(0xee);
    sync.apply(SyncOp::Upsert(desc("probe-a", key_ref, &seed_digest, 1)))
        .expect("seed should succeed");
    let before = format!("{:?}", sync.list());
    let before_view = sync.get("probe-a");

    let kinds = [
        HostedKind::ShareHttp,
        HostedKind::SyncWsR2,
        HostedKind::SupportRelay,
        HostedKind::GithubExchange,
        HostedKind::Deploy,
    ];
    assert_eq!(kinds.len(), 5);
    let mut refused = 0usize;
    for kind in kinds {
        let err = sync
            .apply(SyncOp::ProbeHosted(kind))
            .expect_err("hosted partition must be refused by type");
        assert_eq!(err, ShareError::HostedPartition);
        let rendered = format!("{err:?}");
        assert!(!rendered.contains(key_ref));
        assert!(!rendered.contains(&seed_digest));
        assert_eq!(format!("{:?}", sync.list()), before);
        refused += 1;
    }
    assert_eq!(refused, 5);
    assert_eq!(sync.get("probe-a"), before_view);
}

fn child_pids() -> Vec<u32> {
    let me = std::process::id();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{name}/stat")) else {
            continue;
        };
        let Some(close) = stat.rfind(')') else {
            continue;
        };
        let mut fields = stat[close + 2..].split_whitespace();
        fields.next(); // state
        let is_child = fields.next().and_then(|ppid| ppid.parse::<u32>().ok()) == Some(me);
        if !is_child {
            continue;
        }
        if let Ok(pid) = name.parse::<u32>() {
            out.push(pid);
        }
    }
    out.sort_unstable();
    out
}

fn socket_count() -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
        for entry in entries.flatten() {
            let is_socket = std::fs::read_link(entry.path())
                .map(|target| target.to_string_lossy().starts_with("socket:"))
                .unwrap_or(false);
            if is_socket {
                count += 1;
            }
        }
    }
    count
}

fn drive_matrix(sync: &mut ShareSync, log: &mut String) {
    let key_ref = "k-MATRIX-REF-42";
    let good = hex_digest(0xf0);
    let other = hex_digest(0x0f);

    let ok = sync.apply(SyncOp::Upsert(desc("m", key_ref, &good, 1)));
    assert!(ok.is_ok());
    log.push_str("upsert-fresh-ok ");

    let ok = sync.apply(SyncOp::Upsert(desc("m", key_ref, &other, 1)));
    assert!(ok.is_ok());
    log.push_str("upsert-stale-ok ");

    let err = sync
        .apply(SyncOp::Upsert(desc("", key_ref, &good, 2)))
        .expect_err("bad id must be typed");
    assert_eq!(err, ShareError::InvalidId);
    log.push_str(&format!("{err:?} "));

    let err = sync
        .apply(SyncOp::Upsert(desc("m", "", &good, 2)))
        .expect_err("empty key_ref must be typed");
    assert_eq!(err, ShareError::SecretInvalid);
    log.push_str(&format!("{err:?} "));

    let err = sync
        .apply(SyncOp::Upsert(desc("m", key_ref, "short", 2)))
        .expect_err("short digest must be typed");
    assert_eq!(err, ShareError::InvalidDigest);
    log.push_str(&format!("{err:?} "));

    for kind in [
        HostedKind::ShareHttp,
        HostedKind::SyncWsR2,
        HostedKind::SupportRelay,
        HostedKind::GithubExchange,
        HostedKind::Deploy,
    ] {
        let err = sync
            .apply(SyncOp::ProbeHosted(kind))
            .expect_err("hosted must be refused");
        assert_eq!(err, ShareError::HostedPartition);
        log.push_str(&format!("{err:?} "));
    }

    let ok = sync.apply(SyncOp::Remove {
        id: "m".to_owned(),
        seq: 1,
    });
    assert!(ok.is_ok());
    log.push_str("remove-stale-ok ");

    let removed = sync
        .apply(SyncOp::Remove {
            id: "m".to_owned(),
            seq: 2,
        })
        .expect("fresh remove should succeed");
    assert_eq!(removed, None);
    log.push_str("remove-fresh-ok ");

    assert!(!log.contains(key_ref));
    assert!(!log.contains(&good));
    assert!(!log.contains(&other));
}

#[test]
fn int_010_t05_full_matrix_no_process_files_sockets_env_or_log_leak() {
    let dir = tempfile::tempdir().expect("disposable test dir should exist");
    let before_files: Vec<String> = std::fs::read_dir(dir.path())
        .expect("test dir should list")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let before_children = child_pids();
    let before_sockets = socket_count();

    std::env::set_var("INT_010_SENTINEL", "one");
    let mut first = ShareSync::new();
    let mut first_log = String::new();
    drive_matrix(&mut first, &mut first_log);
    let first_list = format!("{:?}", first.list());

    std::env::set_var("INT_010_SENTINEL", "two");
    let mut second = ShareSync::new();
    let mut second_log = String::new();
    drive_matrix(&mut second, &mut second_log);
    std::env::remove_var("INT_010_SENTINEL");

    assert_eq!(first_list, format!("{:?}", second.list()));
    assert_eq!(first_log, second_log);

    assert_eq!(child_pids(), before_children);
    assert_eq!(socket_count(), before_sockets);
    let after_files: Vec<String> = std::fs::read_dir(dir.path())
        .expect("test dir should list")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(after_files, before_files);
    assert!(std::fs::read_dir(dir.path()).expect("dir").flatten().all(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        !name.ends_with(".db") && !name.ends_with(".sqlite")
    }));
}
