//! PAR-010 differential gate: pinned-source parity dispositions stay honest.
//! Reads checked-in JSON as text (no CWD dependence via include_str!),
//! asserts dep-lane completion, module presence, and evidence integrity.
//! Certified stays false: unwired/unverified/missing rows remain.

use std::collections::BTreeSet;

const PARITY: &str = include_str!("../../../tasks/completion/parity.json");
const CLAIMS: &str = include_str!("../../../tasks/completion/claims.json");
const EVIDENCE: &str = include_str!("../../../sources/completion/surface-evidence.json");
const SERVER_LIB: &str = include_str!("../src/lib.rs");

// (crate dir, module file) for the 8 key modules PAR-002..009 wired.
const MODULES: &[(&str, &str)] = &[
    ("providers", "app_routing"),
    ("sessions", "app_history"),
    ("agents", "app_delegation"),
    ("tools", "app_services"),
    ("security", "app_policy"),
    ("tools", "app_extensions"),
    ("server", "web_turn_adapter"),
    ("server", "app_protocols"),
];

fn workspace_root() -> std::path::PathBuf {
    let m = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    m.join("..").join("..").to_path_buf()
}

#[test]
fn par010_t01_every_story_id_resolvable() {
    for id in [
        "PAR-001", "PAR-002", "PAR-003", "PAR-004", "PAR-005", "PAR-006", "PAR-007", "PAR-008",
        "PAR-009", "PAR-010",
    ] {
        assert!(PARITY.contains(id), "story id missing from parity.json: {id}");
    }
}

#[test]
fn par010_t02_dep_lanes_completed() {
    let v: serde_json::Value = serde_json::from_str(CLAIMS).expect("claims.json parses");
    let claims = v.get("claims").expect("claims map");
    for id in [
        "PAR-002", "PAR-003", "PAR-004", "PAR-005", "PAR-006", "PAR-007", "PAR-008", "PAR-009",
        "TUI-010", "DISC-106", "DISC-110",
    ] {
        let status = claims
            .get(id)
            .and_then(|r| r.get("status"))
            .and_then(|s| s.as_str())
            .unwrap_or("MISSING");
        assert_eq!(status, "completed", "dep lane not completed: {id}={status}");
    }
}

#[test]
fn par010_t03_key_modules_present() {
    let root = workspace_root();
    let mut missing = Vec::new();
    for (krate, module) in MODULES {
        let p = root.join("crates").join(krate).join("src").join(format!("{module}.rs"));
        if !p.is_file() {
            missing.push(p.display().to_string());
        }
    }
    assert!(missing.is_empty(), "key modules missing: {missing:?}");
    // server-side pair also declared in lib.rs (compile-visible wiring)
    for m in ["web_turn_adapter", "app_protocols"] {
        assert!(SERVER_LIB.contains(m), "lib.rs does not declare {m}");
    }
}

#[test]
fn par010_t04_evidence_parses_with_dispositions() {
    let v: serde_json::Value = serde_json::from_str(EVIDENCE).expect("surface-evidence parses");
    let allowed: BTreeSet<&str> = ["implemented", "unwired", "missing", "unverified"]
        .into_iter()
        .collect();
    let surfaces = v.get("surfaces").expect("surfaces array").as_array().unwrap();
    assert!(!surfaces.is_empty(), "no surfaces tracked");
    for s in surfaces {
        let d = s.get("disposition").and_then(|x| x.as_str()).unwrap_or("?");
        assert!(allowed.contains(d), "bad disposition {d}");
    }
    // honest gate: certification blockers present, certified false until wired+verified
    assert_eq!(v.get("certified").and_then(|c| c.as_bool()), Some(false));
    let blockers = v
        .get("certificationBlockers")
        .and_then(|b| b.as_array())
        .expect("blockers array");
    assert!(!blockers.is_empty(), "blockers must be listed while uncertified");
}

#[test]
fn par010_t05_no_silent_implemented_claim() {
    // No row may claim implemented without an executable trace: enforce zero
    // implemented rows at this gate revision (wiring lanes land traces later).
    let v: serde_json::Value = serde_json::from_str(EVIDENCE).unwrap();
    let n = v["surfaces"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s.get("disposition").and_then(|d| d.as_str()) == Some("implemented"))
        .count();
    assert_eq!(n, 0, "implemented claims require entrypoint traces; found {n}");
}
