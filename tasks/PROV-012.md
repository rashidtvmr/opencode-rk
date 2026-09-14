# PROV-012

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: no.
Requirements: REQ-007.
Dependencies: PROV-001.
Test obligations: PROV-012-T01, PROV-012-T02, PROV-012-T03, PROV-012-T04, PROV-012-T05.

## User-observable outcome

Debug export capability for provider configurations. Exposes ExportFormat enum
(Json, Yaml, Binary), DebugExporter struct holding a Vec<Provider> and a format,
with methods export(), export_to_file(path), with_format(format), and
validate_before_export(). Skips export for empty provider lists.

## Source evidence

- crates/providers/src/registry.rs:7-41 Provider struct (id, name, base_url,
  api_key_env, priority, timeout_secs).
- crates/providers/src/lib.rs:7 debug_export module declaration.
- crates/providers/Cargo.toml: serde_json, thiserror, tokio dependencies available.

## Observable contract

- ExportFormat enum with variants Json, Yaml, Binary.
- DebugExporter::new(providers) creates exporter defaulting to Json format.
- with_format(format) sets the export format and returns self for chaining.
- validate_before_export() returns bool -- false when providers is empty.
- export() returns Vec<u8> of serialized data; empty Vec when validation fails.
- export_to_file(path) writes serialized bytes to disk; returns io::Result<()>.
- JSON output is valid serde_json serialization of provider fields.
- YAML output is a minimal hand-rolled emitter (no serde_yaml dependency).
- Binary output uses a custom compact wire format with u32 count header and
  length-prefixed strings.
- No unbounded allocation beyond the serialized output itself.

## Negative cases

- Empty provider list: validate returns false, export returns empty Vec.
- File write errors propagate as std::io::Error.
