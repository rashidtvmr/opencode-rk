//! REL-006 frozen tests: telemetry report renderer (T01..T05).
//!
//! Integration target `stats_report`. The renderer module is included by
//! path until the integrator wires `pub mod stats_report` in the crate
//! root; this test file is frozen after RED and must not be edited to
//! make code pass.

#[path = "../src/stats_report.rs"]
mod stats_report;

use stats_report::{ReportConfig, TelemetryEntry, TelemetrySnapshot, render};

fn entry(label: &str, saved_tokens: u64) -> TelemetryEntry {
    TelemetryEntry {
        label: label.to_string(),
        saved_tokens,
    }
}

#[test]
fn rel006_t01_golden_identical() {
    let snap = TelemetrySnapshot {
        total: 3,
        saved_tokens: 300,
        total_tokens: 1000,
        recent: vec![entry("gzip", 200), entry("rtk", 100)],
    };
    assert_eq!(
        render(&snap, &ReportConfig::default()),
        "telemetry: 3 compressions, 300 tokens saved (30%)\nrecent:\n- gzip: 200 saved\n- rtk: 100 saved\n"
    );
}

#[test]
fn rel006_t02_empty_message() {
    let snap = TelemetrySnapshot {
        total: 0,
        saved_tokens: 0,
        total_tokens: 0,
        recent: vec![],
    };
    assert_eq!(
        render(&snap, &ReportConfig::default()),
        "no telemetry recorded yet\n"
    );
}

#[test]
fn rel006_t03_secret_absence() {
    let secrets = ["sk-live-abc123", "api_key=ZZZ", "-----BEGIN PRIVATE KEY-----"];
    let snap = TelemetrySnapshot {
        total: 3,
        saved_tokens: 30,
        total_tokens: 100,
        recent: secrets.iter().map(|s| entry(s, 10)).collect(),
    };
    let out = render(&snap, &ReportConfig::default());
    for s in secrets {
        assert!(!out.contains(s), "secret leaked: {s}");
    }
    assert!(out.contains("[redacted]"));
}

#[test]
fn rel006_t04_truncation_marker() {
    let snap = TelemetrySnapshot {
        total: 10,
        saved_tokens: 100,
        total_tokens: 1000,
        recent: (0..10).map(|i| entry(&format!("job-{i}"), i)).collect(),
    };
    let cfg = ReportConfig {
        enabled: true,
        max_entries: 5,
        max_bytes: 8192,
    };
    let out = render(&snap, &cfg);
    assert_eq!(out.lines().filter(|l| l.starts_with("- ")).count(), 5);
    assert!(out.ends_with("... [truncated 5 entries]\n"));
}

#[test]
fn rel006_t05_opt_out_message() {
    let snap = TelemetrySnapshot {
        total: 2,
        saved_tokens: 50,
        total_tokens: 200,
        recent: vec![entry("gzip", 40), entry("sk-live-abc123", 10)],
    };
    let cfg = ReportConfig {
        enabled: false,
        max_entries: 5,
        max_bytes: 8192,
    };
    assert_eq!(render(&snap, &cfg), "telemetry disabled (opt-out set)\n");
}
