//! OPS-001 frozen tests T01..T05. Self-contained via #[path] include;
//! integrator wires `pub mod resource_ledger` into lib.rs later.
#[path = "../src/resource_ledger.rs"]
mod resource_ledger;

use resource_ledger::{
    attest_disabled, attest_single_enabled, AdmissionCaps, AttestError, BudgetLedger, LedgerError,
    OverBudget, RetentionPolicy, SubsystemProbe, WorkKind, DisabledCostAttestation,
};
use std::fs;

fn all_disabled_probes() -> Vec<SubsystemProbe> {
    vec![
        SubsystemProbe::disabled("plugin_host"),
        SubsystemProbe::disabled("js_host"),
        SubsystemProbe::disabled("file_watcher"),
        SubsystemProbe::disabled("lsp_service"),
        SubsystemProbe::disabled("optional_web"),
    ]
}

#[test]
fn resource_ledger_t01_all_off_zero_cost() {
    let probes = all_disabled_probes();
    let att = attest_disabled(&probes).expect("all-off must attest");
    assert_eq!(att, DisabledCostAttestation::ZERO);
    assert!(att.is_zero());
    assert_eq!(
        (att.tasks, att.threads, att.stores, att.watchers, att.bytes),
        (0, 0, 0, 0, 0)
    );
    // No fixture-FS handle opened: probe I/O counters stay zero.
    for p in &probes {
        assert_eq!(p.io_ops(), 0);
        assert_eq!(p.report(), DisabledCostAttestation::ZERO);
    }
}

#[test]
fn resource_ledger_t02_single_enable_isolation() {
    let mut probes = all_disabled_probes();
    probes[2] = SubsystemProbe::enabled("file_watcher", 1, 1, 0, 1, 4096);
    let att = attest_single_enabled(&probes, "file_watcher").expect("single enable must attest");
    assert_eq!(att, probes[2].report());
    assert!(!att.is_zero());
    for p in &probes {
        if p.name() != "file_watcher" {
            assert_eq!(p.report(), DisabledCostAttestation::ZERO);
            assert_eq!(p.io_ops(), 0);
        }
    }
    // Enabling a second probe does not change the first probe's report.
    let before = probes[2].report();
    probes[0] = SubsystemProbe::enabled("plugin_host", 2, 2, 1, 0, 8192);
    assert_eq!(probes[2].report(), before);
    assert_eq!(probes[0].report(), SubsystemProbe::enabled("plugin_host", 2, 2, 1, 0, 8192).report());
    // No longer exactly one enabled: isolation verdict refuses.
    assert!(attest_single_enabled(&probes, "file_watcher").is_err());
}

#[test]
fn resource_ledger_t03_admit_release_accounting() {
    let ledger = BudgetLedger::new(AdmissionCaps::default()).unwrap();
    let slots0 = ledger.available_slots(WorkKind::SessionInput);
    let bytes0 = ledger.available_bytes(WorkKind::SessionInput);
    let mut reservations = Vec::new();
    for _ in 0..10 {
        reservations.push(ledger.admit(WorkKind::SessionInput, 1000).unwrap());
    }
    assert_eq!(ledger.available_bytes(WorkKind::SessionInput), bytes0 - 10_000);
    assert_eq!(ledger.available_slots(WorkKind::SessionInput), slots0 - 10);
    // Release half via drop: available rises by exactly the released amount.
    let released: Vec<_> = reservations.drain(..5).collect();
    drop(released);
    assert_eq!(ledger.available_bytes(WorkKind::SessionInput), bytes0 - 5_000);
    assert_eq!(ledger.available_slots(WorkKind::SessionInput), slots0 - 5);
    // Re-admit succeeds.
    for _ in 0..5 {
        reservations.push(ledger.admit(WorkKind::SessionInput, 1000).unwrap());
    }
    assert_eq!(ledger.usage(WorkKind::SessionInput), (10, 10_000));
    // cancel() releases early and is idempotent.
    let mut r = ledger.admit(WorkKind::SessionInput, 1000).unwrap();
    assert_eq!(ledger.usage(WorkKind::SessionInput), (11, 11_000));
    r.cancel();
    assert_eq!(ledger.usage(WorkKind::SessionInput), (10, 10_000));
    r.cancel();
    assert_eq!(ledger.usage(WorkKind::SessionInput), (10, 10_000));
    drop(r);
    assert_eq!(ledger.usage(WorkKind::SessionInput), (10, 10_000));
    // Saturates at zero, never underflows.
    drop(reservations);
    assert_eq!(ledger.usage(WorkKind::SessionInput), (0, 0));
    let mut r2 = ledger.admit(WorkKind::SessionInput, 500).unwrap();
    r2.cancel();
    r2.cancel();
    drop(r2);
    assert_eq!(ledger.usage(WorkKind::SessionInput), (0, 0));
    assert_eq!(ledger.available_bytes(WorkKind::SessionInput), bytes0);
    assert_eq!(ledger.available_slots(WorkKind::SessionInput), slots0);
}

#[test]
fn resource_ledger_t04_count_and_byte_double_bound() {
    // Count bound: 300 tiny agent admits exhaust slots while bytes stay under cap.
    let ledger = BudgetLedger::new(AdmissionCaps::default()).unwrap();
    let mut guards = Vec::new();
    for _ in 0..256 {
        guards.push(ledger.admit(WorkKind::AgentSlot, 1).unwrap());
    }
    let before = ledger.snapshot();
    let err = ledger.admit(WorkKind::AgentSlot, 1).unwrap_err();
    assert_eq!(
        err,
        OverBudget::SlotsExhausted {
            kind: WorkKind::AgentSlot
        }
    );
    assert!(ledger.available_bytes(WorkKind::AgentSlot) > 0);
    assert_eq!(ledger.snapshot(), before);
    drop(guards);
    // Byte bound: one 2 MiB session input refused while slots stay under cap.
    let ledger2 = BudgetLedger::new(AdmissionCaps::default()).unwrap();
    let before2 = ledger2.snapshot();
    let err2 = ledger2
        .admit(WorkKind::SessionInput, 2 * 1024 * 1024)
        .unwrap_err();
    match err2 {
        OverBudget::BytesExhausted {
            kind,
            requested,
            available,
        } => {
            assert_eq!(kind, WorkKind::SessionInput);
            assert_eq!(requested, 2 * 1024 * 1024);
            assert_eq!(available, 1_048_576);
        }
        other => panic!("expected BytesExhausted, got {other:?}"),
    }
    assert!(ledger2.available_slots(WorkKind::SessionInput) > 0);
    assert_eq!(ledger2.snapshot(), before2);
}

#[test]
fn resource_ledger_t05_no_silent_deletion_and_invalid_caps() {
    // Disposable fixture dir; sentinel sibling stands in for DB/outside state.
    let base = std::env::temp_dir().join(format!("ops001-spill-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let fixture = base.join("fixture");
    fs::create_dir_all(&fixture).unwrap();
    let sentinel = base.join("sentinel-outside.txt");
    fs::write(&sentinel, b"untouched").unwrap();

    let ledger = BudgetLedger::new(AdmissionCaps::default()).unwrap();
    let marker = b"precious-user-history-must-survive-";
    let mut payload = Vec::new();
    while payload.len() <= 70_000 {
        payload.extend_from_slice(marker);
    }
    assert!((payload.len() as u64) > AdmissionCaps::default().max_preview_bytes_per_tool);
    let mut log = String::new();
    let spilled = ledger
        .spill_tool_preview(&payload, RetentionPolicy::SpillToDisk, &fixture, &mut log)
        .unwrap();
    assert_eq!(spilled.bytes, payload.len() as u64);
    assert_eq!(spilled.policy, RetentionPolicy::SpillToDisk);
    // Original bytes recoverable from the spill path, inside the fixture dir.
    assert!(spilled.path.starts_with(&fixture));
    assert_eq!(fs::read(&spilled.path).unwrap(), payload);
    // Zero spilled-body bytes in captured logs.
    assert!(!log.contains("precious-user-history"));
    assert!(!log.contains(marker.iter().map(|b| *b as char).collect::<String>().as_str()));
    // Nothing written outside the fixture dir.
    assert_eq!(fs::read(&sentinel).unwrap(), b"untouched");
    let mut stray = Vec::new();
    for entry in fs::read_dir(&base).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name != "fixture" && name != "sentinel-outside.txt" {
            stray.push(name);
        }
    }
    assert!(stray.is_empty(), "writes escaped fixture dir: {stray:?}");
    let _ = fs::remove_dir_all(&base);

    // Each zeroed cap field is rejected with its own name.
    let good = AdmissionCaps::default();
    let cases = [
        (
            AdmissionCaps {
                max_queued_agents: 0,
                ..good
            },
            "max_queued_agents",
        ),
        (
            AdmissionCaps {
                max_input_bytes_per_session: 0,
                ..good
            },
            "max_input_bytes_per_session",
        ),
        (
            AdmissionCaps {
                max_preview_bytes_per_tool: 0,
                ..good
            },
            "max_preview_bytes_per_tool",
        ),
        (
            AdmissionCaps {
                max_subscriber_bytes: 0,
                ..good
            },
            "max_subscriber_bytes",
        ),
    ];
    for (bad, name) in cases {
        assert_eq!(bad.validate(), Err(LedgerError::InvalidCap(name)));
        assert_eq!(BudgetLedger::new(bad), Err(LedgerError::InvalidCap(name)));
    }
    // Silence unused-import check if attestation error type is untouched above.
    let _ = AttestError::CostLeak {
        subsystem: "unused",
    };
}
