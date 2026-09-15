//! Bounded provider debug-export bundle generation.

use std::collections::HashSet;

use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub enum DebugRecord {
    SessionMeta {
        session_id: String,
        metadata: Value,
    },
    Definition {
        identity: String,
        body: Value,
    },
    Turn {
        turn_id: String,
        role: String,
        content: Value,
        definition_refs: Vec<String>,
    },
    Timing {
        turn_id: Option<String>,
        name: String,
        duration_ms: u64,
    },
    Error {
        turn_id: Option<String>,
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DebugExportInput {
    pub records: Vec<DebugRecord>,
    pub secret_values: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportBudget {
    pub max_records: usize,
    pub max_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugBundle {
    pub jsonl: String,
    pub renderer_html: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DebugExportError {
    #[error("debug export serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("debug export budget is too small to record truncation")]
    BudgetTooSmall,
}

const REDACTED: &str = "[REDACTED]";

/// Build one bounded, redacted JSONL artifact plus a self-contained renderer.
///
/// Definitions are emitted once per stable identity. The caller owns the input
/// records and the explicit output budget; this function retains no state after
/// it returns and performs no network or filesystem I/O.
pub fn export_debug_bundle(
    input: &DebugExportInput,
    budget: ExportBudget,
) -> Result<DebugBundle, DebugExportError> {
    let mut jsonl = String::new();
    let mut line_starts = Vec::new();
    let mut emitted_records = 0usize;
    let mut seen_definitions = HashSet::new();

    for record in &input.records {
        if let DebugRecord::Definition { identity, .. } = record {
            if !seen_definitions.insert(identity.clone()) {
                continue;
            }
        }

        let mut value = record_value(record);
        redact_value(&mut value, &input.secret_values);
        let line = serde_json::to_string(&value)?;

        if emitted_records >= budget.max_records
            || jsonl.len().saturating_add(line.len()).saturating_add(1) > budget.max_bytes
        {
            append_truncation_marker(&mut jsonl, &mut line_starts, &mut emitted_records, budget)?;
            return Ok(DebugBundle {
                renderer_html: offline_renderer(&jsonl),
                jsonl,
            });
        }

        line_starts.push(jsonl.len());
        jsonl.push_str(&line);
        jsonl.push('\n');
        emitted_records += 1;
    }

    Ok(DebugBundle {
        renderer_html: offline_renderer(&jsonl),
        jsonl,
    })
}

fn record_value(record: &DebugRecord) -> Value {
    match record {
        DebugRecord::SessionMeta {
            session_id,
            metadata,
        } => json!({
            "kind": "session_meta",
            "session_id": session_id,
            "metadata": metadata,
        }),
        DebugRecord::Definition { identity, body } => json!({
            "kind": "definition",
            "identity": identity,
            "body": body,
        }),
        DebugRecord::Turn {
            turn_id,
            role,
            content,
            definition_refs,
        } => json!({
            "kind": "turn",
            "turn_id": turn_id,
            "role": role,
            "content": content,
            "definition_refs": definition_refs,
        }),
        DebugRecord::Timing {
            turn_id,
            name,
            duration_ms,
        } => json!({
            "kind": "timing",
            "turn_id": turn_id,
            "name": name,
            "duration_ms": duration_ms,
        }),
        DebugRecord::Error {
            turn_id,
            code,
            message,
        } => json!({
            "kind": "error",
            "turn_id": turn_id,
            "code": code,
            "message": message,
        }),
    }
}

fn redact_value(value: &mut Value, secret_values: &[String]) {
    match value {
        Value::String(text) => {
            for secret in secret_values.iter().filter(|secret| !secret.is_empty()) {
                if text.contains(secret) {
                    *text = text.replace(secret, REDACTED);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_value(item, secret_values);
            }
        }
        Value::Object(fields) => {
            for value in fields.values_mut() {
                redact_value(value, secret_values);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn append_truncation_marker(
    jsonl: &mut String,
    line_starts: &mut Vec<usize>,
    emitted_records: &mut usize,
    budget: ExportBudget,
) -> Result<(), DebugExportError> {
    let marker = serde_json::to_string(&json!({
        "kind": "truncation",
        "reason": "debug_export_budget_exhausted"
    }))?;
    let marker_bytes = marker.len().saturating_add(1);

    if budget.max_records == 0 || marker_bytes > budget.max_bytes {
        return Err(DebugExportError::BudgetTooSmall);
    }

    while *emitted_records >= budget.max_records
        || jsonl.len().saturating_add(marker_bytes) > budget.max_bytes
    {
        let Some(start) = line_starts.pop() else {
            return Err(DebugExportError::BudgetTooSmall);
        };
        jsonl.truncate(start);
        *emitted_records -= 1;
    }

    jsonl.push_str(&marker);
    jsonl.push('\n');
    *emitted_records += 1;
    Ok(())
}

fn offline_renderer(jsonl: &str) -> String {
    let escaped = escape_html(jsonl);
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>OpenCode RK Debug Export</title>
<style>
body {{ font-family: ui-monospace, monospace; margin: 1rem; }}
pre {{ white-space: pre-wrap; overflow-wrap: anywhere; }}
</style>
</head>
<body>
<h1>Debug Export</h1>
<pre id="records">{escaped}</pre>
<script>
(() => {{
  const node = document.getElementById('records');
  const count = node.textContent.split('\n').filter(Boolean).length;
  document.title = `OpenCode RK Debug Export (${{count}} records)`;
}})();
</script>
</body>
</html>"#
    )
}

fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
