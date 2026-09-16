// TOOL-019 contract tests: enabled-only MCP payload filter with hash snapshot.
// Maps to obligations TOOL-019-T01..T05 in tasks/TOOL-019.md.
use opencode_rk_tools::mcp_payload_filter::{
    MAX_FIELD_CHARS, PayloadError, PayloadInput, ServerFlag, ToolEntry, filter_payload,
    lookup_tool, verify_snapshot,
};

fn input() -> PayloadInput {
    PayloadInput {
        servers: vec![
            ServerFlag {
                id: "srv-a".to_owned(),
                enabled: true,
            },
            ServerFlag {
                id: "srv-b".to_owned(),
                enabled: true,
            },
            ServerFlag {
                id: "srv-c".to_owned(),
                enabled: false,
            },
        ],
        tools: vec![
            ToolEntry {
                server_id: "srv-a".to_owned(),
                name: "read".to_owned(),
            },
            ToolEntry {
                server_id: "srv-b".to_owned(),
                name: "write".to_owned(),
            },
            ToolEntry {
                server_id: "srv-c".to_owned(),
                name: "exec".to_owned(),
            },
        ],
    }
}

#[test]
fn tool019_t01_exclusion_happy_path() {
    let filtered = filter_payload(&input()).unwrap();
    assert_eq!(
        filtered.servers,
        vec!["srv-a".to_string(), "srv-b".to_string()]
    );
    assert_eq!(filtered.tools.len(), 2);
    assert!(filtered.tools.iter().all(|t| t.server_id != "srv-c"));
    assert!(verify_snapshot(&filtered, filtered.snapshot_hash));
}

#[test]
fn tool019_t02_lookup_gating() {
    let filtered = filter_payload(&input()).unwrap();
    let hit = lookup_tool(&filtered, "srv-a", "read").unwrap();
    assert_eq!(hit.name, "read");
    assert_eq!(
        lookup_tool(&filtered, "srv-c", "exec"),
        Err(PayloadError::NotEnabled)
    );
    assert_eq!(
        lookup_tool(&filtered, "srv-a", "missing"),
        Err(PayloadError::Unknown)
    );
    assert_eq!(
        lookup_tool(&filtered, "ghost", "read"),
        Err(PayloadError::Unknown)
    );
}

#[test]
fn tool019_t03_stale_state_proof() {
    let before = filter_payload(&input()).unwrap();
    let old_hash = before.snapshot_hash;
    let mut flipped = input();
    flipped
        .servers
        .iter_mut()
        .find(|s| s.id == "srv-b")
        .unwrap()
        .enabled = false;
    let after = filter_payload(&flipped).unwrap();
    assert!(after.tools.iter().all(|t| t.server_id != "srv-b"));
    assert_ne!(after.snapshot_hash, old_hash);
    assert!(!verify_snapshot(&after, old_hash));
    assert!(verify_snapshot(&after, after.snapshot_hash));
}

#[test]
fn tool019_t04_validation() {
    let mut bad = input();
    bad.servers.push(ServerFlag {
        id: String::new(),
        enabled: true,
    });
    assert_eq!(filter_payload(&bad), Err(PayloadError::EmptyField));
    let mut big = PayloadInput {
        servers: vec![ServerFlag {
            id: "s".to_owned(),
            enabled: true,
        }],
        tools: Vec::new(),
    };
    for i in 0..513 {
        big.tools.push(ToolEntry {
            server_id: "s".to_owned(),
            name: format!("t{i}"),
        });
    }
    assert_eq!(filter_payload(&big), Err(PayloadError::OverCap));
    let long = "x".repeat(MAX_FIELD_CHARS + 1);
    let over = PayloadInput {
        servers: vec![ServerFlag {
            id: long,
            enabled: true,
        }],
        tools: Vec::new(),
    };
    assert_eq!(filter_payload(&over), Err(PayloadError::OverCap));
}

#[test]
fn tool019_t05_determinism_and_isolation() {
    let a = filter_payload(&input()).unwrap();
    let b = filter_payload(&input()).unwrap();
    assert_eq!(a.snapshot_hash, b.snapshot_hash);
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    let text = serde_json::to_vec(&a)
        .map(String::from_utf8)
        .unwrap()
        .unwrap();
    assert!(text.contains("srv-a"));
    assert!(!text.contains("secret"));
    let dbg = format!("{a:?}");
    assert!(!dbg.contains("secret"));
    assert!(!dbg.contains("sk-"));
}
