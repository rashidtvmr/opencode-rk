use opencode_rk_providers::debug_export::{
    export_debug_bundle, DebugExportInput, DebugRecord, ExportBudget,
};
use serde_json::{json, Value};

fn parse_jsonl(jsonl: &str) -> Vec<Value> {
    jsonl
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("debug export must be valid JSONL"))
        .collect()
}

fn generous_budget() -> ExportBudget {
    ExportBudget {
        max_records: 64,
        max_bytes: 64 * 1024,
    }
}

fn sample_records() -> Vec<DebugRecord> {
    vec![
        DebugRecord::SessionMeta {
            session_id: "session-1".to_owned(),
            metadata: json!({
                "title": "provider debug session",
                "provider": "anthropic",
            }),
        },
        DebugRecord::Definition {
            identity: "tool:read:v1".to_owned(),
            body: json!({
                "name": "read",
                "input_schema": {"type": "object"},
            }),
        },
        DebugRecord::Turn {
            turn_id: "turn-1".to_owned(),
            role: "assistant".to_owned(),
            content: json!({"text": "I will inspect the file."}),
            definition_refs: vec!["tool:read:v1".to_owned()],
        },
        DebugRecord::Timing {
            turn_id: Some("turn-1".to_owned()),
            name: "provider_round_trip".to_owned(),
            duration_ms: 37,
        },
        DebugRecord::Error {
            turn_id: Some("turn-1".to_owned()),
            code: "provider_timeout".to_owned(),
            message: "provider timed out after the final delta".to_owned(),
        },
    ]
}

#[test]
fn prov_014_t01_structured_records_render_as_one_json_object_per_line() {
    let input = DebugExportInput {
        records: sample_records(),
        secret_values: Vec::new(),
    };

    let bundle = export_debug_bundle(&input, generous_budget())
        .expect("structured debug records should export");
    let rows = parse_jsonl(&bundle.jsonl);

    assert_eq!(rows.len(), 5);
    assert_eq!(
        rows.iter()
            .map(|row| row["kind"].as_str().expect("record kind"))
            .collect::<Vec<_>>(),
        vec!["session_meta", "definition", "turn", "timing", "error"]
    );
    assert_eq!(rows[0]["session_id"], "session-1");
    assert_eq!(rows[1]["identity"], "tool:read:v1");
    assert_eq!(rows[2]["turn_id"], "turn-1");
    assert_eq!(rows[3]["duration_ms"], 37);
    assert_eq!(rows[4]["code"], "provider_timeout");
}

#[test]
fn prov_014_t02_repeated_definitions_are_deduped_by_stable_identity_and_turns_keep_refs() {
    let definition = DebugRecord::Definition {
        identity: "tool:read:v1".to_owned(),
        body: json!({
            "name": "read",
            "input_schema": {"type": "object", "properties": {"path": {"type": "string"}}},
        }),
    };
    let input = DebugExportInput {
        records: vec![
            definition.clone(),
            definition,
            DebugRecord::Turn {
                turn_id: "turn-2".to_owned(),
                role: "assistant".to_owned(),
                content: json!({"tool_use": "read"}),
                definition_refs: vec!["tool:read:v1".to_owned()],
            },
        ],
        secret_values: Vec::new(),
    };

    let bundle = export_debug_bundle(&input, generous_budget())
        .expect("duplicate definitions should still export");
    let rows = parse_jsonl(&bundle.jsonl);
    let definitions = rows
        .iter()
        .filter(|row| row["kind"] == "definition")
        .collect::<Vec<_>>();
    let turn = rows
        .iter()
        .find(|row| row["kind"] == "turn")
        .expect("turn record must be retained");

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0]["identity"], "tool:read:v1");
    assert_eq!(turn["definition_refs"], json!(["tool:read:v1"]));
}

#[test]
fn prov_014_t03_secret_values_and_tokens_are_redacted_before_export() {
    let input = DebugExportInput {
        records: vec![DebugRecord::Turn {
            turn_id: "turn-secret".to_owned(),
            role: "user".to_owned(),
            content: json!({
                "api_key": "sk-live-secret-123",
                "authorization": "Bearer bearer-token-456",
                "nested": {"token": "bearer-token-456"},
            }),
            definition_refs: Vec::new(),
        }],
        secret_values: vec![
            "sk-live-secret-123".to_owned(),
            "bearer-token-456".to_owned(),
        ],
    };

    let bundle = export_debug_bundle(&input, generous_budget())
        .expect("records containing secrets should export only after redaction");

    assert!(!bundle.jsonl.contains("sk-live-secret-123"));
    assert!(!bundle.jsonl.contains("bearer-token-456"));
    assert!(
        bundle.jsonl.contains("[REDACTED]"),
        "export should make redaction visible instead of silently dropping the record"
    );
}

fn many_large_turns() -> Vec<DebugRecord> {
    (0..8)
        .map(|index| DebugRecord::Turn {
            turn_id: format!("turn-{index}"),
            role: "assistant".to_owned(),
            content: json!({"text": "x".repeat(512)}),
            definition_refs: Vec::new(),
        })
        .collect()
}

#[test]
fn prov_014_t04_record_and_byte_budgets_truncate_with_an_explicit_marker() {
    let record_limited = DebugExportInput {
        records: many_large_turns(),
        secret_values: Vec::new(),
    };
    let record_bundle = export_debug_bundle(
        &record_limited,
        ExportBudget {
            max_records: 3,
            max_bytes: 64 * 1024,
        },
    )
    .expect("record-bounded export should succeed with a truncation marker");
    let record_rows = parse_jsonl(&record_bundle.jsonl);

    assert!(record_rows.len() <= 3, "marker must count toward record budget");
    assert!(
        record_rows.iter().any(|row| row["kind"] == "truncation"),
        "record-budget truncation must be explicit"
    );

    let byte_limited = DebugExportInput {
        records: many_large_turns(),
        secret_values: Vec::new(),
    };
    let byte_budget = 256;
    let byte_bundle = export_debug_bundle(
        &byte_limited,
        ExportBudget {
            max_records: 64,
            max_bytes: byte_budget,
        },
    )
    .expect("byte-bounded export should succeed with a truncation marker");
    let byte_rows = parse_jsonl(&byte_bundle.jsonl);

    assert!(byte_bundle.jsonl.len() <= byte_budget);
    assert!(
        byte_rows.iter().any(|row| row["kind"] == "truncation"),
        "byte-budget truncation must be explicit"
    );
}

#[test]
fn prov_014_t05_offline_renderer_is_self_contained_and_has_no_network_urls() {
    let input = DebugExportInput {
        records: sample_records(),
        secret_values: Vec::new(),
    };

    let bundle = export_debug_bundle(&input, generous_budget())
        .expect("debug bundle should include an offline renderer");
    let html = bundle.renderer_html.to_ascii_lowercase();

    assert!(html.contains("<!doctype html"));
    assert!(html.contains("<style"), "renderer should carry its own styles");
    assert!(html.contains("<script"), "renderer should carry its own script");

    for forbidden in ["http://", "https://", "//cdn.", "<script src=", "<link rel="] {
        assert!(
            !html.contains(forbidden),
            "offline renderer must not require network resource {forbidden:?}"
        );
    }
}
